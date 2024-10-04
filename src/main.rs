use std::io::{self, BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Sender};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use lazy_static::lazy_static;
use regex::Regex;
use tui::backend::CrosstermBackend;
use tui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame, Terminal,
};

use libc::{
    c_int, host_info64_t, host_statistics64, mach_host_self, mach_msg_type_number_t, natural_t,
    vm_statistics64_data_t, HOST_VM_INFO64,
};

#[derive(Clone)]
struct CPUMetrics {
    e_cluster_active: i32,
    e_cluster_freq_mhz: i32,
    p_cluster_active: i32,
    p_cluster_freq_mhz: i32,
    ane_w: f64,
    cpu_w: f64,
    gpu_w: f64,
    package_w: f64,
}

impl CPUMetrics {
    fn new() -> Self {
        Self {
            e_cluster_active: 0,
            e_cluster_freq_mhz: 0,
            p_cluster_active: 0,
            p_cluster_freq_mhz: 0,
            ane_w: 0.0,
            cpu_w: 0.0,
            gpu_w: 0.0,
            package_w: 0.0,
        }
    }
}

#[derive(Clone)]
struct NetDiskMetrics {
    out_packets_per_sec: f64,
    out_bytes_per_sec: f64,
    in_packets_per_sec: f64,
    in_bytes_per_sec: f64,
    read_ops_per_sec: f64,
    write_ops_per_sec: f64,
    read_kbytes_per_sec: f64,
    write_kbytes_per_sec: f64,
}

impl NetDiskMetrics {
    fn new() -> Self {
        Self {
            out_packets_per_sec: 0.0,
            out_bytes_per_sec: 0.0,
            in_packets_per_sec: 0.0,
            in_bytes_per_sec: 0.0,
            read_ops_per_sec: 0.0,
            write_ops_per_sec: 0.0,
            read_kbytes_per_sec: 0.0,
            write_kbytes_per_sec: 0.0,
        }
    }
}

#[derive(Clone)]
struct GPUMetrics {
    freq_mhz: i32,
    active: f64,
}

impl GPUMetrics {
    fn new() -> Self {
        Self {
            freq_mhz: 0,
            active: 0.0,
        }
    }
}

struct MemoryMetrics {
    total: u64,
    used: u64,
    available: u64,
    free: u64,
    swap_total: u64,
    swap_used: u64,
    used_percent: f32,
}

impl MemoryMetrics {
    fn new() -> Self {
        get_memory_metrics()
    }
}

struct EventThrottler {
    last_event: Instant,
    grace_period: Duration,
}

impl EventThrottler {
    fn new(grace_period: Duration) -> Self {
        Self {
            last_event: Instant::now() - grace_period,
            grace_period,
        }
    }

    fn should_notify(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_event) >= self.grace_period {
            self.last_event = now;
            true
        } else {
            false
        }
    }
}

