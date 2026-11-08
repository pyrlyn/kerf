# T16.3. Remove the GPU renderer

Delete `crates/kerf-render` and the `wgpu`, `glyphon` and `pollster` dependencies once nothing uses them, with their `toolchain.md` rows, the lavapipe CI step and the `docs/architecture.md` row. Done when the workspace builds and tests pass without them.

Plan:

1. Confirm with grep that no crate other than `kerf-render` names it, `wgpu`, `glyphon` or `pollster` (checked: only the root `Cargo.toml` does).
2. Delete `crates/kerf-render`; drop the `kerf-render`, `wgpu`, `glyphon` and `pollster` entries from the root `[workspace.dependencies]`.
3. `toolchain.md`: drop the three Crates rows and the Mesa lavapipe row. `ci.yml`: drop `mesa-vulkan-drivers` and `KERF_REQUIRE_GPU` (keep `libfontconfig1-dev`). `docs/architecture.md` §2.2: drop the `kerf-render` row and fix text that says kerf-ffi draws through it.
4. `rust.md` (workspace file) needs the creator's OK: ask in chat before touching it.
5. Verify with the existing suite (a removal has no behaviour of its own to test: it can only break what it was supposed to leave alone): `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo build --locked -p kerf-ffi`. `wgpu 30` and `pollster 0.4` stay in the macOS and Windows tree through `slint-bindings-core`: no direct dependency remains, the lockfile still has them.
