# T12.4. macOS host

A SwiftPM app in the repository: an AppKit window, the Edit menu (Undo, Redo) and an `NSView` that adopts `NSTextInputClient` and hosts the renderer through the C ABI. It looks like Xcode's editor: SF Mono, the user's Xcode theme when Xcode is installed (Kerf's own otherwise), `NSTextInsertionIndicator` as the caret, Light/Dark with the system. Selection is T12.5. Done when `swift build` and `swift test` pass in CI on macOS arm64 and a test drives open, type, undo and redo through the view.

Plan:

1. `macos/` SwiftPM package (`swift-tools-version` 6.2, macOS 14 for `NSTextInsertionIndicator`): `CKerf`, a system-library module over `crates/kerf-ffi/include/kerf.h` (no copy); `KerfKit`, the library the app and tests share; `Kerf`, the executable. It links the static library `cargo build -p kerf-ffi` leaves in `target/debug`, since SwiftPM cannot build Cargo crates.
2. `KerfKit.Editor`: a Swift owner of `KerfEditor` (open, insert, command, text, cursor) that turns `KerfStatus` into thrown errors.
3. `KerfKit.EditorView`: a layer-backed, flipped `NSView` that attaches a `KerfView` with `kerf_view_attach_appkit`, resizes it at the backing scale, renders after every change, adopts `NSTextInputClient` (`insertText`, `doCommand(by:)` for delete, left, right, newline, tab), answers `undo:`/`redo:` from the Edit menu, and moves an `NSTextInsertionIndicator` to the caret rectangle. Marked (IME) text is committed as typed; showing it inline needs a marked range in the core.
4. `KerfKit.XcodeTheme`: Xcode's current Light or Dark theme name from its defaults, the theme file from the user folder or Xcode's bundle (read at run time, never copied), its background, plain text colour and font; the font family is the one in the font file, the name `cosmic-text`'s font database sees. Without Xcode, Kerf's own Light and Dark themes. The view reloads it when the appearance changes.
5. `Kerf` app: window, App menu (Quit), File > Open, Edit menu (Undo, Redo); opens the path given on the command line.
6. Tests (`swift test`): open a temp file, type, delete, undo and redo through `EditorView`, render, caret indicator placed; the theme parser on a fixture theme written by the test.
7. CI: a `macos-host` job on `xcode-27` (setup-xcode, mise Rust, `cargo build -p kerf-ffi`, `swift build`, `swift test`). Rows in `toolchain.md` and `docs/architecture.md`.
