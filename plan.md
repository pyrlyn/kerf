# Kerf

https://github.com/pyrlyn/kerf

An IDE with a small Rust core and sandboxed WASM plugins. Language servers and debug adapters run as separate processes the core starts. See `research.md`.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T12.4 | in progress | P2 | 4 | 5% | Claude Code / claude-opus-5-5 |
| T12.5 | todo | P2 | 3 | 0% | |
| T15 | todo | P3 | 5 | 0% | |


### T12. UI shell

Split when claimed (500-line task cap). The first window. The creator's direction (`research.md` §7): native controls in a native host per platform, talking to the Rust core through a C ABI, and Kerf's own GPU renderer for the editor surface inside a native view. The editor looks native: like Xcode's on macOS and like Visual Studio's on Windows (`research.md` §7, "Look"); the host supplies the font, colours and caret, the renderer hard-codes none. macOS first; Windows and Linux hosts are in `roadmap.md`. Rendering is a trait `kerf-core` defines (`docs/architecture.md` §2.1). Plugin UI and redraw-on-request come in later tasks. The subtasks together are done when the macOS app opens a file, shows it in the GPU-drawn editor view inside a native AppKit window, takes typing, undo and redo through `kerf-core`, and headless tests cover that path.

#### T12.4. macOS host

A SwiftPM app in the repository: an AppKit window, the Edit menu (Undo, Redo) and an `NSView` that adopts `NSTextInputClient` and hosts the renderer through the C ABI. It looks like Xcode's editor: SF Mono, the user's Xcode theme when Xcode is installed (Kerf's own otherwise), `NSTextInsertionIndicator` as the caret, Light/Dark with the system. Selection is T12.5. Done when `swift build` and `swift test` pass in CI on macOS arm64 and a test drives open, type, undo and redo through the view.

Plan:

1. `macos/` SwiftPM package (`swift-tools-version` 6.2, macOS 14 for `NSTextInsertionIndicator`): `CKerf`, a system-library module over `crates/kerf-ffi/include/kerf.h` (no copy); `KerfKit`, the library the app and tests share; `Kerf`, the executable. It links the static library `cargo build -p kerf-ffi` leaves in `target/debug`, since SwiftPM cannot build Cargo crates.
2. `KerfKit.Editor`: a Swift owner of `KerfEditor` (open, insert, command, text, cursor) that turns `KerfStatus` into thrown errors.
3. `KerfKit.EditorView`: a layer-backed, flipped `NSView` that attaches a `KerfView` with `kerf_view_attach_appkit`, resizes it at the backing scale, renders after every change, adopts `NSTextInputClient` (`insertText`, `doCommand(by:)` for delete, left, right, newline, tab), answers `undo:`/`redo:` from the Edit menu, and moves an `NSTextInsertionIndicator` to the caret rectangle. Marked (IME) text is committed as typed; showing it inline needs a marked range in the core.
4. `KerfKit.XcodeTheme`: Xcode's current Light or Dark theme name from its defaults, the theme file from the user folder or Xcode's bundle (read at run time, never copied), its background, plain text colour and font; the font family is the one in the font file, the name `cosmic-text`'s font database sees. Without Xcode, Kerf's own Light and Dark themes. The view reloads it when the appearance changes.
5. `Kerf` app: window, App menu (Quit), File > Open, Edit menu (Undo, Redo); opens the path given on the command line.
6. Tests (`swift test`): open a temp file, type, delete, undo and redo through `EditorView`, render, caret indicator placed; the theme parser on a fixture theme written by the test.
7. CI: a `macos-host` job on `xcode-27` (setup-xcode, mise Rust, `cargo build -p kerf-ffi`, `swift build`, `swift test`). Rows in `toolchain.md` and `docs/architecture.md`.

#### T12.5. Editor selection

Selection from the core to the screen: a selection range in `Editor` (anchor and cursor, extended by Shift-moves and replaced by typing), carried in `Frame`; a selection colour in `Theme` that `kerf-render` fills behind the selected text; C ABI calls to set and read the selection; the macOS host passes `NSColor.selectedTextBackgroundColor` (or the Xcode theme's selection colour) and maps Shift-arrows and `selectedRange`/`setSelectedRange` in `NSTextInputClient`. Done when core tests cover selection edits and undo, the offscreen render test finds the selection colour behind the selected text, and the Swift test selects and replaces text through the view.

### T15. Mobile plugin backend

An interpreter backend (wasm3, WAMR or a maintained pure-Rust interpreter, chosen against each other) behind the same host API, for iOS and Android where wasmtime cannot JIT. Depends on T6. Done when the T6 plugin tests pass unchanged against the interpreter backend on the desktop host, and the choice and its limits are recorded in `research.md`.