lazy_static! {
    static ref OUT_REGEX: Regex =
        Regex::new(r"out:\s*([\d.]+)\s*packets/s,\s*([\d.]+)\s*bytes/s").unwrap();
    static ref IN_REGEX: Regex =
        Regex::new(r"in:\s*([\d.]+)\s*packets/s,\s*([\d.]+)\s*bytes/s").unwrap();
    static ref READ_REGEX: Regex =
        Regex::new(r"read:\s*([\d.]+)\s*ops/s\s*([\d.]+)\s*KBytes/s").unwrap();
    static ref WRITE_REGEX: Regex =
        Regex::new(r"write:\s*([\d.]+)\s*ops/s,\s*([\d.]+)\s*KBytes/s").unwrap();
    static ref RESIDENCY_RE: Regex =
        Regex::new(r"(\w+-Cluster)\s+HW active residency:\s+(\d+\.\d+)%").unwrap();
    static ref FREQUENCY_RE: Regex =
        Regex::new(r"(\w+-Cluster)\s+HW active frequency:\s+(\d+)\s+MHz").unwrap();
    static ref GPU_ACTIVE_RE: Regex =
        Regex::new(r"GPU\s*(HW)?\s*active\s*residency:\s+(\d+\.\d+)%").unwrap();
    static ref GPU_FREQ_RE: Regex =
        Regex::new(r"GPU\s*(HW)?\s*active\s*frequency:\s+(\d+)\s+MHz").unwrap();
    static ref SWAP_REGEX: Regex =
        Regex::new(r"total = (\d+\.\d+)([MG])\s+used = (\d+\.\d+)([MG])\s+free = (\d+\.\d+)([MG])").unwrap();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if unsafe { libc::geteuid() } != 0 {
        eprintln!(
            "welcome to mtop!"
        );
        std::process::exit(1);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (cpu_tx, cpu_rx) = unbounded();
    let (gpu_tx, gpu_rx) = unbounded();
    let (netdisk_tx, netdisk_rx) = unbounded();

    let running = Arc::new(Mutex::new(true));
    let running_clone = Arc::clone(&running);

    thread::spawn(move || {
        collect_metrics(cpu_tx, gpu_tx, netdisk_tx, running_clone);
    });

    let mut need_render = EventThrottler::new(Duration::from_millis(500));

    let mut cpu_metrics = CPUMetrics::new();
    let mut gpu_metrics = GPUMetrics::new();
    let mut netdisk_metrics = NetDiskMetrics::new();

    let model_info = get_apple_silicon_info();

    // Main Event Loop
    loop {
        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
                    let mut running = running.lock().unwrap();
                    *running = false;
                    break;
                }
            }
        }

        let mut updated = false;

        while let Ok(metrics) = cpu_rx.try_recv() {
            cpu_metrics = metrics;
            updated = true;
        }

        while let Ok(metrics) = gpu_rx.try_recv() {
            gpu_metrics = metrics;
            updated = true;
        }

        while let Ok(metrics) = netdisk_rx.try_recv() {
            netdisk_metrics = metrics;
            updated = true;
        }

        if updated || need_render.should_notify() {
            let memory_metrics = MemoryMetrics::new();

            // Render UI
            terminal.draw(|f| {
                draw_ui(
                    f,
                    &cpu_metrics,
                    &gpu_metrics,
                    &netdisk_metrics,
                    &model_info,
                    &memory_metrics,
                )
            })?;
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn draw_ui(
    f: &mut Frame<CrosstermBackend<std::io::Stdout>>,
    cpu_metrics: &CPUMetrics,
    gpu_metrics: &GPUMetrics,
    netdisk_metrics: &NetDiskMetrics,
    model_info: &AppleSiliconInfo,
    memory_metrics: &MemoryMetrics,
) {
    let size = f.size();

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(50), // Top Half
                Constraint::Percentage(50), // Bottom Half
            ]
            .as_ref(),
        )
        .split(size);

    let top_columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(50), // Left Column
                Constraint::Percentage(50), // Right Column
            ]
            .as_ref(),
        )
        .split(vertical_chunks[0]);

    let left_top_bottom = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(50), // Top Block
                Constraint::Percentage(50), // Bottom Block
            ]
            .as_ref(),
        )
        .split(top_columns[0]);

    let right_top_bottom = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(50), // Top Block
                Constraint::Percentage(50), // Bottom Block
            ]
            .as_ref(),
        )
        .split(top_columns[1]);

    let bottom_vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(50), // Memory Usage
                Constraint::Percentage(50), // Other Info
            ]
            .as_ref(),
        )
        .split(vertical_chunks[1]);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ]
            .as_ref(),
        )
        .split(bottom_vertical_chunks[1]);

    // --- Top Half Widgets ---

    // Left Column - Top: E-CPU Usage
    render_label_and_gauge(
        f,
        left_top_bottom[0],
        &format!(
            "E-CPU Usage: {}% @ {}MHz",
            cpu_metrics.e_cluster_active, cpu_metrics.e_cluster_freq_mhz
        ),
        cpu_metrics.e_cluster_active as u16,
        Color::Green,
    );

    // Left Column - Bottom: P-CPU Usage
    render_label_and_gauge(
        f,
        left_top_bottom[1],
        &format!(
            "P-CPU Usage: {}% @ {}MHz",
            cpu_metrics.p_cluster_active, cpu_metrics.p_cluster_freq_mhz
        ),
        cpu_metrics.p_cluster_active as u16,
        Color::Yellow,
    );

    // Right Column - Top: GPU Usage
    render_label_and_gauge(
        f,
        right_top_bottom[0],
        &format!(
            "GPU Usage: {:.0}% @ {}MHz",
            gpu_metrics.active, gpu_metrics.freq_mhz
        ),
        gpu_metrics.active as u16,
        Color::Magenta,
    );

    // Right Column - Bottom: ANE Usage
    let ane_util = (cpu_metrics.ane_w * 100.0 / 8.0).clamp(0.0, 100.0); // Assuming 8W max ANE power
    render_label_and_gauge(
        f,
        right_top_bottom[1],
        &format!(
            "ANE Usage: {:.0}% @ {:.2} W",
            ane_util, cpu_metrics.ane_w
        ),
        ane_util as u16,
        Color::Blue,
    );

    // --- Bottom Half Widgets ---

    // Memory Usage: Block Title + Gauge
    let memory_label = if memory_metrics.swap_total > 0 {
        format!(
            "Memory Usage: {:.2} GB / {:.2} GB (Swap Used: {:.2} GB / {:.2} GB)",
            (memory_metrics.used) as f64 / 1024.0 / 1024.0 / 1024.0,
            (memory_metrics.total) as f64 / 1024.0 / 1024.0 / 1024.0,
            (memory_metrics.swap_used) as f64 / 1024.0 / 1024.0 / 1024.0,
            (memory_metrics.swap_total) as f64 / 1024.0 / 1024.0 / 1024.0,
        )
    } else {
        format!(
            "Memory Usage: {:.2} GB / {:.2} GB (Swap Used: 0.00 GB / 0.00 GB)",
            (memory_metrics.used) as f64 / 1024.0 / 1024.0 / 1024.0,
            (memory_metrics.total) as f64 / 1024.0 / 1024.0 / 1024.0,
        )
    };

    render_label_and_gauge(
        f,
        bottom_vertical_chunks[0],
        &memory_label,
        memory_metrics.used_percent as u16,
        Color::Cyan,
    );

    // --- Bottom Bottom Half Widgets ---

    // Apple Silicon Info
    let model_text = format!(
        "Model: {}\nE-Cores: {}\nP-Cores: {}\nGPU Cores: {}",
        model_info.name,
        model_info.e_core_count,
        model_info.p_core_count,
        model_info.gpu_core_count,
    );
    let model_paragraph = Paragraph::new(model_text)
        .block(
            Block::default()
                .title("Apple Silicon Info")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(model_paragraph, bottom_chunks[0]);

    // Network & Disk Info
    let netdisk_text = format!(
        "Out: {:.1} packets/s, {:.1} bytes/s\n\
        In: {:.1} packets/s, {:.1} bytes/s\n\
        Read: {:.1} ops/s, {:.1} KB/s\n\
        Write: {:.1} ops/s, {:.1} KB/s",
        netdisk_metrics.out_packets_per_sec,
        netdisk_metrics.out_bytes_per_sec,
        netdisk_metrics.in_packets_per_sec,
        netdisk_metrics.in_bytes_per_sec,
        netdisk_metrics.read_ops_per_sec,
        netdisk_metrics.read_kbytes_per_sec,
        netdisk_metrics.write_ops_per_sec,
        netdisk_metrics.write_kbytes_per_sec,
    );
    let netdisk_paragraph = Paragraph::new(netdisk_text)
        .block(
            Block::default()
                .title("Network & Disk Info")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(netdisk_paragraph, bottom_chunks[1]);

    // Power Information
    let power_text = format!(
        "CPU Power: {:.2} W\n\
        GPU Power: {:.2} W\n\
        ANE Power: {:.2} W\n\
        Total Power: {:.2} W",
        cpu_metrics.cpu_w,
        cpu_metrics.gpu_w,
        cpu_metrics.ane_w,
        cpu_metrics.package_w
    );
    let power_paragraph = Paragraph::new(power_text)
        .block(Block::default().title("Power Usage").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(power_paragraph, bottom_chunks[2]);
}

/// Renders a block title above a gauge within a given area.
///
/// # Arguments
///
/// * `f` - The frame to render to.
/// * `area` - The area allocated for this widget.
/// * `label` - The text label to display as the block title.
/// * `percent` - The percentage value to display inside the gauge.
/// * `color` - The color of the gauge.
fn render_label_and_gauge(
    f: &mut Frame<CrosstermBackend<std::io::Stdout>>,
    area: Rect,
    label: &str,
    percent: u16,
    color: Color,
) {
    // Render gauge with block title
    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(label)
                .borders(Borders::ALL),
        )
        .gauge_style(Style::default().fg(color))
        .percent(percent)
        .label(format!("{}%", percent));
    f.render_widget(gauge, area);
}

fn collect_metrics(
    cpu_tx: Sender<CPUMetrics>,
    gpu_tx: Sender<GPUMetrics>,
    netdisk_tx: Sender<NetDiskMetrics>,
    running: Arc<Mutex<bool>>,
) {
    let mut cmd = Command::new("powermetrics")
        .args(&[
            "--samplers",
            "cpu_power,gpu_power,thermal,network,disk",
            "--show-initial-usage",
            "-i",
            "1000",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start powermetrics");

    let stdout = cmd.stdout.take().expect("Failed to get stdout");
    let reader = BufReader::new(stdout);

    let mut cpu_metrics = CPUMetrics::new();
    let mut gpu_metrics = GPUMetrics::new();
    let mut netdisk_metrics = NetDiskMetrics::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        if !*running.lock().unwrap() {
            let _ = cmd.kill();
            break;
        }

        parse_cpu_metrics(&line, &mut cpu_metrics);
        parse_gpu_metrics(&line, &mut gpu_metrics);
        parse_netdisk_metrics(&line, &mut netdisk_metrics);

        let _ = cpu_tx.send(cpu_metrics.clone());
        let _ = gpu_tx.send(gpu_metrics.clone());
        let _ = netdisk_tx.send(netdisk_metrics.clone());
    }
}

fn parse_cpu_metrics(line: &str, cpu_metrics: &mut CPUMetrics) {
    if let Some(caps) = RESIDENCY_RE.captures(line) {
        let cluster = &caps[1];
        let percent: f64 = caps[2].parse().unwrap_or(0.0);
        match cluster {
            "E-Cluster" | "E0-Cluster" => cpu_metrics.e_cluster_active = percent as i32,
            "P-Cluster" | "P0-Cluster" => cpu_metrics.p_cluster_active = percent as i32,
            _ => {}
        }
    }

    if let Some(caps) = FREQUENCY_RE.captures(line) {
        let cluster = &caps[1];
        let freq_mhz: i32 = caps[2].parse().unwrap_or(0);
        match cluster {
            "E-Cluster" | "E0-Cluster" => cpu_metrics.e_cluster_freq_mhz = freq_mhz,
            "P-Cluster" | "P0-Cluster" => cpu_metrics.p_cluster_freq_mhz = freq_mhz,
            _ => {}
        }
    }

    if line.contains("ANE Power") {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            cpu_metrics.ane_w =
                parts[2].trim_end_matches("mW").parse().unwrap_or(0.0) / 1000.0;
        }
    } else if line.contains("CPU Power") {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            cpu_metrics.cpu_w =
                parts[2].trim_end_matches("mW").parse().unwrap_or(0.0) / 1000.0;
        }
    } else if line.contains("GPU Power") {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            cpu_metrics.gpu_w =
                parts[2].trim_end_matches("mW").parse().unwrap_or(0.0) / 1000.0;
        }
    } else if line.contains("Combined Power (CPU + GPU + ANE)") {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 8 {
            cpu_metrics.package_w =
                parts[7].trim_end_matches("mW").parse().unwrap_or(0.0) / 1000.0;
        }
    }
}

