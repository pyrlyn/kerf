# Kerf

https://github.com/pyrlyn/kerf

An IDE with a small Rust core and sandboxed WASM plugins. Language servers and debug adapters run as separate processes the core starts. See `research.md`.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T6 | todo | P1 | 4 | 0% | |
| T8 | todo | P2 | 5 | 0% | |
| T12 | todo | P2 | 5 | 0% | |
| T13 | todo | P3 | 5 | 0% | |
| T15 | todo | P3 | 5 | 0% | |

### T6. Plugin host on `wasm-plugin-host`

`crates/kerf-plugin-host`: load, activate and call WASM plugins through the shared `wasm-plugin-host` crate on cox's ABI (`cox:host/v1`, extism), not a third copy of cox's host. Manifest read without running code, lazy activation, a plugin crash disables the plugin and never the IDE (`docs/architecture.md`). The package layout depends on crates-packages#42. Done when a test plugin loads, activates on its trigger, answers a call, and a trapping plugin is disabled while the host keeps running.

### T8. LSP results in the semantic layer

Merge language-server answers into `kerf-semantic` where tree-sitter misses (macro bodies, type-resolved references), tagged by source so tree-sitter results stay the base. Add at least one more tree-sitter language. Done when the recall test on the labelled fixture improves with a scripted language server and a second language has its own fixture.

### T12. UI shell

The first window. Pick Slint through slint-bindings or Flutter through slint_dart against each other, and record why in `research.md`. Rendering is a trait `kerf-core` defines (`docs/architecture.md` §2.1); the shell implements it in its own crate. Plugin UI and redraw-on-request come in later tasks. Done when a `kerf` binary opens a file, shows it, takes typing, undo and redo through `kerf-core`, and a headless UI test covers that path.

### T13. Remote development

The plugin host and the language servers run next to the code; the UI stays local. First slice: a versioned UI ↔ host protocol and a headless host process that owns buffers and the semantic layer, over any byte stream (stdio, SSH). Every message and queue from the remote side is capped. Done when tests drive open, edit, undo and a symbol query through an in-memory transport and the protocol refuses an unknown major version.

### T15. Mobile plugin backend

An interpreter backend (wasm3, WAMR or a maintained pure-Rust interpreter, chosen against each other) behind the same host API, for iOS and Android where wasmtime cannot JIT. Depends on T6. Done when the T6 plugin tests pass unchanged against the interpreter backend on the desktop host, and the choice and its limits are recorded in `research.md`.
