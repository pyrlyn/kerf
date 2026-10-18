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

### T8. LSP results in the semantic layer

Merge language-server answers into `kerf-semantic` where tree-sitter misses (macro bodies, type-resolved references), tagged by source so tree-sitter results stay the base. Add at least one more tree-sitter language. Done when the recall test on the labelled fixture improves with a scripted language server and a second language has its own fixture.

Split when claimed (500-line task cap); both subtasks land in one PR, in order.

#### T8.1. Merge API and a second grammar in `kerf-semantic`

1. Tag every `Symbol` with its source (`TreeSitter` or `LanguageServer`). `Semantic::merge_lsp` takes `gen-lsp-types` document symbols and reference locations (`lsp-types` has had no release since 2024-06; `gen-lsp-types` is generated from the upstream schema and used by rust-analyzer and ruff), maps UTF-16 or UTF-8 positions to bytes, keeps only answers that land on an identifier with the claimed name, never replaces a tree-sitter symbol, caps answers per file, and drops a file's LSP symbols on its next edit (their positions are stale).
2. Python grammar and tags query from `tree-sitter-python`, picked by file extension; `tests/fixtures/recall.py` with its own labels and recall test.
3. The Rust fixture gains what tree-sitter cannot see (an item a macro defines, a function named as a value); the recall test reports tree-sitter alone and tree-sitter plus a scripted answer file `tests/fixtures/recall.lsp.json`.
4. Unit tests for hostile answers: out of range, wrong name, split UTF-8, over the cap.

#### T8.2. LSP client with a scripted server in `crates/kerf-lsp`

1. JSON-RPC over stdio with `Content-Length` framing, as `docs/architecture.md` §5 says: 4 MiB message cap (kill the process on abuse), a bounded inbound queue that drops notifications with a counter, an 8 KiB stderr ring, a request timeout, and `shutdown`/`exit` with a 2 s budget before kill.
2. The process starts through a small `Spawn` trait, so T7's trust check can wrap it later.
3. A scripted fake server binary replays `recall.lsp.json`; an end-to-end test runs it through the client into `kerf-semantic` and checks the same recall gain.
4. Verify: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`; numbers in the commits.

Result: `Semantic::merge_lsp` and Python in `kerf-semantic`, the client in `crates/kerf-lsp` with the scripted server `kerf-fake-lsp`. Recall on `recall.rs` (23 definitions, 31 references): tree-sitter alone 0.957 and 0.968; with the scripted answers 1.000 and 1.000, both in-process and over the wire, two symbols added, no unlabelled hits. `recall.py` (11 definitions, 15 references): 1.000 and 1.000 with the upstream query. Not done here: process counts and the restart budget of `docs/architecture.md` §5.2 (they belong to `kerf-process`), and diagnostics.

### T5. Return an error for the largest invalid line index

TextBuffer::line must reject every out-of-range index without panicking or wrapping its diagnostic range.

Execution plan: add a usize::MAX regression in buffer.rs; use the standard saturating integer operation for the diagnostic end; run the crate tests, clippy and formatting. Existing range checks remain authoritative; no shared helper or dependency is needed for a single integer operation.

Validation: the regression panicked on overflow before the fix. All 44 tests pass under nextest; clippy with -D warnings and formatting checks pass. Reused the standard usize::saturating_add operation and preserved ordinary diagnostic ranges.
