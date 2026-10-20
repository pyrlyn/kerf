# Kerf

https://github.com/pyrlyn/kerf

An IDE with a small Rust core and sandboxed WASM plugins. Language servers and debug adapters run as separate processes the core starts. See `research.md`.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T15 | in progress | P3 | 5 | 70% | Claude / opus-5.5 |


### T15. Mobile plugin backend

An interpreter backend (wasm3, WAMR or a maintained pure-Rust interpreter, chosen against each other) behind the same host API, for iOS and Android where wasmtime cannot JIT. Depends on T6. Done when the T6 plugin tests pass unchanged against the interpreter backend on the desktop host, and the choice and its limits are recorded in `research.md`.

Plan (approved by the creator: wasmtime's Pulley interpreter through a `pulley` feature of the shared host):

1. pyrlyn/crates T20.6 (pyrlyn/crates#107): feature `pulley` on `wasm-plugin-host` and `plugwright-extism` hands extism a `wasmtime::Config` targeting Pulley; their suites pass with it on.
2. Kerf: bump the `wasm-plugin-host` git pin to the merged commit; `kerf-plugin` feature `interpreter` forwards to `wasm-plugin-host/pulley`, and iOS and Android targets turn it on.
3. CI runs `cargo test -p kerf-plugin --features interpreter` beside the default run; `crates/kerf-plugin/tests/plugins.rs` stays unchanged.
4. `research.md` §8: candidates with primary sources, the choice and its limits; `toolchain.md` row.
