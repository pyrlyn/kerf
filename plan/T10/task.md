# T10. Plugin lockfile

A committed lockfile of plugin ids, versions and package digests. Load refuses a plugin whose digest does not match. Done when tests cover write, read, a digest mismatch and a missing entry.

Plan:

1. Reuse from cox: it has no lockfile, but it has the package digest (`cox-plugin` `package_digest`, the same function `wasm-plugin-host` carries: SHA-256 hex over the package tree). The lockfile stores that digest; it does not define a second one.
2. New crate `crates/kerf-plugin-lock`, the one module that reads and writes the lockfile: `format = 1` and a `[plugins.<id>]` table of `version` and `digest` (a table keyed by id makes a duplicate id a TOML error), TOML through `toml`, written atomically through `fs-kit`, sorted by id.
3. Validation in the types (`deny_unknown_fields`, a 64-hex digest newtype, `format` other than 1 refused); the file is repository input, so reads are capped at 1 MiB.
4. `Lockfile::verify(id, version, digest)` for the plugin host (T6) to call before it loads a package; mismatch and missing entry are typed errors.
5. JSON Schema committed under `docs/` and checked by a test through `config-schema`.
6. Tests (20): write, read, digest mismatch, version mismatch, missing entry, malformed TOML, unknown key, duplicate id, bad format, bad digest, oversized and non-UTF-8 file, stale schema. Verify with `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`.