fn parse_gpu_metrics(line: &str, gpu_metrics: &mut GPUMetrics) {
    if let Some(caps) = GPU_ACTIVE_RE.captures(line) {
        let percent: f64 = caps[2].parse().unwrap_or(0.0);
        gpu_metrics.active = percent;
    }

    if let Some(caps) = GPU_FREQ_RE.captures(line) {
        let freq_mhz: i32 = caps[2].parse().unwrap_or(0);
        gpu_metrics.freq_mhz = freq_mhz;
    }
}

fn parse_netdisk_metrics(line: &str, netdisk_metrics: &mut NetDiskMetrics) {
    if let Some(caps) = OUT_REGEX.captures(line) {
        netdisk_metrics.out_packets_per_sec = caps[1].parse().unwrap_or(0.0);
        netdisk_metrics.out_bytes_per_sec = caps[2].parse().unwrap_or(0.0);
    }

    if let Some(caps) = IN_REGEX.captures(line) {
        netdisk_metrics.in_packets_per_sec = caps[1].parse().unwrap_or(0.0);
        netdisk_metrics.in_bytes_per_sec = caps[2].parse().unwrap_or(0.0);
    }

    if let Some(caps) = READ_REGEX.captures(line) {
        netdisk_metrics.read_ops_per_sec = caps[1].parse().unwrap_or(0.0);
        netdisk_metrics.read_kbytes_per_sec = caps[2].parse().unwrap_or(0.0);
    }

    if let Some(caps) = WRITE_REGEX.captures(line) {
        netdisk_metrics.write_ops_per_sec = caps[1].parse().unwrap_or(0.0);
        netdisk_metrics.write_kbytes_per_sec = caps[2].parse().unwrap_or(0.0);
    }
}

