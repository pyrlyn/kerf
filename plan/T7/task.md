# T7. Workspace Trust

An untrusted folder starts no language servers, debuggers or tasks until the user trusts it. A trust store in the core, a check every process spawn goes through, and denial as a typed error. Done when tests show a spawn refused in an untrusted folder and allowed after trust is granted.

Plan:

1. Add a `trust` module to `crates/kerf-core` (the card says "in the core"; `kerf-process` is still only proposed): an in-memory `TrustStore` of canonical folder roots, where a trusted root covers its subfolders (VS Code and Zed both do this).
2. One check, `TrustStore::authorize(cwd, kind)`, returns a `SpawnPermit` that only it can build, so a future `kerf-process` spawn takes the permit and cannot skip the check. Denial is a typed `TrustError`. Paths are canonicalized so `..` and symlinks cannot reach a trusted root's prefix.
3. `revoke` returns the roots it removed so a supervisor can stop their processes (§5.3). Hard cap on stored roots.
4. Persistence: a user-level `trust.toml` in the OS config directory (architecture decision 7), owned by one `trust::file` module with a committed schema.
5. Tests: spawn refused while untrusted, allowed after trust, refused after revoke; subfolder, sibling-prefix and `..` cases; the cap. Verify with `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
