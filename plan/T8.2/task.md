# T8.2. LSP client with a scripted server in `crates/kerf-lsp`

1. JSON-RPC over stdio with `Content-Length` framing, as `docs/architecture.md` §5 says: 4 MiB message cap (kill the process on abuse), a bounded inbound queue that drops notifications with a counter, an 8 KiB stderr ring, a request timeout, and `shutdown`/`exit` with a 2 s budget before kill.
2. The process starts through a small `Spawn` trait, so T7's trust check can wrap it later.
3. A scripted fake server binary replays `recall.lsp.json`; an end-to-end test runs it through the client into `kerf-semantic` and checks the same recall gain.
4. Verify: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`; numbers in the commits.
