# T16.3. Remove the GPU renderer: done

Result: `crates/kerf-render` and the direct `wgpu`, `glyphon` and `pollster` dependencies are gone, with their `toolchain.md` rows, the lavapipe CI step and the architecture row; the workspace builds and its tests, clippy and fmt pass without them.

Commits: pyrlyn/kerf#35 (`c72891a`).
