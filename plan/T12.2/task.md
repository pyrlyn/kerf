# T12.2. GPU editor renderer

A crate that implements `kerf_core::Render` on `wgpu` + `cosmic-text` + `glyphon` (`research.md` §7): the buffer text, a surface size and scale factor, and a `Theme` the host passes in (font family and size, background, text and selection colours, syntax colours). It reports the caret rectangle so a host can draw the system caret; it draws its own only when the host asks. Done when a test lays out a frame and checks the caret position without a GPU, and a test renders a frame offscreen and reads pixels back on the adapter the CI runner offers (or says in the card why a runner cannot).

Plan:

1. `crates/kerf-render` (row in `docs/architecture.md` §2.2); `wgpu`, `glyphon` (and its `cosmic-text`) in `[workspace.dependencies]`, rows in `toolchain.md`.
2. `Theme` (font family, size, line height, colours) and `Layout`: shapes the frame's text with `cosmic-text` at the host's scale factor and returns the caret rectangle in physical pixels. No GPU, unit-tested: caret on line 2, column after a wide character, scale 2.
3. `GpuRenderer` on a `wgpu::Device`/`Queue` and a target texture format: clears to the background, draws the text with `glyphon`, and a caret quad only when `Theme::draw_caret` is set. `impl Render` draws into a surface; an offscreen `render_to(&TextureView)` serves tests and hosts that own the texture.
4. Offscreen test: render into a texture, copy it to a buffer, read pixels back, check background and text colours. Skips with a message when no adapter exists; CI installs Mesa's lavapipe on Linux, Windows has WARP, macOS arm64 has Metal.
5. fmt, clippy `-D warnings`, tests; CI green on all three runners.
