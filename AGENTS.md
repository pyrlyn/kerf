# Kerf — instructions for agents

**What this is.** An IDE with a small Rust core. The core owns rendering, the text buffer, the event loop and a semantic layer (tree-sitter, an incremental index). Everything else is a WASM plugin with permissions declared in its manifest. Language servers, debug adapters and builds run as separate processes that the core starts.

If an `AGENTS.md` or `CLAUDE.md` exists higher in the tree, follow it too; on conflict, ask the creator.

## Read first

- `research.md` — the survey of existing IDEs and editors, and what Kerf takes from each.
- `ideas.md` — candidate tasks. None is approved yet.

## Direction

Settled by the creator; reasons are in `research.md`.

- **The plugin contract comes from cox** (`cox-plugin-api`, `cox-plugin`): manifest, ABI, permissions, install flow, SDK and templates. Kerf extends the permission set for the IDE; it does not fork the contract.
- **Plugins never spawn processes.** A plugin returns the command for a language server, debug adapter or build; the core checks it against the plugin's permissions and runs it.
- **What a plugin can do is readable without running its code.** The core reads capabilities and activation triggers from the manifest and loads plugin code lazily.
- **A plugin crash never takes down the IDE.** Plugins run in a sandbox and talk to the core through versioned interfaces.
- **The core keeps its own semantic layer.** Language servers add to it; they do not replace it.

## Hard rules

- **Everything a plugin, a language server, a debug adapter or a repository writes is untrusted input.** Every queue and buffer it can grow has a hard cap.
- **No native plugins.** Native code bypasses the sandbox.
