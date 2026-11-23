# Kerf

An IDE with a small Rust core and sandboxed WASM plugins.

The core draws the editor, holds the text buffer, runs the event loop and keeps a semantic index of the code (tree-sitter plus an incremental index). Languages and debugging come in through LSP and DAP: separate processes the core starts on a plugin's request. Plugins declare their permissions in a manifest and cannot spawn processes themselves.

The Cargo workspace under `crates/` holds that core, the plugin host, the C ABI (`kerf-ffi`), a Linux GTK host (`kerf-gtk`) and supporting crates. The macOS host is a Swift package in `macos/` that links `kerf-ffi`. Status: under development; this is not a released app.

## Quick start

The Rust toolchain is pinned in `mise.toml`. Run `mise trust` once in a fresh checkout, then:

```bash
mise install
cargo test --locked --all-targets
cargo build --locked -p kerf-ffi
```

On macOS, after `kerf-ffi` is built:

```bash
cd macos
swift build --arch arm64
swift test --arch arm64
```

These are the commands CI runs (`.github/workflows/ci.yml`).

## Documentation

- [docs/](docs/README.md) — documentation index
- [architecture.md](docs/architecture.md) — core boundary, plugin lifecycle, permissions, Workspace Trust
- [research.md](research.md) — how other editors and IDEs are built, and what Kerf takes from each
- [ideas.md](ideas.md) — candidate tasks
- [plan/](plan/README.md) — tasks, one folder per task
- [roadmap.md](roadmap.md) — approved later work

## License

You can use this project under **either** of the following licenses, at your choice:

1. [GNU GPLv3](LICENSE): free for open source applications on any platform, including embedded systems.
2. [Commercial license](PRICING.md): for proprietary applications, including embedded systems.
