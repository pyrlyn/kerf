# Kerf

https://github.com/pyrlyn/kerf

An IDE with a small Rust core and sandboxed WASM plugins. Language servers and debug adapters run as separate processes the core starts. See `research.md`.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T1 | in progress | P0 | 3 | 0% | Claude Code / claude-opus-5-5 |
| T2 | in progress | P1 | 3 | 0% | Claude Code / claude-sonnet-5-5 |
| T3 | in progress | P1 | 4 | 0% | Claude Code / claude-opus-5-5 |
| T4 | todo | P1 | 3 | 0% | |

### T1. Architecture document

The contracts every other task builds on, in `docs/architecture.md`. Done when the creator approves the document.

It covers:

- the core boundary (rendering, buffer, event loop, semantic layer) and the crate map;
- the plugin lifecycle on cox's ABI as is (`cox:host/v1`, extism, `wasm-plugin-host`): manifest read without running code, lazy activation, crash and disable;
- the IDE permission set and the manifest tables for language servers and debug adapters, which are the input for T4;
- how the core starts, supervises, caps and stops LSP and DAP processes;
- Workspace Trust;
- interface versioning.

Plan:

1. Read `research.md`, cox's `docs/design/plugins.md`, `crates/cox-plugin-api/src/manifest.rs` and `wasm-plugin-host` (with crates-packages#42).
2. Write `docs/architecture.md`. Every borrowed idea links to its `research.md` section.
3. List the open points for the creator at the end of the document.
4. Verify: every permission and manifest table named in the document maps to a cox concept or is marked new.

### T2. Core skeleton

The first crate, `crates/kerf-core`, with no UI. Done when `cargo test`, `cargo clippy -- -D warnings` and `cargo fmt --check` pass locally and in CI.

It holds:

- the text buffer, on a maintained rope crate chosen against the alternatives;
- edits with undo and redo, emitting edit events (byte ranges and positions) that the semantic layer (T3) can consume;
- a single-threaded event loop with bounded queues.

Plan:

1. Pick the rope crate: compare maintained options on crates.io and in `rust.md`, and give the reason in the commit.
2. Write the buffer, edits, undo and redo with tests.
3. Write the event loop with bounded queues and tests for back-pressure.
4. Add a GitHub Actions workflow for fmt, clippy and test on Linux x86_64, macOS arm64 and Windows x86_64.
5. Add the crates to `toolchain.md`, and to `rust.md` if they are new.

### T3. Semantic layer: incremental parse and symbol index

`crates/kerf-semantic`. Done when an edit re-parses only the changed file incrementally, the index updates only that file's symbols, and a test measures definition and reference recall on a labelled Rust fixture.

It holds:

- tree-sitter parsing per file, with incremental re-parse from edit events (the same shape as T2's);
- a symbol index (definitions and references) from tree-sitter tags queries, updated per file.

Rust is the first language. Scope stays within the 500-line task cap; more languages and LSP merging are later tasks.

Plan:

1. Study `rtok graph` (`apps/rtok`, tree-sitter tags into SQLite) for reuse.
   - If its indexing can be shared, propose extracting it into a neutral crate in `packages/` and **ask the creator before touching rtok**.
   - Otherwise write the minimum here and record why in the commit.
2. Write incremental parsing behind an edit-event type that does not depend on `kerf-core`; T2 adapts to it later.
3. Write the per-file index with tests for insert, edit and delete.
4. Add a labelled fixture and a recall test, and report the numbers in the commit.
5. Add the crates to `toolchain.md`, and to `rust.md` if they are new.

### T4. IDE permissions in `cox-plugin-api`

Add the IDE permission set and the manifest tables for language servers and debug adapters, as T1 defines them, to cox's `cox-plugin-api`, its JSON Schemas and its docs. The plugin declares a process; the host spawns it, as with `[[mcp]]` today.

Blocked by T1. The work happens in the cox repository and gets a matching task in cox's `plan.md` when it starts.