fn get_memory_metrics() -> MemoryMetrics {
    unsafe {
        let mut vm_info: vm_statistics64_data_t = std::mem::zeroed();
        let mut count = std::mem::size_of::<vm_statistics64_data_t>() as mach_msg_type_number_t
            / std::mem::size_of::<natural_t>() as mach_msg_type_number_t;

        let result = host_statistics64(
            mach_host_self(),
            HOST_VM_INFO64,
            &mut vm_info as *mut _ as host_info64_t,
            &mut count,
        );

        if result != 0 {
            return MemoryMetrics {
                total: 0,
                used: 0,
                available: 0,
                free: 0,
                swap_total: 0,
                swap_used: 0,
                used_percent: 0.0,
            };
        }

        let page_size = libc::sysconf(libc::_SC_PAGESIZE) as u64;

        let free = vm_info.free_count as u64 * page_size;
        let active = vm_info.active_count as u64 * page_size;
        let inactive = vm_info.inactive_count as u64 * page_size;
        let wired = vm_info.wire_count as u64 * page_size;
        let compressed = vm_info.compressor_page_count as u64 * page_size;

        let total = match get_total_memory() {
            Ok(val) => val,
            Err(_) => {
                return MemoryMetrics {
                    total: 0,
                    used: 0,
                    available: 0,
                    free: 0,
                    swap_total: 0,
                    swap_used: 0,
                    used_percent: 0.0,
                }
            }
        };

        let used = active + wired + compressed;
        let available = free + inactive;

        let (swap_total, swap_used, _) = match get_swap_memory() {
            Ok((t, u, f)) => (t, u, f),
            Err(_) => (0, 0, 0),
        };

        let total_with_swap = total + swap_total;
        let used_with_swap = used + swap_used;

        let used_percent = if total_with_swap > 0 {
            (used_with_swap as f64 / total_with_swap as f64) * 100.0
        } else {
            0.0
        };

        MemoryMetrics {
            total: total_with_swap,
            used: used_with_swap,
            available,
            free,
            swap_total,
            swap_used,
            used_percent: used_percent as f32,
        }
    }
}

