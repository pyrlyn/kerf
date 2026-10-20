# Toolchain

## Programs

| Program | How to install | Why here | Source |
| --- | --- | --- | --- |
| rustc, cargo, rustfmt, clippy | mise (`mise.toml`, 1.99.0) | Build, format, lint | https://github.com/rust-lang/rust |
| mise | system | Pins the Rust toolchain | https://github.com/jdx/mise |
| ketch | mise (`mise.toml`, `github:pyrlyn/ketch`) | `kerf-binaries` installs language servers and debug adapters through its CLI; the crate's tests run it | https://github.com/pyrlyn/ketch |
| Xcode 27 (Swift 6.4, SwiftPM, `swift-format`) | system (Mac App Store); CI: the `xcode-27` image, selected by pyrlyn/ci `setup-xcode` | Builds, lints and tests the macOS host in `macos/` | https://developer.apple.com/xcode/ |
| Mesa lavapipe (`mesa-vulkan-drivers`) | apt, CI Linux runners only | Software Vulkan adapter for the `kerf-render` offscreen test | https://gitlab.freedesktop.org/mesa/mesa |
| fontconfig headers (`libfontconfig1-dev`) | apt, CI Linux runners only | `kerf-slint`: Slint's text stack links fontconfig to find fonts on Linux | https://gitlab.freedesktop.org/fontconfig/fontconfig |
| GTK 4 headers (`libgtk-4-dev`) | apt (Ubuntu 24.04: GTK 4.14); CI Linux runners and the Lima test VM | `kerf-gtk`: the Linux host links GTK 4 | https://gitlab.gnome.org/GNOME/gtk |
| Xvfb (`xvfb`) | apt, CI Linux runners and the Lima test VM | A virtual X display for the `kerf-gtk` host test | https://gitlab.freedesktop.org/xorg/xserver |
| Lima | mise (local, macOS only) | Runs the Linux host test on a Mac in an Ubuntu 24.04 VM (`limactl start template:ubuntu-24.04`) | https://github.com/lima-vm/lima |

## Crates

