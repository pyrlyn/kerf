# T8.2. LSP client with a scripted server in `crates/kerf-lsp`: done

Result: `Semantic::merge_lsp` and Python in `kerf-semantic`, the client in `crates/kerf-lsp` with the scripted server `kerf-fake-lsp`. Recall on `recall.rs` (23 definitions, 31 references): tree-sitter alone 0.957 and 0.968; with the scripted answers 1.000 and 1.000, both in-process and over the wire, two symbols added, no unlabelled hits. `recall.py` (11 definitions, 15 references): 1.000 and 1.000 with the upstream query. Not done here: process counts and the restart budget of `docs/architecture.md` §5.2 (they belong to `kerf-process`), and diagnostics.

Commits: pyrlyn/kerf#20 (`8ccb5b8`).