fn get_swap_memory() -> Result<(u64, u64, u64), std::io::Error> {
    let output = Command::new("sysctl")
        .arg("vm.swapusage")
        .output()?;
    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        if let Some(caps) = SWAP_REGEX.captures(&output_str) {
            let total = parse_size(&caps[1], &caps[2]);
            let used = parse_size(&caps[3], &caps[4]);
            let free = parse_size(&caps[5], &caps[6]);
            return Ok((total, used, free));
        } else {
            eprintln!("Failed to parse swap usage: {}", output_str);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Failed to get swap memory",
    ))
}

fn parse_size(size_str: &str, unit: &str) -> u64 {
    let size: f64 = size_str.parse().unwrap_or(0.0);
    match unit {
        "G" => (size * 1024.0 * 1024.0 * 1024.0) as u64,
        "M" => (size * 1024.0 * 1024.0) as u64,
        _ => 0,
    }
}

fn get_total_memory() -> Result<u64, std::io::Error> {
    let mut size: u64 = 0;
    let mut size_len = std::mem::size_of::<u64>();
    let mib = [libc::CTL_HW, libc::HW_MEMSIZE];
    let ret = unsafe {
        libc::sysctl(
            mib.as_ptr() as *mut c_int,
            mib.len() as libc::c_uint,
            &mut size as *mut u64 as *mut libc::c_void,
            &mut size_len as *mut usize,
            std::ptr::null_mut(),
            0,
        )
    };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(size)
}

