# T12.1. Rendering trait, editor and toolkit choice

- `research.md` §7: the UI shell direction with primary sources — first Slint through slint-bindings against Flutter through slint_dart, then the creator's choice of native hosts over a C ABI with Kerf's own GPU editor renderer (`wgpu` + `cosmic-text` + `glyphon`).
- `kerf-core`: the `Render` trait and the `Frame` the core hands it, and an `Editor` (buffer + cursor) that turns typing, backspace, cursor moves, undo and redo into `TextBuffer` edits.

Plan:

1. Compare the paths in `research.md` §7 (slint-bindings and slint_dart READMEs at `4d61045`, Slint, Flutter, wgpu and vello docs, crates.io metadata).
2. `crates/kerf-core/src/render.rs`: `Render`, `Frame` (buffer, cursor byte, line, column).
3. `crates/kerf-core/src/editor.rs`: `Editor`; `TextBuffer::line_col`, `prev_boundary`, `next_boundary` for whole-character steps.
4. Verify: unit tests for each editor operation and a recording `Render`; `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
