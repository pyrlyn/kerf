# Kerf

https://github.com/pyrlyn/kerf

An IDE with a small Rust core and sandboxed WASM plugins. Language servers and debug adapters run as separate processes the core starts. See `research.md`.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T16.1 | in progress | P1 | 4 | 80% | Claude / opus-5.5 |
| T16.3 | todo | P1 | 2 | 0% | |
| T15 | todo | P3 | 5 | 0% | |
| T17.1 | in progress | P1 | 3 | 60% | Claude / opus-5.5 |
| T17.2 | in progress | P1 | 4 | 60% | Claude / opus-5.5 |
| T17.3 | in progress | P1 | 2 | 60% | Claude / opus-5.5 |


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

#### T16.3. Remove the GPU renderer

Delete `crates/kerf-render` and the `wgpu`, `glyphon` and `pollster` dependencies once nothing uses them, with their `toolchain.md` rows, the lavapipe CI step and the `docs/architecture.md` row. Done when the workspace builds and tests pass without them.

### T17. Linux host on GTK 4

The Linux host for the UI shell (from `roadmap.md`, approved with T12's native direction): a GTK 4 window over T12's C ABI with the Slint editor surface from T16, looking like GNOME Text Editor. Done when the Linux app opens a file, shows it in the Slint-drawn view, takes typing, selection, undo and redo through `kerf-core`, takes input-method text through GTK, draws a GTK-style caret, and headless tests of that path run in CI on `ubuntu-latest`.

The creator's choices (2026-10-20): the host is Rust on gtk4-rs calling `kerf-ffi`'s C functions; the look is GNOME Text Editor's on plain GTK 4 (no libadwaita).

Split when claimed (500-line task cap) into three stacked pull requests. Research for all three is in `research.md` §7, "Linux host (T17)". Locally the Linux tests run in a Lima VM from `template:ubuntu-24.04` (GTK 4.14, as on the CI runner).

#### T17.1. GTK host crate: the editor over the C ABI and GNOME's look

`crates/kerf-gtk` (Linux only; on other targets only the plain `look` rules build): `Editor`, a safe wrapper over the `kerf_editor_*` calls; `look`, GNOME Text Editor's Adwaita and Adwaita-dark colours, line height and GTK's caret width and blink rules as plain values; `gnome`, GNOME's monospace font measured by Pango and the theme's darkness from the widget's text colour. CI installs `libgtk-4-dev` and runs Xvfb on Linux with `KERF_REQUIRE_DISPLAY`. Done when the editor tests (open, type, select, undo, redo, refusals), the `look` unit tests and a display test of the GNOME look pass in CI.

#### T17.2. Editor view widget

`EditorView`, a `GtkWidget` subclass: `kerf_view_*` draws into a `GdkMemoryTexture` shown in the widget's snapshot; keys through a key controller with a `GtkIMMulticontext` (commits as typed, no inline preedit); GTK's caret over the frame with GtkTextView's blink, hidden over a selection; the look re-applied on theme and font changes. Done when tests under Xvfb type through the input method and keys, select, undo, redo and read the frame's pixels and the caret back.

#### T17.3. Linux app window

`kerf-gtk` binary: a `GtkApplication` that opens files from the command line and an Open dialog, one window per file with a header bar (Open, main menu with Undo, Redo, Select All) and GNOME's shortcuts. Done when a test builds the window for an opened file and drives its actions.

### T15. Mobile plugin backend

An interpreter backend (wasm3, WAMR or a maintained pure-Rust interpreter, chosen against each other) behind the same host API, for iOS and Android where wasmtime cannot JIT. Depends on T6. Done when the T6 plugin tests pass unchanged against the interpreter backend on the desktop host, and the choice and its limits are recorded in `research.md`.
