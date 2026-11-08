# T16.1. Slint editor component

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
