# Done

### T3. Semantic layer: incremental parse and symbol index

`crates/kerf-semantic`. Done when an edit re-parses only the changed file incrementally, the index updates only that file's symbols, and a test measures definition and reference recall on a labelled Rust fixture.

It holds:

- tree-sitter parsing per file, with incremental re-parse from edit events (the same shape as T2.1's);
- a symbol index (definitions and references) from tree-sitter tags queries, updated per file.

Rust is the first language. Scope stays within the 500-line task cap; more languages and LSP merging are later tasks.

Plan:

1. Reuse check, done: rtok's tags code (`src/plugins/read/outline.rs`) runs `TagsContext::generate_tags`, which always parses the whole file and takes no existing tree, so it cannot be incremental. The only overlap is a few lines of extra Rust query. Write the minimum here; nothing in rtok changes.
2. Edits come in as `kerf_core::EditEvent` plus the post-edit text. T2 has merged, so a second edit-event type would duplicate it. `kerf-semantic` depends on `kerf-core` for that type only, never on the buffer.
3. `crates/kerf-semantic`:
   - `document.rs`: source and tree per file; `EditEvent` → `InputEdit`, validated against both texts; incremental re-parse with the old tree.
   - `index.rs`: per-file definitions and references from `tree_sitter_rust::TAGS_QUERY` (plus path-qualified calls), run with a `QueryCursor` on the existing tree; hard caps on file size, symbols per file and files.
   - `lib.rs`: `Semantic` with `open`, `edit`, `close` and lookups by name.
4. Tests: insert, edit and delete keep the index right; an incremental re-parse matches a fresh parse and reports changed ranges only around the edit; editing one file leaves the other files' entries untouched; bad edits and over-cap input are refused.
5. Labelled fixture `tests/fixtures/recall.rs` with `recall.labels`; a recall test that prints definition and reference recall and fails below the threshold. Report the numbers in the commit.
6. `tree-sitter` and `tree-sitter-rust` rows in `toolchain.md`; `rust.md` already lists both.
7. Verify: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, then CI on the PR.

Result: definition recall 1.000 and reference recall 1.000 on `tests/fixtures/recall.rs` (20 definitions, 27 references, no unlabelled hits). The upstream tags query alone reaches 0.750 and 0.519; the extra patterns cover path-qualified and turbofish calls, calls inside macro arguments, `impl Trait for Type`, trait method signatures, enum variants, `const` and `static`.

### T9. Inspections with quick fixes

A plugin export that runs over the core's index and returns diagnostics with edits, as in IntelliJ. The contract (JSON in and out, versioned), a core-side runner that validates and caps what a plugin returns, and applying a fix as one undoable edit. Done when a test inspection reports a problem on a fixture and its fix applies and undoes.

Plan:

1. Reuse check: `kerf-core` already has the undoable replace (`TextBuffer::apply`) and `kerf-semantic` the per-file symbols (`FileSymbols`); cox's `cox-plugin-api` `abi.rs` sets the JSON style (`api` major in the input, no `deny_unknown_fields`, caps as consts). Nothing to extract.
2. New crate `crates/kerf-inspect`:
   - `contract.rs`: `InspectIn` (`api`, file path and text, the file's index slice) and `InspectOut` (diagnostics with severity, byte range, message and fixes made of edits), serde JSON in cox's ABI style; export name `kerf_inspect`.
   - `runner.rs`: an `Inspection` trait (JSON bytes in, bytes out) so T6's plugin host plugs in later; the runner builds the input from `TextBuffer` and `Semantic`, caps the reply size before parsing, then validates and caps diagnostics, fixes, edits, text lengths, ranges (in bounds, on character boundaries, edits sorted and not overlapping). A bad reply is a typed error and no diagnostics.
   - Applying a fix: its edits collapse into one replace over their span, so it is one `TextBuffer` undo step and one `EditEvent`; it refuses a buffer that changed under the span since the run.
3. Tests with an in-process fake inspection: a problem reported on a fixture, the fix applies and undoes, the index re-reads the edit; oversized, malformed, out-of-range, overlapping and over-cap replies are rejected; a stale fix is refused.
4. `serde` and `serde_json` rows in `toolchain.md`.
5. Verify: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.

Result: `crates/kerf-inspect` with the `kerf_inspect` contract (`api = 1`), the runner behind the `Inspection` trait for T6 to implement, and `Report::apply_fix`. 14 integration tests on `tests/fixtures/deprecated.rs`.

### T5. Return an error for the largest invalid line index

TextBuffer::line must reject every out-of-range index without panicking or wrapping its diagnostic range.

Execution plan: add a usize::MAX regression in buffer.rs; use the standard saturating integer operation for the diagnostic end; run the crate tests, clippy and formatting. Existing range checks remain authoritative; no shared helper or dependency is needed for a single integer operation.

Validation: the regression panicked on overflow before the fix. All 44 tests pass under nextest; clippy with -D warnings and formatting checks pass. Reused the standard usize::saturating_add operation and preserved ordinary diagnostic ranges.
