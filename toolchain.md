# Toolchain

## Programs

| Program | How to install | Why here | Source |
| --- | --- | --- | --- |
| rustc, cargo, rustfmt, clippy | mise (`mise.toml`, 1.99.0) | Build, format, lint | https://github.com/rust-lang/rust |
| mise | system | Pins the Rust toolchain | https://github.com/jdx/mise |

## Crates

| Crate | How to install | Why here | Source |
| --- | --- | --- | --- |
| `ropey` | Cargo (workspace dep) | `kerf-core` text buffer: most used editor rope (Helix), stable, with char/line/UTF-16 metrics | https://crates.io/crates/ropey |
| `thiserror` | Cargo (workspace dep) | Typed errors in library crates | https://crates.io/crates/thiserror |

## Linkers

- **Windows (MSVC):** `rust-lld` through `.cargo/config.toml` (`[target.*-pc-windows-msvc] linker`). It ships with rustup, nothing to install.
- **Linux:** `mold`, pinned in `mise.toml` for Linux only and driven by `clang`. mise `[env]` sets `CARGO_TARGET_{X86_64,AARCH64}_UNKNOWN_LINUX_GNU_RUSTFLAGS` to `-C linker=clang -C link-arg=-fuse-ld=mold` only when mise has mold installed (`mise install`); otherwise the variable is empty and the default linker stays. A Linux machine without mold (or without mise) builds as before; to use mold, run `mise install` and make sure `clang` is on PATH (`apt install clang`). `RUSTFLAGS`, when set, overrides these flags. CI installs mold on Linux runners.
- **macOS:** Apple's native linker (mold has no macOS support).
- **Cranelift:** not used. It needs a nightly toolchain and this repository is on stable.

## cargo

External crates only; workspace crates such as `kerf-core` are not listed. Rows
are added by the task that wires each crate in — T2.1 adds the rope crate once
it picks one against the alternatives (`plan.md` T2.1).

| Package | Where | Source | Why here |
| --- | --- | --- | --- |

