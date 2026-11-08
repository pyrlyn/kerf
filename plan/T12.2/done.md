# T12.2. GPU editor renderer: done

Outcome:

- `crates/kerf-render`: `Theme`, `Size`, `Rect`; `Layout` (GPU-free, returns the caret rectangle); `GpuRenderer::draw` into any texture view; `Offscreen`, a renderer with its own texture that implements `Render`. The caret is a solid custom glyph through `glyphon`'s pipeline, drawn only with `draw_caret`.
- Layout lines are built from the core's lines, split the way ropey splits them (`\n\r`, U+2028, NEL), so `Frame::line` indexes the layout; `cosmic-text` alone splits only on CR and LF. Host numbers (scale, sizes, caret width) are clamped, since they cross the C ABI.
- `Render` gained `type Error`: a GPU draw can fail (atlas full, evicted glyph).
- Left out: selection and syntax colours, because `Frame` carries no selection or highlight spans yet (noted on T12.4). The `wgpu` surface over an `NSView` comes with the C ABI in T12.3, where the host handle exists.
- Tests: six layout tests without a GPU, and `tests/offscreen.rs`, which renders on the machine's adapter and reads pixels back (background, text, caret on and off). CI installs Mesa's lavapipe on Linux and sets `KERF_REQUIRE_GPU`, so a runner without an adapter fails instead of skipping.

Commits: pyrlyn/kerf#26 (`8060a5d`).
