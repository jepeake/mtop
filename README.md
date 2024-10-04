# _mtop_

_**mtop** is a terminal-based performance monitor for Apple Silicon._

_It provides a powerful & efficient way to monitor utilisation of CPU (P & E-Cores), GPU, ANE, Memory - and other system metrics._

_Written in < 1k Lines of Rust._

![image](https://github.com/user-attachments/assets/b46233df-f051-46ce-8d45-8f23f293f83d)


## _Features_

- _CPU (P & E-Core), GPU, ANE, & Memory Utilisation_
- _Network & Disk Information_
- _Power Information_
- _Intuituve UI_
- _High Performance_

## _Installation_

1. _Install Rust (if not already installed):_

```curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh```

2. _Clone the **mtop** repo:_

```git clone https://github.com/jepeake/mtop && cd mtop```

3. _Build & Run the application using cargo:_

```sudo cargo build && sudo cargo run```



## _Acknowledgements_

- _[asitop](https://github.com/tlkh/asitop) / [mactop](https://github.com/context-labs/mactop) / [nvitop](https://github.com/XuehaiPan/nvitop) for inspiration_
- _[tui-rs](https://github.com/fdehau/tui-rs) / [crossterm](https://github.com/crossterm-rs/crossterm) for the terminal user interface_
- _[rust-psutil](https://github.com/rust-psutil/rust-psutil) for process & system memory monitoring_
- _[sysinfo](https://github.com/GuillaumeGomez/sysinfo) for system information_
- _[regex](https://github.com/rust-lang/regex) for regular experessions_
- _[lazy_static](https://github.com/rust-lang-nursery/lazy-static.rs) for lazy-evaluated static variables_
- _[crossbeam](https://github.com/crossbeam-rs/crossbeam) for concurrent programming_
- _[libc](https://github.com/rust-lang/libc)_
