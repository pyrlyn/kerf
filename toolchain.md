# Toolchain

## Programs

| Program | How to install | Why here | Source |
| --- | --- | --- | --- |
| rustc, cargo, rustfmt, clippy | mise (`mise.toml`, 1.99.0) | Build, format, lint | https://github.com/rust-lang/rust |
| mise | system | Pins the Rust toolchain | https://github.com/jdx/mise |

## Linkers

- **Windows (MSVC):** `rust-lld` through `.cargo/config.toml` (`[target.*-pc-windows-msvc] linker`). It ships with rustup, nothing to install.
- **Linux:** `mold`, pinned in `mise.toml` for Linux only and driven by `clang`. mise `[env]` sets `CARGO_TARGET_{X86_64,AARCH64}_UNKNOWN_LINUX_GNU_RUSTFLAGS` to `-C linker=clang -C link-arg=-fuse-ld=mold` only when mise has mold installed (`mise install`); otherwise the variable is empty and the default linker stays. A Linux machine without mold (or without mise) builds as before; to use mold, run `mise install` and make sure `clang` is on PATH (`apt install clang`). `RUSTFLAGS`, when set, overrides these flags. CI installs mold on Linux runners.
- **macOS:** Apple's native linker (mold has no macOS support).
- **Cranelift:** not used. It needs a nightly toolchain and this repository is on stable.
