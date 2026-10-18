# Toolchain

## Programs

| Program | How to install | Why here | Source |
| --- | --- | --- | --- |
| rustc, cargo, rustfmt, clippy | mise (`mise.toml`, 1.99.0) | Build, format, lint | https://github.com/rust-lang/rust |
| mise | system | Pins the Rust toolchain | https://github.com/jdx/mise |
| Mesa lavapipe (`mesa-vulkan-drivers`) | apt, CI Linux runners only | Software Vulkan adapter for the `kerf-render` offscreen test | https://gitlab.freedesktop.org/mesa/mesa |

## Crates

| Crate | How to install | Why here | Source |
| --- | --- | --- | --- |
| `ropey` | Cargo (workspace dep) | `kerf-core` text buffer: most used editor rope (Helix), stable, with char/line/UTF-16 metrics | https://crates.io/crates/ropey |
| `thiserror` | Cargo (workspace dep) | Typed errors in library crates | https://crates.io/crates/thiserror |
| `tree-sitter` | Cargo (workspace dep) | `kerf-semantic`: incremental re-parse from the old tree, and the query engine for tags | https://crates.io/crates/tree-sitter |
| `tree-sitter-rust` | Cargo (workspace dep) | `kerf-semantic`: Rust grammar and its tags query | https://crates.io/crates/tree-sitter-rust |
| `fs-kit` | Cargo (workspace dep) | `kerf-core` trust file: canonical paths without the Windows `\\?\` prefix, case-folded prefix checks, atomic writes; `kerf-plugin-lock`: atomic lockfile writes | https://crates.io/crates/fs-kit |
| `dirs` | Cargo (workspace dep) | `kerf-core`: the OS config directory for the trust file | https://crates.io/crates/dirs |
| `serde` | Cargo (workspace dep) | `kerf-core`: typed trust file; `kerf-inspect`: the inspection export's JSON payloads; `kerf-plugin-lock`: (de)serialization of the plugin lockfile | https://crates.io/crates/serde |
| `toml` | Cargo (workspace dep) | `kerf-core`: trust file format; `kerf-plugin-lock`: reads and writes the plugin lockfile | https://crates.io/crates/toml |
| `schemars` | Cargo (workspace dev dep) | `kerf-core`: JSON Schema of the trust file, checked by a test; `kerf-plugin-lock` tests: the lockfile's JSON Schema from its types | https://crates.io/crates/schemars |
| `config-schema` | Cargo (workspace dev dep) | `kerf-plugin-lock` tests: fails when the committed schema is stale; shared crate from pyrlyn/crates | https://crates.io/crates/config-schema |
| `serde_json` | Cargo (workspace dep) | `kerf-inspect`: JSON encoding of the inspection payloads, as cox's ABI uses through extism; `kerf-core` (dev): writes the generated schema | https://crates.io/crates/serde_json |
| `tempfile` | Cargo (workspace dev dep) | `kerf-core` Workspace Trust tests: throwaway folder trees removed on drop; `kerf-plugin-lock` tests: per-test temporary directories | https://crates.io/crates/tempfile |
| `wgpu` | Cargo (workspace dep) | `kerf-render`: one GPU API over Metal, D3D12 and Vulkan, and a surface from the host's native view | https://crates.io/crates/wgpu |
| `glyphon` | Cargo (workspace dep) | `kerf-render`: draws `cosmic-text` layouts on wgpu through a glyph atlas; its re-exported `cosmic-text` shapes the text | https://crates.io/crates/glyphon |
| `pollster` | Cargo (workspace dev dep) | `kerf-render` tests: block on wgpu's async adapter and device requests | https://crates.io/crates/pollster |

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

