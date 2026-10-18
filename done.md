# Done

### T1. Architecture document

The contracts every other task builds on, in `docs/architecture.md`. Done when the creator approves the document. The creator delegated the open points; the decisions are in §8 of the document.

It covers:

- the core boundary (rendering, buffer, event loop, semantic layer) and the crate map;
- the plugin lifecycle on cox's ABI as is (`cox:host/v1`, extism, `wasm-plugin-host`): manifest read without running code, lazy activation, crash and disable;
- the IDE permission set and the manifest tables for language servers and debug adapters, which are the input for T4.1 and T4.2;
- how the core starts, supervises, caps and stops LSP and DAP processes;
- Workspace Trust;
- interface versioning.

Plan:

1. Read `research.md`, cox's `docs/design/plugins.md`, `crates/cox-plugin-api/src/manifest.rs` and `wasm-plugin-host` (with crates-packages#42).
2. Write `docs/architecture.md`. Every borrowed idea links to its `research.md` section.
3. List the open points for the creator at the end of the document.
4. Verify: every permission and manifest table named in the document maps to a cox concept or is marked new.

### T2. Core skeleton

Split when claimed (500-line task cap). The first crate, `crates/kerf-core`, with no UI. The three subtasks together are done when `cargo test`, `cargo clippy -- -D warnings` and `cargo fmt --check` pass locally and in CI.

### T2.1. Text buffer, edits, undo and redo

- the text buffer, on a maintained rope crate chosen against the alternatives;
- edits with undo and redo, emitting edit events (byte ranges and positions) that the semantic layer (T3) can consume.

Plan:

1. Pick the rope crate: compare maintained options on crates.io and in `rust.md`, and give the reason in the commit.
2. Write the buffer, edits, undo and redo with tests.
3. Define the edit-event type so T2.2 and T3 can consume it without depending on the buffer internals.

### T2.2. Event loop with bounded queues

- a single-threaded event loop with bounded queues;
- back-pressure when a queue is full.

Plan:

1. Write the event loop and bounded queues against T2.1's edit events (or a stand-in type if T2.1 is not merged yet).
2. Tests for back-pressure and for a full queue answering rather than growing.

### T2.3. CI and toolchain bookkeeping

- a GitHub Actions workflow for fmt, clippy and test on Linux x86_64, macOS arm64 and Windows x86_64;
- `toolchain.md` and `rust.md` rows for every crate T2 introduces.

Plan:

1. Add the workflow (Linux x86_64, macOS arm64, Windows x86_64 — no Intel Mac).
2. Add the crates to `toolchain.md`, and to `rust.md` if they are new.

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

### T7. Workspace Trust

An untrusted folder starts no language servers, debuggers or tasks until the user trusts it. A trust store in the core, a check every process spawn goes through, and denial as a typed error. Done when tests show a spawn refused in an untrusted folder and allowed after trust is granted.

Plan:

1. Add a `trust` module to `crates/kerf-core` (the card says "in the core"; `kerf-process` is still only proposed): an in-memory `TrustStore` of canonical folder roots, where a trusted root covers its subfolders (VS Code and Zed both do this).
2. One check, `TrustStore::authorize(cwd, kind)`, returns a `SpawnPermit` that only it can build, so a future `kerf-process` spawn takes the permit and cannot skip the check. Denial is a typed `TrustError`. Paths are canonicalized so `..` and symlinks cannot reach a trusted root's prefix.
3. `revoke` returns the roots it removed so a supervisor can stop their processes (§5.3). Hard cap on stored roots.
4. Persistence: a user-level `trust.toml` in the OS config directory (architecture decision 7), owned by one `trust::file` module with a committed schema.
5. Tests: spawn refused while untrusted, allowed after trust, refused after revoke; subfolder, sibling-prefix and `..` cases; the cap. Verify with `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.

Result: `kerf_core::trust` — `TrustStore` (`grant`, `revoke`, `is_trusted`, `authorize`), `SpawnPermit`, `SpawnKind`, `TrustError`, `MAX_TRUSTED_ROOTS`. Ten unit tests cover refusal before trust, a permit after trust, refusal after revoke, subfolders, a sibling sharing the prefix, `..`, a symlink out of the root, revoking a parent or a subfolder, a deleted root, unresolvable paths and the cap. Persistence followed architecture decision 7: `TrustStore::load` / `save` / `default_path` keep the roots in `kerf/trust.toml` under the OS config directory (`dirs`), with a committed JSON Schema checked by a test, a 4 MiB read cap, the 1024-root cap, absolute roots only, and an atomic write through `fs-kit`, which also gives canonical paths without the Windows `\\?\` prefix and case-folded prefix checks. Eight more tests cover the round trip, a missing file, a relative root, unknown keys and bad TOML, an oversized file, the root cap, the default path and the schema.

### T5. Return an error for the largest invalid line index

TextBuffer::line must reject every out-of-range index without panicking or wrapping its diagnostic range.

Execution plan: add a usize::MAX regression in buffer.rs; use the standard saturating integer operation for the diagnostic end; run the crate tests, clippy and formatting. Existing range checks remain authoritative; no shared helper or dependency is needed for a single integer operation.

Validation: the regression panicked on overflow before the fix. All 44 tests pass under nextest; clippy with -D warnings and formatting checks pass. Reused the standard usize::saturating_add operation and preserved ordinary diagnostic ranges.
