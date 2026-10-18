# Kerf

https://github.com/pyrlyn/kerf

An IDE with a small Rust core and sandboxed WASM plugins. Language servers and debug adapters run as separate processes the core starts. See `research.md`.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T4.1 | in progress | P1 | 3 | 10% | MiMo Desktop / mimo |
| T4.2 | in progress | P1 | 3 | 10% | MiMo Desktop / mimo |
| T6 | todo | P1 | 4 | 0% | |
| T8 | todo | P2 | 5 | 0% | |
| T11 | todo | P3 | 3 | 0% | |
| T12 | todo | P2 | 5 | 0% | |
| T14 | todo | P3 | 4 | 0% | |
| T15 | todo | P3 | 5 | 0% | |

### T4. IDE permissions in `cox-plugin-api`

Split when claimed (500-line task cap). Add the IDE permission set and the manifest tables for language servers and debug adapters, as T1 defines them, to cox's `cox-plugin-api`, its JSON Schemas and its docs. The plugin declares a process; the host spawns it, as with `[[mcp]]` today.

The work happens in the cox repository and gets a matching task in cox's `plan.md` when it starts. Blocked on T1 only for the final names; the permission families are already in `research.md` (buffer, LSP, DAP, index, terminal, git) and can be implemented against that list.

#### T4.1. IDE permission set

The buffer, LSP, DAP, index, terminal and git permission classes in `cox-plugin-api`, with validation and schema.

#### T4.2. LSP and DAP manifest tables

`[[language_servers]]` and `[[debug_adapters]]` (names as T1 settles them) in `PluginManifest`, JSON Schema, and the docs. The plugin declares the command; the host spawns it.

### T6. Plugin host on `wasm-plugin-host`

`crates/kerf-plugin-host`: load, activate and call WASM plugins through the shared `wasm-plugin-host` crate on cox's ABI (`cox:host/v1`, extism), not a third copy of cox's host. Manifest read without running code, lazy activation, a plugin crash disables the plugin and never the IDE (`docs/architecture.md`). The package layout depends on crates-packages#42. Done when a test plugin loads, activates on its trigger, answers a call, and a trapping plugin is disabled while the host keeps running.

### T8. LSP results in the semantic layer

Merge language-server answers into `kerf-semantic` where tree-sitter misses (macro bodies, type-resolved references), tagged by source so tree-sitter results stay the base. Add at least one more tree-sitter language. Done when the recall test on the labelled fixture improves with a scripted language server and a second language has its own fixture.

### T11. Language-server and debug-adapter binaries through ketch

Install and update language-server and debug-adapter binaries from GitHub releases with checksums, through ketch. Done when a test installs a pinned binary from a local fixture release, rejects a bad checksum, and the core resolves the installed path.

### T12. UI shell

The first window. Pick Slint through slint-bindings or Flutter through slint_dart against each other, and record why in `research.md`. Rendering is a trait `kerf-core` defines (`docs/architecture.md` §2.1); the shell implements it in its own crate. Plugin UI and redraw-on-request come in later tasks. Done when a `kerf` binary opens a file, shows it, takes typing, undo and redo through `kerf-core`, and a headless UI test covers that path.

### T14. Local code completion

A fill-in-the-middle endpoint in runa, used by the editor. Chat and agents go through `cox acp`, not here. The runa side is a matching task in runa's `plan.md`; the Kerf side is a completion client with a timeout, a size cap and cancellation on the next edit. Done when Kerf's client passes tests against a fake endpoint and runa's endpoint has its own tests.

### T15. Mobile plugin backend

An interpreter backend (wasm3, WAMR or a maintained pure-Rust interpreter, chosen against each other) behind the same host API, for iOS and Android where wasmtime cannot JIT. Depends on T6. Done when the T6 plugin tests pass unchanged against the interpreter backend on the desktop host, and the choice and its limits are recorded in `research.md`.
