# Kerf

https://github.com/pyrlyn/kerf

An IDE with a small Rust core and sandboxed WASM plugins. Language servers and debug adapters run as separate processes the core starts. See `research.md`.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T1 | in progress | P0 | 3 | 100% | MiMo Desktop / mimo |
| T2.1 | done | P1 | 3 | 100% | MiMo Desktop / mimo |
| T2.2 | done | P1 | 3 | 100% | MiMo Desktop / mimo |
| T2.3 | done | P1 | 1 | 100% | MiMo Desktop / mimo |
| T4.1 | in progress | P1 | 3 | 10% | MiMo Desktop / mimo |
| T4.2 | in progress | P1 | 3 | 10% | MiMo Desktop / mimo |
| T6 | todo | P1 | 4 | 0% | |
| T7 | todo | P1 | 3 | 0% | |
| T8 | in progress | P2 | 5 | 5% | Claude Code / opus-5.5 |
| T9 | todo | P2 | 4 | 0% | |
| T10 | todo | P2 | 2 | 0% | |
| T11 | todo | P3 | 3 | 0% | |

### T1. Architecture document

The contracts every other task builds on, in `docs/architecture.md`. Done when the creator approves the document.

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

#### T2.1. Text buffer, edits, undo and redo

- the text buffer, on a maintained rope crate chosen against the alternatives;
- edits with undo and redo, emitting edit events (byte ranges and positions) that the semantic layer (T3) can consume.

Plan:

1. Pick the rope crate: compare maintained options on crates.io and in `rust.md`, and give the reason in the commit.
2. Write the buffer, edits, undo and redo with tests.
3. Define the edit-event type so T2.2 and T3 can consume it without depending on the buffer internals.

#### T2.2. Event loop with bounded queues

- a single-threaded event loop with bounded queues;
- back-pressure when a queue is full.

Plan:

1. Write the event loop and bounded queues against T2.1's edit events (or a stand-in type if T2.1 is not merged yet).
2. Tests for back-pressure and for a full queue answering rather than growing.

#### T2.3. CI and toolchain bookkeeping

- a GitHub Actions workflow for fmt, clippy and test on Linux x86_64, macOS arm64 and Windows x86_64;
- `toolchain.md` and `rust.md` rows for every crate T2 introduces.

Plan:

1. Add the workflow (Linux x86_64, macOS arm64, Windows x86_64 — no Intel Mac).
2. Add the crates to `toolchain.md`, and to `rust.md` if they are new.

### T4. IDE permissions in `cox-plugin-api`

Split when claimed (500-line task cap). Add the IDE permission set and the manifest tables for language servers and debug adapters, as T1 defines them, to cox's `cox-plugin-api`, its JSON Schemas and its docs. The plugin declares a process; the host spawns it, as with `[[mcp]]` today.

The work happens in the cox repository and gets a matching task in cox's `plan.md` when it starts. Blocked on T1 only for the final names; the permission families are already in `research.md` (buffer, LSP, DAP, index, terminal, git) and can be implemented against that list.

#### T4.1. IDE permission set

The buffer, LSP, DAP, index, terminal and git permission classes in `cox-plugin-api`, with validation and schema.

#### T4.2. LSP and DAP manifest tables

`[[language_servers]]` and `[[debug_adapters]]` (names as T1 settles them) in `PluginManifest`, JSON Schema, and the docs. The plugin declares the command; the host spawns it.

### T6. Plugin host on `wasm-plugin-host`

`crates/kerf-plugin-host`: load, activate and call WASM plugins through the shared `wasm-plugin-host` crate on cox's ABI (`cox:host/v1`, extism), not a third copy of cox's host. Manifest read without running code, lazy activation, a plugin crash disables the plugin and never the IDE (`docs/architecture.md`). The package layout depends on crates-packages#42. Done when a test plugin loads, activates on its trigger, answers a call, and a trapping plugin is disabled while the host keeps running.

### T7. Workspace Trust

An untrusted folder starts no language servers, debuggers or tasks until the user trusts it. A trust store in the core, a check every process spawn goes through, and denial as a typed error. Done when tests show a spawn refused in an untrusted folder and allowed after trust is granted.

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

### T9. Inspections with quick fixes

A plugin export that runs over the core's index and returns diagnostics with edits, as in IntelliJ. The contract (JSON in and out, versioned), a core-side runner that validates and caps what a plugin returns, and applying a fix as one undoable edit. Done when a test inspection reports a problem on a fixture and its fix applies and undoes.

### T10. Plugin lockfile

A committed lockfile of plugin ids, versions and package digests. Load refuses a plugin whose digest does not match. Done when tests cover write, read, a digest mismatch and a missing entry.

### T11. Language-server and debug-adapter binaries through ketch

Install and update language-server and debug-adapter binaries from GitHub releases with checksums, through ketch. Done when a test installs a pinned binary from a local fixture release, rejects a bad checksum, and the core resolves the installed path.
