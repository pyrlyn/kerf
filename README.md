# Kerf

An IDE with a small Rust core and sandboxed WASM plugins.

The core draws the editor, holds the text buffer, runs the event loop and keeps a semantic index of the code (tree-sitter plus an incremental index). Languages and debugging come in through LSP and DAP: separate processes the core starts on a plugin's request. Plugins declare their permissions in a manifest and cannot spawn processes themselves.

Status: research only, no code yet.

- [research.md](research.md) — how Zed, VS Code, IntelliJ, Visual Studio, Lapce, Neovim, Lite XL, Pragtical, Zellij and others work, and what Kerf takes from each.
- [ideas.md](ideas.md) — candidate tasks.
- [plan.md](plan.md) — active tasks.

## License

You can use this project under **any** of the following licenses, at your choice:

1. [GNU GPLv3](LICENSE): free for open source applications on any platform, including embedded systems.
2. [Royalty-free License](LICENSE-ROYALTY-FREE.md): free for proprietary desktop, mobile, and web applications, as long as you disclose that your application uses this project. Embedded systems are not covered.
3. [Commercial license](PRICING.md): for proprietary applications, including embedded systems, without the attribution requirement.
