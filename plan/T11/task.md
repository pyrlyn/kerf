# T11. Language-server and debug-adapter binaries through ketch

Install and update language-server and debug-adapter binaries from GitHub releases with checksums, through ketch. Done when a test installs a pinned binary from a local fixture release, rejects a bad checksum, and the core resolves the installed path.

Plan:

1. Study ketch (read-only): library crate or CLI only, how it verifies checksums, how it can be pointed at a local release.
2. Add a crate: a pinned binary spec (repo, tag, per-target asset and sha256, binary name), install through ketch into a Kerf-owned root, and `resolve` returning the installed binary path the core spawns.
3. Treat every download as untrusted: cap the download and the installed size, hold the asset to the pinned sha256 before anything is unpacked, refuse a binary that resolves outside the root.
4. Tests against a local fixture release (no network): install a pinned binary, reject a bad checksum, resolve the path, refuse oversize downloads and installs.
5. Verify with `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`; update `toolchain.md`.
