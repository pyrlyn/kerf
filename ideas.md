# Ideas

Not approved. Nothing here moves to `roadmap.md` or `plan.md` without the creator. Reasons are in `research.md`.

## Candidate tasks

- **Plugin host on `wasm-plugin-host`.** Reuse the shared crate rather than a third copy of cox's host. The package layout depends on crates-packages#42.
- **LSP results in the semantic layer.** Merge language-server answers where tree-sitter misses (macro bodies, type-resolved references). Add more languages.
- **Inspections with quick fixes.** A plugin export that runs over the core's index and returns edits, as in IntelliJ.
- **Workspace Trust.** An untrusted folder starts no language servers, debuggers or tasks until the user trusts it.
- **Plugin lockfile.** A committed lockfile of plugin ids, versions and package digests.
- **UI shell.** Slint through slint-bindings, or Flutter through slint_dart. Plugin UI from cox's `Widget` tree or from weft. Plugins redraw only when they ask to.
- **Remote development.** The plugin host and the language servers run next to the code; the UI stays local.
- **Language-server and debug-adapter binaries through ketch.** Install and update them from GitHub releases with checksums.
- **Local code completion.** A fill-in-the-middle endpoint in runa, used by the editor. Chat and agents go through `cox acp`.
- **Mobile.** An interpreter backend (wasm3 or WAMR) behind the same host API for iOS and Android, where wasmtime cannot JIT.
