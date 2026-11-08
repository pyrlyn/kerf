# T12.4. macOS host: done

Outcome:

- `macos/`: `CKerf` (system module over `crates/kerf-ffi/include/kerf.h`), `KerfKit` (`Editor`, `EditorView`, `XcodeTheme`) and the `Kerf` app (File > Open, Edit > Undo and Redo, a path on the command line). No Swift package dependencies.
- `EditorView` renders after every change and on resize, scale and appearance changes; the caret is the system `NSTextInsertionIndicator` at Kerf's caret rectangle, and `selectedRange` converts Kerf's UTF-8 cursor to UTF-16. Key bindings past delete, left, right, newline and tab wait for core support; IME composition is committed as typed.
- `XcodeTheme`: Xcode's `XCFontAndColorCurrentTheme`/`XCFontAndColorCurrentDarkTheme`, the theme from the user folder or Xcode's bundle, background, plain text colour and font; anything missing falls back to Kerf's Light and Dark themes. The font family comes from the font's `name` table, since CoreText's name for the system monospace (".AppleSystemUIFontMonospaced") is in no file and Kerf's font database would miss it. Font weight is not passed yet (`Theme` has none).
- Fix found by the host test: a hidden window skips frames (wgpu reports `Occluded`), and the caret stayed where the last drawn frame had it. `GpuRenderer::lay_out` now updates the caret on skipped frames.
- Tests: `swift test` types, deletes, undoes and redoes through the view in a window that is not on screen, checks the caret and the UTF-16 cursor, follows Light/Dark, and parses fixture themes the test writes (Kerf never copies Apple's files). CI: the `macos-host` job on `xcode-27` builds `kerf-ffi`, then runs `swift-format lint --strict`, `swift build` and `swift test`.

Commits: pyrlyn/kerf#28 (`a182afc`).
