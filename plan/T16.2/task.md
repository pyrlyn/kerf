# T16.2. Native host on the Slint view

`kerf-ffi`'s view handle draws through `kerf-slint` into a buffer the host owns, instead of a `wgpu` surface over the host's `NSView`. The macOS `EditorView` presents that buffer (as slint-bindings' `SlintNSView` does) on each display-link tick and keeps `NSTextInputClient`, `NSTextInsertionIndicator` and the Xcode theme. Done when the Rust end-to-end test over the C ABI and the headless Swift test open, type, select, undo and redo, and the header has no drift.

Plan:

1. `kerf-ffi`: `KerfView` wraps `kerf_slint::SlintView` and keeps its last frame; `kerf_view_new` replaces `kerf_view_new_offscreen` and `kerf_view_attach_appkit`; `kerf_view_render` draws into the kept frame, `kerf_view_read_rgba` copies it out, `kerf_view_caret` reports logical points. Drop `kerf-render`, `wgpu` and `pollster` from the crate.
2. Slint objects are `!Send`, so every view call checks the creating thread; `KERF_STATUS_GPU` becomes `KERF_STATUS_RENDER`, plus `KERF_STATUS_WRONG_THREAD`.
3. macOS: `EditorView` copies the frame after each render into a `CGImage` on its layer; the caret is placed in points; the package links the frameworks Slint's text stack needs instead of Metal.
4. Tests: the C ABI end-to-end test on the CPU view and a cross-thread test; the Swift test checks the frame on the layer. Header regenerated.
