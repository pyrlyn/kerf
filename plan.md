# Kerf

https://github.com/pyrlyn/kerf

An IDE with a small Rust core and sandboxed WASM plugins. Language servers and debug adapters run as separate processes the core starts. See `research.md`.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T12.2 | todo | P2 | 4 | 0% | |
| T12.3 | todo | P2 | 3 | 0% | |
| T12.4 | todo | P2 | 4 | 0% | |
| T15 | todo | P3 | 5 | 0% | |


### T12. UI shell

Split when claimed (500-line task cap). The first window. The creator's direction (`research.md` §7): native controls in a native host per platform, talking to the Rust core through a C ABI, and Kerf's own GPU renderer for the editor surface inside a native view. The editor looks native: like Xcode's on macOS and like Visual Studio's on Windows (`research.md` §7, "Look"); the host supplies the font, colours and caret, the renderer hard-codes none. macOS first; Windows and Linux hosts are in `roadmap.md`. Rendering is a trait `kerf-core` defines (`docs/architecture.md` §2.1). Plugin UI and redraw-on-request come in later tasks. The subtasks together are done when the macOS app opens a file, shows it in the GPU-drawn editor view inside a native AppKit window, takes typing, undo and redo through `kerf-core`, and headless tests cover that path.

#### T12.2. GPU editor renderer

A crate that implements `kerf_core::Render` on `wgpu` + `cosmic-text` + `glyphon` (`research.md` §7): the buffer text, a surface size and scale factor, and a `Theme` the host passes in (font family and size, background, text and selection colours, syntax colours). It reports the caret rectangle so a host can draw the system caret; it draws its own only when the host asks. Done when a test lays out a frame and checks the caret position without a GPU, and a test renders a frame offscreen and reads pixels back on the adapter the CI runner offers (or says in the card why a runner cannot).

#### T12.3. C ABI for native hosts

A `cdylib`/`staticlib` crate with a generated C header: open a file into an `Editor`, attach the renderer to a host view (an `NSView` on macOS), resize, forward text input, keys, undo and redo, render. Every pointer and length from the host is checked; a panic never crosses the boundary. Done when a Rust end-to-end test drives the whole path through the C functions only.

#### T12.4. macOS host

A SwiftPM app in the repository: an AppKit window, the Edit menu (Undo, Redo) and an `NSView` that adopts `NSTextInputClient` and hosts the renderer through the C ABI. It looks like Xcode's editor: SF Mono, the user's Xcode theme when Xcode is installed (Kerf's own otherwise), system selection colour, `NSTextInsertionIndicator` as the caret, Light/Dark with the system. Done when `swift build` and `swift test` pass in CI on macOS arm64 and a test drives open, type, undo and redo through the view.

### T15. Mobile plugin backend

An interpreter backend (wasm3, WAMR or a maintained pure-Rust interpreter, chosen against each other) behind the same host API, for iOS and Android where wasmtime cannot JIT. Depends on T6. Done when the T6 plugin tests pass unchanged against the interpreter backend on the desktop host, and the choice and its limits are recorded in `research.md`.
