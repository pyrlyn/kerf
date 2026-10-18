# Kerf

https://github.com/pyrlyn/kerf

An IDE with a small Rust core and sandboxed WASM plugins. Language servers and debug adapters run as separate processes the core starts. See `research.md`.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T12.5 | todo | P2 | 3 | 0% | |
| T15 | todo | P3 | 5 | 0% | |


### T12. UI shell

Split when claimed (500-line task cap). The first window. The creator's direction (`research.md` §7): native controls in a native host per platform, talking to the Rust core through a C ABI, and Kerf's own GPU renderer for the editor surface inside a native view. The editor looks native: like Xcode's on macOS and like Visual Studio's on Windows (`research.md` §7, "Look"); the host supplies the font, colours and caret, the renderer hard-codes none. macOS first; Windows and Linux hosts are in `roadmap.md`. Rendering is a trait `kerf-core` defines (`docs/architecture.md` §2.1). Plugin UI and redraw-on-request come in later tasks. The subtasks together are done when the macOS app opens a file, shows it in the GPU-drawn editor view inside a native AppKit window, takes typing, undo and redo through `kerf-core`, and headless tests cover that path.

#### T12.5. Editor selection

Selection from the core to the screen: a selection range in `Editor` (anchor and cursor, extended by Shift-moves and replaced by typing), carried in `Frame`; a selection colour in `Theme` that `kerf-render` fills behind the selected text; C ABI calls to set and read the selection; the macOS host passes `NSColor.selectedTextBackgroundColor` (or the Xcode theme's selection colour) and maps Shift-arrows and `selectedRange`/`setSelectedRange` in `NSTextInputClient`. Done when core tests cover selection edits and undo, the offscreen render test finds the selection colour behind the selected text, and the Swift test selects and replaces text through the view.

### T15. Mobile plugin backend

An interpreter backend (wasm3, WAMR or a maintained pure-Rust interpreter, chosen against each other) behind the same host API, for iOS and Android where wasmtime cannot JIT. Depends on T6. Done when the T6 plugin tests pass unchanged against the interpreter backend on the desktop host, and the choice and its limits are recorded in `research.md`.
