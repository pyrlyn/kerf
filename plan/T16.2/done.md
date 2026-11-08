# T16.2. Native host on the Slint view: done

Result: `kerf_view_new`, `kerf_view_render` (`KERF_STATUS_NOTHING` when Slint repainted nothing), `kerf_view_read_rgba` (premultiplied RGBA8), `kerf_view_caret` in logical points; a view called or freed from another thread gets `KERF_STATUS_WRONG_THREAD` or is leaked rather than dropped there. `EditorView` renders on each edit, resize and appearance change instead of a display-link tick: the component has no animation and the caret is the system's, so nothing changes between edits. Three C ABI tests and eight Swift tests pass; `cargo clippy --workspace --all-targets -D warnings`, `cargo fmt --check` and `swift-format lint --strict` are clean.

Commits: pyrlyn/kerf#34 (`991c2fe`).
