# T9. Inspections with quick fixes

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