| Crate | How to install | Why here | Source |
| --- | --- | --- | --- |
| `ropey` | Cargo (workspace dep) | `kerf-core` text buffer: most used editor rope (Helix), stable, with char/line/UTF-16 metrics | https://crates.io/crates/ropey |
| `thiserror` | Cargo (workspace dep) | Typed errors in library crates | https://crates.io/crates/thiserror |
| `tree-sitter` | Cargo (workspace dep) | `kerf-semantic`: incremental re-parse from the old tree, and the query engine for tags | https://crates.io/crates/tree-sitter |
| `tree-sitter-rust` | Cargo (workspace dep) | `kerf-semantic`: Rust grammar and its tags query | https://crates.io/crates/tree-sitter-rust |
| `tree-sitter-python` | Cargo (workspace dep) | `kerf-semantic`: Python grammar and its tags query, the second language | https://crates.io/crates/tree-sitter-python |
| `gen-lsp-types` | Cargo (workspace dep) | LSP types for answers merged into `kerf-semantic`, generated from the upstream metamodel; `lsp-types` has had no release since 2024-06 | https://crates.io/crates/gen-lsp-types |
| `wasm-plugin-host` | Cargo (workspace dep, git pin by `rev`) | `kerf-plugin`: the shared extism engine (worker per plugin, lanes, deadlines, memory cap, package layout); brings `extism` and `wasmtime`. Not on crates.io, where `wasm_plugin_host` is an unrelated crate | https://github.com/pyrlyn/crates/tree/main/rust/wasm-plugin-host |
| `cox-plugin-api` | Cargo (workspace dep, git pin by `rev`) | `kerf-plugin`: cox's `plugin.toml` manifest and ABI payloads, taken as is. Not on crates.io | https://github.com/pyrlyn/cox/tree/main/crates/cox-plugin-api |
| `fs-kit` | Cargo (workspace dep) | `kerf-core` trust file: canonical paths without the Windows `\\?\` prefix, case-folded prefix checks, atomic writes; `kerf-plugin-lock` and `kerf-binaries`: atomic lockfile writes | https://crates.io/crates/fs-kit |
| `dirs` | Cargo (workspace dep) | `kerf-core`: the OS config directory for the trust file | https://crates.io/crates/dirs |
| `serde` | Cargo (workspace dep) | `kerf-core`: typed trust file; `kerf-inspect`: the inspection export's JSON payloads; `kerf-plugin-lock`: (de)serialization of the plugin lockfile; `kerf-binaries`: the ketch lockfile it hands to `ketch sync`; `kerf-plugin`: JSON in and out of every plugin call | https://crates.io/crates/serde |
| `toml` | Cargo (workspace dep) | `kerf-core`: trust file format; `kerf-plugin-lock`: reads and writes the plugin lockfile; `kerf-binaries`: renders the ketch lockfile; `kerf-plugin`: reads `plugin.toml` into cox's `PluginManifest` | https://crates.io/crates/toml |
| `schemars` | Cargo (workspace dev dep) | `kerf-core`: JSON Schema of the trust file, checked by a test; `kerf-plugin-lock` tests: the lockfile's JSON Schema from its types | https://crates.io/crates/schemars |
| `config-schema` | Cargo (workspace dev dep) | `kerf-plugin-lock` tests: fails when the committed schema is stale; shared crate from pyrlyn/crates | https://crates.io/crates/config-schema |
| `serde_json` | Cargo (workspace dep) | `kerf-inspect`: JSON encoding of the inspection payloads, as cox's ABI uses through extism; `kerf-lsp`: LSP messages and the scripted server's answer files; `kerf-remote`: protocol params and results; `kerf-core` (dev): writes the generated schema; `kerf-plugin`: plugin call payloads | https://crates.io/crates/serde_json |
| `tempfile` | Cargo (workspace dev dep) | `kerf-core` Workspace Trust tests: throwaway folder trees removed on drop; `kerf-plugin-lock` and `kerf-binaries` tests: per-test temporary directories; `kerf-remote` tests: workspace roots; `kerf-plugin` tests: per-test plugin roots; `kerf-ffi` and `kerf-gtk` tests: the file a host opens | https://crates.io/crates/tempfile |
| `walkdir` | Cargo (workspace dep) | `kerf-binaries`: installed-size audit without following symlinks | https://crates.io/crates/walkdir |
| `sha2` | Cargo (workspace dev dep) | `kerf-binaries` tests: fixture asset digests | https://crates.io/crates/sha2 |
| `hex` | Cargo (workspace dev dep) | `kerf-binaries` tests: hex form of fixture digests | https://crates.io/crates/hex |
| `tokio-util` | Cargo (workspace dep, `codec`) | `kerf-remote`: `LinesCodec` frames the UI <-> host wire with a maximum line length, synchronously on a `BytesMut` | https://crates.io/crates/tokio-util |
| `jsonrpsee-types` | Cargo (workspace dep) | `kerf-remote`: JSON-RPC 2.0 request, response and error envelopes | https://crates.io/crates/jsonrpsee-types |
| `wgpu` | Cargo (workspace dep) | `kerf-render`: one GPU API over Metal, D3D12 and Vulkan, and a surface from the host's native view | https://crates.io/crates/wgpu |
| `glyphon` | Cargo (workspace dep) | `kerf-render`: draws `cosmic-text` layouts on wgpu through a glyph atlas; its re-exported `cosmic-text` shapes the text | https://crates.io/crates/glyphon |
| `pollster` | Cargo (workspace dep) | `kerf-render` tests: blocks on wgpu's async adapter and device requests for the offscreen test | https://crates.io/crates/pollster |
| `slint-bindings-core` | Cargo (workspace dep, git pin by `rev`) | `kerf-slint`: host-driven Slint embedding, a platform with no event loop that renders a component into a buffer the host owns; `publish = false`, so not on crates.io | https://github.com/pyrlyn/slint-bindings/tree/main/crates/slint-bindings-core |
| `slint` | Cargo (workspace dep, `=1.18.1`, software renderer only) | `kerf-slint`: the editor component's runtime; the exact pin is the one `slint-bindings-core` forces | https://crates.io/crates/slint |
| `slint-build` | Cargo (workspace build dep, `=1.18.1`) | `kerf-slint`: compiles `ui/editor.slint` to Rust at build time, in lockstep with `slint` | https://crates.io/crates/slint-build |
| `gtk4` | Cargo (workspace dep as `gtk`, feature `v4_14`, Linux only) | `kerf-gtk`: the Linux host's window, widgets, input methods and textures through gtk-rs's bindings | https://crates.io/crates/gtk4 |
| `cbindgen` | Cargo (workspace dev dep, default features off) | `kerf-ffi` tests: generate `include/kerf.h` from the Rust declarations and fail when the committed header is stale | https://crates.io/crates/cbindgen |
| `ureq` | Cargo (workspace dep) | `kerf-complete`: blocking HTTP for fill-in-the-middle requests on a worker thread; whole-call timeout and body limit built in, no async runtime; gzip off so the limit caps decoded bytes | https://crates.io/crates/ureq |
| `httpmock` | Cargo (workspace dev dep) | `kerf-complete` tests: a fake `/v1/infill` with per-mock delays, bodies and call counts (timeout, size cap, cancellation) | https://crates.io/crates/httpmock |

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

