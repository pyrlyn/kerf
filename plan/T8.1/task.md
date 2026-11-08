# T8.1. Merge API and a second grammar in `kerf-semantic`

1. Tag every `Symbol` with its source (`TreeSitter` or `LanguageServer`). `Semantic::merge_lsp` takes `gen-lsp-types` document symbols and reference locations (`lsp-types` has had no release since 2024-06; `gen-lsp-types` is generated from the upstream schema and used by rust-analyzer and ruff), maps UTF-16 or UTF-8 positions to bytes, keeps only answers that land on an identifier with the claimed name, never replaces a tree-sitter symbol, caps answers per file, and drops a file's LSP symbols on its next edit (their positions are stale).
2. Python grammar and tags query from `tree-sitter-python`, picked by file extension; `tests/fixtures/recall.py` with its own labels and recall test.
3. The Rust fixture gains what tree-sitter cannot see (an item a macro defines, a function named as a value); the recall test reports tree-sitter alone and tree-sitter plus a scripted answer file `tests/fixtures/recall.lsp.json`.
4. Unit tests for hostile answers: out of range, wrong name, split UTF-8, over the cap.
