# T6. Plugin host on `wasm-plugin-host`

`crates/kerf-plugin`: load, activate and call WASM plugins through the shared `wasm-plugin-host` crate on cox's ABI (`cox:host/v1`, extism), not a third copy of cox's host. Manifest read without running code, lazy activation, a plugin crash disables the plugin and never the IDE (`docs/architecture.md`). The package layout depends on crates-packages#42. Done when a test plugin loads, activates on its trigger, answers a call, and a trapping plugin is disabled while the host keeps running.

Plan:

1. Depend on `wasm-plugin-host` and `cox-plugin-api` as git dependencies pinned by `rev` (`pyrlyn/crates`, `pyrlyn/cox`): neither is on crates.io, and the crates.io name `wasm_plugin_host` belongs to an unrelated crate, so the registry route in `rust.md` is closed.
2. `crates/kerf-plugin`: `manifest` (the one module that reads `plugin.toml` into cox's `PluginManifest` with `toml`, then `validate`), `trigger` (activation triggers derived from `[[language_servers]]` and `[[debug_adapters]]`, read without running code), `host` (discover through `wasm-plugin-host`, a gate for siblings T7/T10, lazy load and `cox_init` on the first matching trigger, calls through the control or event lane, three consecutive failures or a failed `cox_init` disable the plugin).
3. Tests with WAT fixtures: a manifest is read without loading wasm, a plugin activates only on its trigger, answers a call, a trapping plugin is disabled while another plugin keeps answering, project plugins are refused by the default gate, the notice list is capped.
4. Verify: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`. Rows in `toolchain.md`.
