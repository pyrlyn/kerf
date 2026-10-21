# Kerf

https://github.com/pyrlyn/kerf

An IDE with a small Rust core and sandboxed WASM plugins. Language servers and debug adapters run as separate processes the core starts. See `research.md`.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T15 | in progress | P3 | 5 | 70% | Claude / opus-5.5 |
| T15 | todo | P3 | 5 | 0% | |
| T17.1 | in progress | P1 | 3 | 60% | Claude / opus-5.5 |
| T17.2 | in progress | P1 | 4 | 60% | Claude / opus-5.5 |
| T17.3 | in progress | P1 | 2 | 60% | Claude / opus-5.5 |


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

Plan (approved by the creator: wasmtime's Pulley interpreter through a `pulley` feature of the shared host):

1. pyrlyn/crates T20.6 (pyrlyn/crates#107): feature `pulley` on `wasm-plugin-host` and `plugwright-extism` hands extism a `wasmtime::Config` targeting Pulley; their suites pass with it on.
2. Kerf: bump the `wasm-plugin-host` git pin to the merged commit; `kerf-plugin` feature `interpreter` forwards to `wasm-plugin-host/pulley`, and iOS and Android targets turn it on.
3. CI runs `cargo test -p kerf-plugin --features interpreter` beside the default run; `crates/kerf-plugin/tests/plugins.rs` stays unchanged.
4. `research.md` §8: candidates with primary sources, the choice and its limits; `toolchain.md` row.
