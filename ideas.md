# Ideas

Not approved. Nothing here moves to `roadmap.md` or `plan.md` without the creator. Reasons are in `research.md`.

## Candidate tasks

- **UI shell.** Slint through slint-bindings, or Flutter through slint_dart. Plugin UI from cox's `Widget` tree or from weft. Plugins redraw only when they ask to.
- **Remote development.** The plugin host and the language servers run next to the code; the UI stays local.
- **Local code completion.** A fill-in-the-middle endpoint in runa, used by the editor. Chat and agents go through `cox acp`.
- **Mobile.** An interpreter backend (wasm3 or WAMR) behind the same host API for iOS and Android, where wasmtime cannot JIT.
