# _mtop_

[![Rust](https://img.shields.io/badge/Rust-%23000000.svg?e&logo=rust&logoColor=white)](#) &ensp; ![GitHub Release](https://img.shields.io/github/v/release/jepeake/mtop)

_**mtop** is a terminal-based performance monitor for Apple Silicon._

_It provides an efficient way to monitor utilisation & power consumption of the CPU (P & E-Cores), GPU, ANE, Memory - and other system metrics straight from the terminal._

_Written in ~1000 Lines of Rust._

![image](https://github.com/user-attachments/assets/623a9955-37a7-40ba-970a-48815f76e3d9)

## _Features_

- _CPU (P & E-Core), GPU, ANE, & Memory Utilisation_
- _Power Information_
- _Network & Disk Information_
- _Apple Silicon Info_
- _Intuituve UI_
- _High Performance_

## _Installation_

_Install using [Homebrew](https://brew.sh):_

`brew tap jepeake/mtop && brew install mtop`

_Run:_

`sudo mtop`

## _Contributions_

_Any contributions to this open-source project are greatly appreciated._

_The issues section is a good place to start for things to contribute._

_To contribute:_

- _Fork mtop._
- _Create a branch for your feature - i.e. `feature/power-util`_
- _Implement the features & commit your changes._
- _Push to the branch: `git push origin feature/power-util`_
- _Open a Pull Request to mtop._

_Thank you!_

## _Acknowledgements_

- _[asitop](https://github.com/tlkh/asitop) / [mactop](https://github.com/context-labs/mactop) / [nvitop](https://github.com/XuehaiPan/nvitop) for inspiration_
- _[tui-rs](https://github.com/fdehau/tui-rs) / [crossterm](https://github.com/crossterm-rs/crossterm) for the terminal user interface_
- _[rust-psutil](https://github.com/rust-psutil/rust-psutil) for process & system memory monitoring_
- _[sysinfo](https://github.com/GuillaumeGomez/sysinfo) for system information_
- _[regex](https://github.com/rust-lang/regex) for regular experessions_
- _[lazy_static](https://github.com/rust-lang-nursery/lazy-static.rs) for lazy-evaluated static variables_
- _[crossbeam](https://github.com/crossbeam-rs/crossbeam) for concurrent programming_
- _[libc](https://github.com/rust-lang/libc)_
