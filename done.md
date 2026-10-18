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

### T5. Return an error for the largest invalid line index

TextBuffer::line must reject every out-of-range index without panicking or wrapping its diagnostic range.

Execution plan: add a usize::MAX regression in buffer.rs; use the standard saturating integer operation for the diagnostic end; run the crate tests, clippy and formatting. Existing range checks remain authoritative; no shared helper or dependency is needed for a single integer operation.

Validation: the regression panicked on overflow before the fix. All 44 tests pass under nextest; clippy with -D warnings and formatting checks pass. Reused the standard usize::saturating_add operation and preserved ordinary diagnostic ranges.

### T10. Plugin lockfile

A committed lockfile of plugin ids, versions and package digests. Load refuses a plugin whose digest does not match. Done when tests cover write, read, a digest mismatch and a missing entry.

Result: crate `crates/kerf-plugin-lock` with `Lockfile::read`, `write`, `pin`, `unpin` and `verify`; schema in `docs/schema/kerf-plugins.lock.schema.json`. The plugin host (T6) calls `verify` with the digest it computes through `wasm-plugin-host`; that wiring belongs to T6.

Plan:

1. Reuse from cox: it has no lockfile, but it has the package digest (`cox-plugin` `package_digest`, the same function `wasm-plugin-host` carries: SHA-256 hex over the package tree). The lockfile stores that digest; it does not define a second one.
2. New crate `crates/kerf-plugin-lock`, the one module that reads and writes the lockfile: `format = 1` and a `[plugins.<id>]` table of `version` and `digest` (a table keyed by id makes a duplicate id a TOML error), TOML through `toml`, written atomically through `fs-kit`, sorted by id.
3. Validation in the types (`deny_unknown_fields`, a 64-hex digest newtype, `format` other than 1 refused); the file is repository input, so reads are capped at 1 MiB.
4. `Lockfile::verify(id, version, digest)` for the plugin host (T6) to call before it loads a package; mismatch and missing entry are typed errors.
5. JSON Schema committed under `docs/` and checked by a test through `config-schema`.
6. Tests (20): write, read, digest mismatch, version mismatch, missing entry, malformed TOML, unknown key, duplicate id, bad format, bad digest, oversized and non-UTF-8 file, stale schema. Verify with `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`.
