# T13. Remote development

The plugin host and the language servers run next to the code; the UI stays local. First slice: a versioned UI ↔ host protocol and a headless host process that owns buffers and the semantic layer, over any byte stream (stdio, SSH). Every message and queue from the remote side is capped. Done when tests drive open, edit, undo and a symbol query through an in-memory transport and the protocol refuses an unknown major version.

Execution plan:

1. `research.md` §6: how Zed's remote server, VS Code Remote - SSH and `cox acp` split UI and host, with primary sources; pick the wire format and the framing crate.
2. New crate `crates/kerf-remote` (crate map row in `docs/architecture.md`): JSON-RPC 2.0, one JSON object per line (ACP's stdio framing). Framing through `tokio-util`'s `LinesCodec` with a maximum line length, envelopes through `jsonrpsee-types`; no hand-written framing.
3. `initialize` carries the protocol `{ major, minor }`; an unknown major is refused with a typed error and the connection ends. Methods: `buffer/open|read|edit|undo|redo|close`, `index/lookup`.
4. Headless `Host` owns `kerf-core` buffers and `kerf-semantic` under a workspace root. Sans-IO `Connection` (bytes in, bytes out) with caps on message size (both directions), inbound queue depth, open buffers and buffer size; `serve(Read, Write)` drives it over any byte stream; `kerf-host` binary serves stdio, which SSH carries.
5. Tests through an in-memory transport: open, edit, undo, symbol query, unknown major refused, oversized message refused, full queue answers busy.
6. Verify: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`.
