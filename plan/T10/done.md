# T10. Plugin lockfile: done

Result: crate `crates/kerf-plugin-lock` with `Lockfile::read`, `write`, `pin`, `unpin` and `verify`; schema in `docs/schema/kerf-plugins.lock.schema.json`. The plugin host (T6) calls `verify` with the digest it computes through `wasm-plugin-host`; that wiring belongs to T6.

Commits: pyrlyn/kerf#17 (`d6c866f`).
