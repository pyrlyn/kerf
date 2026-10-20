# Kerf

https://github.com/pyrlyn/kerf

An IDE with a small Rust core and sandboxed WASM plugins. Language servers and debug adapters run as separate processes the core starts. See `research.md`.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T16.1 | in progress | P1 | 4 | 80% | Claude / opus-5.5 |
| T16.2 | todo | P1 | 4 | 0% | |
| T16.3 | todo | P1 | 2 | 0% | |
| T15 | in progress | P3 | 5 | 70% | Claude / opus-5.5 |


### T16. Slint editor surface

The creator's direction of 2026-10-20 (`research.md` §7): the editor surface is a Slint component, embedded in the native host through slint-bindings, instead of Kerf's own GPU renderer. The native window, menus, input (`NSTextInputClient`), the system caret and the Xcode look stay as T12 built them. The subtasks together are done when the macOS app opens a file, shows it in the Slint-drawn editor view, takes typing, selection, undo and redo through `kerf-core`, headless tests cover that path, and `kerf-render` with its GPU dependencies is gone.

#### T16.1. Slint editor component

A crate that implements `kerf_core::Render` with a Slint component: the buffer text, the selection, a surface size and scale factor and the host's `Theme` (the same fields `kerf-render` takes), rendered by `slint-bindings-core`'s `EmbeddedHost` into an RGBA buffer the caller owns. It reports the caret rectangle so the host keeps the system caret. Done when tests render a frame on the CPU and check the background, text and selection pixels and the caret rectangle.

Plan:

1. Check the dependency and the licence and record both in `research.md` §7 with sources: `slint-bindings-core` from `pyrlyn/slint-bindings` as a git dependency pinned to a commit (it is `publish = false`), the exact `slint` pin it requires, and the Slint licence terms for a desktop app under Kerf's licences.
2. `crates/kerf-slint` (row in `docs/architecture.md` §2.2): `ui/editor.slint` built with `slint-build`; a `SlintView` over `EmbeddedHost` with `resize`, `set_theme`, `render(&mut [u8])` and the caret rectangle; `impl Render` feeds it the `Frame`.
3. Tests: render a known `Frame` into a buffer and compare pixels; caret rectangle after moves and edits; a resize changes the buffer length.
4. `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`.

Execution:

- Dependency: `slint-bindings-core` git pin `9c1b353e114c83d5cb03cb61d504a166ed49efa1`; `slint` and `slint-build` at the `=1.18.1` it forces. Facts, sources and the licence terms go into `research.md` §7 "Slint dependency and licence".
- `ui/editor.slint`: an `EditorComponent` window that draws the visible lines as one `Text` each, a selection fill per line and an optional caret. Column offsets are measured by Slint itself (hidden `Text` elements over the line prefix), so the caret and the selection follow the same shaping as the drawn text.
- `src/lib.rs`: `Theme` (the `kerf-render` fields, kept here because T16.3 deletes `kerf-render`), `Rect` in logical points, `SlintView` with `new`, `resize`, `set_theme`, `draw(&mut [u8])` (not `render`, which `impl Render` already names for the frame), `caret()` and `impl Render`.
- Tests (`tests/render.rs`, CPU only): background, text and selection pixels of a known frame; caret after moves, edits and a new line; resize changes the frame length; a short buffer is refused.

#### T16.2. Native host on the Slint view

`kerf-ffi`'s view handle draws through `kerf-slint` into a buffer the host owns, instead of a `wgpu` surface over the host's `NSView`. The macOS `EditorView` presents that buffer (as slint-bindings' `SlintNSView` does) on each display-link tick and keeps `NSTextInputClient`, `NSTextInsertionIndicator` and the Xcode theme. Done when the Rust end-to-end test over the C ABI and the headless Swift test open, type, select, undo and redo, and the header has no drift.

#### T16.3. Remove the GPU renderer

Delete `crates/kerf-render` and the `wgpu`, `glyphon` and `pollster` dependencies once nothing uses them, with their `toolchain.md` rows, the lavapipe CI step and the `docs/architecture.md` row. Done when the workspace builds and tests pass without them.

### T15. Mobile plugin backend

An interpreter backend (wasm3, WAMR or a maintained pure-Rust interpreter, chosen against each other) behind the same host API, for iOS and Android where wasmtime cannot JIT. Depends on T6. Done when the T6 plugin tests pass unchanged against the interpreter backend on the desktop host, and the choice and its limits are recorded in `research.md`.

Plan (approved by the creator: wasmtime's Pulley interpreter through a `pulley` feature of the shared host):

1. pyrlyn/crates T20.6 (pyrlyn/crates#107): feature `pulley` on `wasm-plugin-host` and `plugwright-extism` hands extism a `wasmtime::Config` targeting Pulley; their suites pass with it on.
2. Kerf: bump the `wasm-plugin-host` git pin to the merged commit; `kerf-plugin` feature `interpreter` forwards to `wasm-plugin-host/pulley`, and iOS and Android targets turn it on.
3. CI runs `cargo test -p kerf-plugin --features interpreter` beside the default run; `crates/kerf-plugin/tests/plugins.rs` stays unchanged.
4. `research.md` §8: candidates with primary sources, the choice and its limits; `toolchain.md` row.