#[derive(Clone)]
struct AppleSiliconInfo {
    name: String,
    e_core_count: i32,
    p_core_count: i32,
    gpu_core_count: String,
}

fn get_apple_silicon_info() -> AppleSiliconInfo {
    let model_name = get_sysctl_string("machdep.cpu.brand_string")
        .unwrap_or_else(|_| "Unknown".to_string());

    let e_core_count = get_sysctl_int("hw.perflevel1.logicalcpu").unwrap_or(0);
    let p_core_count = get_sysctl_int("hw.perflevel0.logicalcpu").unwrap_or(0);

    let gpu_core_count = get_gpu_core_count().unwrap_or_else(|_| "?".to_string());

    AppleSiliconInfo {
        name: model_name,
        e_core_count,
        p_core_count,
        gpu_core_count,
    }
}

fn get_sysctl_string(name: &str) -> Result<String, std::io::Error> {
    let output = Command::new("sysctl").arg(name).output()?;
    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = output_str.trim().split(": ").collect();
        if parts.len() > 1 {
            return Ok(parts[1].to_string());
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Failed to get sysctl string",
    ))
}

fn get_sysctl_int(name: &str) -> Result<i32, std::io::Error> {
    let output = Command::new("sysctl").arg(name).output()?;
    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = output_str.trim().split(": ").collect();
        if parts.len() > 1 {
            return parts[1]
                .trim()
                .parse::<i32>()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Parse error: {}", e)));
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Failed to get sysctl int",
    ))
}

fn get_gpu_core_count() -> Result<String, std::io::Error> {
    let output = Command::new("system_profiler")
        .args(&["-detailLevel", "basic", "SPDisplaysDataType"])
        .output()?;
    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.contains("Total Number of Cores") {
                let parts: Vec<&str> = line.split(": ").collect();
                if parts.len() > 1 {
                    return Ok(parts[1].trim().to_string());
                }
            }
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Failed to get GPU core count",
    ))
}