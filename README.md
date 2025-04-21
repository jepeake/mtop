# mtop

[![Rust](https://img.shields.io/badge/Rust-%23000000.svg?e&logo=rust&logoColor=white)](#) &ensp; ![GitHub Release](https://img.shields.io/github/v/release/jepeake/mtop)

**mtop** is a terminal-based performance monitor for apple silicon.

![image](https://github.com/user-attachments/assets/b3dfbcf2-c43e-4cac-adf9-90fea1113fcd)

## Installation

Install using [Homebrew](https://brew.sh):

`brew tap jepeake/mtop && brew install mtop`

Run:

`sudo mtop`

## Acknowledgements

- [asitop](https://github.com/tlkh/asitop) / [mactop](https://github.com/context-labs/mactop) / [nvitop](https://github.com/XuehaiPan/nvitop) for inspiration
- [tui-rs](https://github.com/fdehau/tui-rs) / [crossterm](https://github.com/crossterm-rs/crossterm) for the terminal user interface
- [rust-psutil](https://github.com/rust-psutil/rust-psutil) for process & system memory monitoring
- [sysinfo](https://github.com/GuillaumeGomez/sysinfo) for system information
- [regex](https://github.com/rust-lang/regex) for regular experessions
- [lazy_static](https://github.com/rust-lang-nursery/lazy-static.rs) for lazy-evaluated static variables
- [crossbeam](https://github.com/crossbeam-rs/crossbeam) for concurrent programming
- [libc](https://github.com/rust-lang/libc)
