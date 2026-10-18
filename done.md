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

### T11. Language-server and debug-adapter binaries through ketch

Install and update language-server and debug-adapter binaries from GitHub releases with checksums, through ketch. Done when a test installs a pinned binary from a local fixture release, rejects a bad checksum, and the core resolves the installed path.

Plan:

1. Study ketch (read-only): library crate or CLI only, how it verifies checksums, how it can be pointed at a local release.
2. Add a crate: a pinned binary spec (repo, tag, per-target asset and sha256, binary name), install through ketch into a Kerf-owned root, and `resolve` returning the installed binary path the core spawns.
3. Treat every download as untrusted: cap the download and the installed size, hold the asset to the pinned sha256 before anything is unpacked, refuse a binary that resolves outside the root.
4. Tests against a local fixture release (no network): install a pinned binary, reject a bad checksum, resolve the path, refuse oversize downloads and installs.
5. Verify with `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`; update `toolchain.md`.

Result: `crates/kerf-binaries` drives the `ketch` CLI. `ketch-core` is a library but unpublished, pulls SQLite, octocrab/tokio and signature stacks, and is GPL-only, which would remove Kerf's royalty-free licence option. `Installer::install` writes a one-entry `ketch.lock` for the host target and runs `ketch sync`, the only CLI path that holds a download to a caller-chosen digest; ketch refuses a mismatch before unpacking. Downloads are capped through `KETCH_MAX_DOWNLOAD_MIB`; ketch has no unpack cap, so Kerf measures the installed tree and uninstalls it when over its cap. Six integration tests run the real ketch (from `mise.toml`, also installed in CI) against a loopback stand-in for the GitHub API.

### T5. Return an error for the largest invalid line index

TextBuffer::line must reject every out-of-range index without panicking or wrapping its diagnostic range.

Execution plan: add a usize::MAX regression in buffer.rs; use the standard saturating integer operation for the diagnostic end; run the crate tests, clippy and formatting. Existing range checks remain authoritative; no shared helper or dependency is needed for a single integer operation.

Validation: the regression panicked on overflow before the fix. All 44 tests pass under nextest; clippy with -D warnings and formatting checks pass. Reused the standard usize::saturating_add operation and preserved ordinary diagnostic ranges.
