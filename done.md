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

### T4. IDE permissions in `cox-plugin-api`

Split when claimed (500-line task cap). Add the IDE permission set and the manifest tables for language servers and debug adapters, as T1 defines them, to cox's `cox-plugin-api`, its JSON Schemas and its docs. The plugin declares a process; the host spawns it, as with `[[mcp]]` today.

The work happens in the cox repository and gets a matching task in cox's `plan.md` when it starts. Blocked on T1 only for the final names; the permission families are already in `research.md` (buffer, LSP, DAP, index, terminal, git) and can be implemented against that list.

#### T4.1. IDE permission set

The buffer, LSP, DAP, index, terminal and git permission classes in `cox-plugin-api`, with validation and schema.

#### T4.2. LSP and DAP manifest tables

`[[language_servers]]` and `[[debug_adapters]]` (names as T1 settles them) in `PluginManifest`, JSON Schema, and the docs. The plugin declares the command; the host spawns it.

#### T4 outcome

MiMo's [pyrlyn/cox#367](https://github.com/pyrlyn/cox/pull/367) had already landed both subtasks in cox with names that differ from the first draft of `docs/architecture.md` §4: `[capabilities.ide]` with `ide.*` grant lines, and the LSP/DAP tables keyed by `languages` (plus `globs`, `env`, `config`). Those names were kept, so T6 needed no change. [pyrlyn/cox#385](https://github.com/pyrlyn/cox/pull/385) (cox T380) tightened least privilege: `ide.buffer` is `"read"` | `"edit"` and `ide.git` is `"read"` | `"write"` (stage and commit only), with grant lines `ide.buffer.read`, `ide.buffer.edit`, `ide.git.read`, `ide.git.write`; a language server needs `languages` or `globs` and a debug adapter needs `languages`. `docs/architecture.md` §3.4, §4, §7 and §8 decisions 6, 8 and 9 now describe what cox has.

Tests (in cox): `ide_caps_parse_per_family_and_validate`, `ide_buffer_and_git_refuse_a_flag_or_an_unknown_level`, `language_server_and_debug_adapter_need_something_to_route_on`, `widening_ide_buffer_or_git_needs_approval`, plus #367's table, schema and API-index drift tests; `cargo test -p cox-plugin-api -p cox-plugin`, workspace clippy `-D warnings` and `fmt --check` green.

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

### T11. Language-server and debug-adapter binaries through ketch

Install and update language-server and debug-adapter binaries from GitHub releases with checksums, through ketch. Done when a test installs a pinned binary from a local fixture release, rejects a bad checksum, and the core resolves the installed path.

Plan:

1. Study ketch (read-only): library crate or CLI only, how it verifies checksums, how it can be pointed at a local release.
2. Add a crate: a pinned binary spec (repo, tag, per-target asset and sha256, binary name), install through ketch into a Kerf-owned root, and `resolve` returning the installed binary path the core spawns.
3. Treat every download as untrusted: cap the download and the installed size, hold the asset to the pinned sha256 before anything is unpacked, refuse a binary that resolves outside the root.
4. Tests against a local fixture release (no network): install a pinned binary, reject a bad checksum, resolve the path, refuse oversize downloads and installs.
5. Verify with `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`; update `toolchain.md`.

Result: `crates/kerf-binaries` drives the `ketch` CLI. `ketch-core` is a library but unpublished, pulls SQLite, octocrab/tokio and signature stacks, and is GPL-only, which would remove Kerf's royalty-free licence option. `Installer::install` writes a one-entry `ketch.lock` for the host target and runs `ketch sync`, the only CLI path that holds a download to a caller-chosen digest; ketch refuses a mismatch before unpacking. Downloads are capped through `KETCH_MAX_DOWNLOAD_MIB`; ketch has no unpack cap, so Kerf measures the installed tree and uninstalls it when over its cap. Six integration tests run the real ketch (from `mise.toml`, also installed in CI) against a loopback stand-in for the GitHub API.

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
