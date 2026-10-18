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
| T12.2 | todo | P2 | 4 | 0% | |
| T12.3 | todo | P2 | 3 | 0% | |
| T12.4 | todo | P2 | 4 | 0% | |
| T13 | todo | P3 | 5 | 0% | |
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

Split when claimed (500-line task cap). The first window. The creator's direction (`research.md` §6): native controls in a native host per platform, talking to the Rust core through a C ABI, and Kerf's own GPU renderer for the editor surface inside a native view. The editor looks native: like Xcode's on macOS and like Visual Studio's on Windows (`research.md` §6, "Look"); the host supplies the font, colours and caret, the renderer hard-codes none. macOS first; Windows and Linux hosts are in `roadmap.md`. Rendering is a trait `kerf-core` defines (`docs/architecture.md` §2.1). Plugin UI and redraw-on-request come in later tasks. The subtasks together are done when the macOS app opens a file, shows it in the GPU-drawn editor view inside a native AppKit window, takes typing, undo and redo through `kerf-core`, and headless tests cover that path.

#### T12.2. GPU editor renderer

A crate that implements `kerf_core::Render` on `wgpu` + `cosmic-text` + `glyphon` (`research.md` §6): the buffer text, a surface size and scale factor, and a `Theme` the host passes in (font family and size, background, text and selection colours, syntax colours). It reports the caret rectangle so a host can draw the system caret; it draws its own only when the host asks. Done when a test lays out a frame and checks the caret position without a GPU, and a test renders a frame offscreen and reads pixels back on the adapter the CI runner offers (or says in the card why a runner cannot).

#### T12.3. C ABI for native hosts

A `cdylib`/`staticlib` crate with a generated C header: open a file into an `Editor`, attach the renderer to a host view (an `NSView` on macOS), resize, forward text input, keys, undo and redo, render. Every pointer and length from the host is checked; a panic never crosses the boundary. Done when a Rust end-to-end test drives the whole path through the C functions only.

#### T12.4. macOS host

A SwiftPM app in the repository: an AppKit window, the Edit menu (Undo, Redo) and an `NSView` that adopts `NSTextInputClient` and hosts the renderer through the C ABI. It looks like Xcode's editor: SF Mono, the user's Xcode theme when Xcode is installed (Kerf's own otherwise), system selection colour, `NSTextInsertionIndicator` as the caret, Light/Dark with the system. Done when `swift build` and `swift test` pass in CI on macOS arm64 and a test drives open, type, undo and redo through the view.

### T13. Remote development

The plugin host and the language servers run next to the code; the UI stays local. First slice: a versioned UI ↔ host protocol and a headless host process that owns buffers and the semantic layer, over any byte stream (stdio, SSH). Every message and queue from the remote side is capped. Done when tests drive open, edit, undo and a symbol query through an in-memory transport and the protocol refuses an unknown major version.

### T14. Local code completion

A fill-in-the-middle endpoint in runa, used by the editor. Chat and agents go through `cox acp`, not here. The runa side is a matching task in runa's `plan.md`; the Kerf side is a completion client with a timeout, a size cap and cancellation on the next edit. Done when Kerf's client passes tests against a fake endpoint and runa's endpoint has its own tests.

### T15. Mobile plugin backend

An interpreter backend (wasm3, WAMR or a maintained pure-Rust interpreter, chosen against each other) behind the same host API, for iOS and Android where wasmtime cannot JIT. Depends on T6. Done when the T6 plugin tests pass unchanged against the interpreter backend on the desktop host, and the choice and its limits are recorded in `research.md`.
