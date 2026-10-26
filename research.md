# Kerf — research

How existing IDEs and editors split the core from plugins, and what Kerf takes from each. All sources were checked on 2026-10-09 unless a version is given. Facts without a primary source are marked **unverified**.

## 1. Conclusion

- **Plugin contract:** take the coding agent's (`agent-plugin-api`, `agent-plugin`) as is: manifest, ABI, grants, install flow, SDK, templates. Extend the permission set for the IDE: buffer, LSP, DAP, index, terminal, git.
- **Core:** rendering, text buffer, event loop and Kerf's own semantic layer: tree-sitter, an incremental index, a PSI-like code model.
- **Languages and debugging:** LSP and DAP servers run as separate processes. The core starts them on a plugin's declaration; the plugin never spawns them.

No surveyed product has all three. Zed comes closest (WASM sandbox, capabilities, built-in LSP and DAP), but it is not a small core and its extensions cannot draw UI. IntelliJ has the strongest code analysis, but its plugins run inside the IDE without process isolation. Lite XL and Pragtical are the smallest, but they have no sandbox and keep most of the editor in Lua.

## 2. Survey

| Product | Plugin model | Isolation | LSP / DAP | Download size | Fits Kerf |
| --- | --- | --- | --- | --- | --- |
| Zed 1.23.2 | WASM components (`wasm32-wasip2`), versioned WIT | Sandbox; user-side capability restrictions | Both built in; extensions supply launch commands | 113 MiB dmg (macOS arm64), 120 MiB tar.gz (Linux x86_64) | Closest. Too big for "small core"; extensions draw no UI |
| VS Code | JS/TS in an extension host | Separate host process, shared by extensions | Via extensions | not checked | Manifest, lazy activation, Workspace Trust |
| IntelliJ Platform (IDEA, CLion, Rider) | JVM plugins | Per-plugin class loader only | Own PSI and indexes | not checked | Analysis model only; isolation does not fit |
| Visual Studio (VisualStudio.Extensibility) | .NET, out of process | ServiceHub process over RPC | — | 2.5–210 GB installed | Versioned RPC contracts, crash isolation |
| Lapce 0.4.6 | WASI (wasmtime 14) | WASM sandbox in the proxy | Via plugins, run in the proxy | not checked | Plugin host next to the code in remote work |
| Neovim 0.12.5 | Lua | None | Built-in LSP client | not checked | Lockfile, lazy loading |
| Lite XL 2.1.8 | Lua | None | Plugins | 2.0 MiB (Linux portable), 2.2 MiB (Windows zip), 4.2 MiB (macOS arm64) | Smallest. Buffer and loop are in Lua, not C |
| Pragtical 3.13.1 | Lua (Lite XL fork) | None | Plugins | 5.7 MiB (macOS arm64), 5.9 MiB (Linux portable), 12 MiB (Windows zip) | Small core |
| Zellij | WASM/WASI | WASM sandbox | — (terminal multiplexer) | not checked | Plugin decides when to re-render |
| Helix 25.07.1 | none on master | — | Built-in LSP and DAP | not checked | No plugin model to reuse |
| JetBrains Fleet | — | — | — | discontinued | Not usable |

### Sources

**Zed** (main branch, v1.23.2, 2026-10-07)
- Extensions are Rust compiled to `wasm32-wasip2` (WASI component model): https://zed.dev/docs/extensions/developing-extensions
- The WIT interfaces are versioned in `crates/extension_api/wit/`: `since_v0.0.1` through `since_v0.8.0` (there is no 0.7.0). `process` arrived in 0.3.0, `context-server` in 0.5.0, `dap` in 0.6.0: https://github.com/zed-industries/zed/tree/main/crates/extension_api/wit
- Stable and preview builds cap at `since_v0_6_0::MAX_VERSION`; 0.8.0 is dev and nightly only: https://github.com/zed-industries/zed/blob/main/crates/extension_host/src/wasm_host/wit.rs
- The latest published `zed_extension_api` is 0.7.0 (2025-09-12): https://crates.io/api/v1/crates/zed_extension_api
- `language-server-command` returns `{command, args, env}`. `get-dap-binary` returns the debug adapter binary: https://github.com/zed-industries/zed/blob/main/crates/extension_api/wit/since_v0.8.0/extension.wit, https://github.com/zed-industries/zed/blob/main/crates/extension_api/wit/since_v0.8.0/dap.wit
- Extensions can also call `process.run-command`, `download-file` and npm install themselves.
- Capabilities are `process:exec`, `download_file` and `npm:install`. The user restricts them with `granted_extension_capabilities`; the defaults are wildcards: https://zed.dev/docs/extensions/capabilities
  - Unlike the coding agent, these grants are a user setting, not permissions declared in each extension's manifest.
- "Extensions cannot draw custom UI": **unverified** as a statement. The docs list only languages, debuggers, themes, icon themes, snippets, MCP servers and slash commands, and the WIT has no UI interface.
- Zed implements the DAP client: https://zed.dev/docs/debugger
- Worktree trust (Restricted Mode): every worktree starts restricted, and project settings, language-server and MCP-server installs and spawns wait for trust. Trusting a directory covers all its subdirectories; manual grants persist across restarts per host (checked 2026-10-19): https://zed.dev/docs/worktree-trust
- Sizes are release assets, compressed: `Zed-aarch64.dmg` 118,562,674 B and `zed-linux-x86_64.tar.gz` 125,626,819 B (https://github.com/zed-industries/zed/releases/tag/v1.23.2). The installed size is **unverified**.

**VS Code**
- `contributes` and `activationEvents` are static JSON in `package.json`: https://code.visualstudio.com/api/references/contribution-points, https://code.visualstudio.com/api/references/activation-events
  - Since 1.74, contributed commands, languages and views activate the extension without an explicit event.
  - That the manifest is read without running extension code follows from the format being static JSON; the docs do not say it outright (inference).
- Workspace Trust: Restricted Mode disables debugging and the extensions that did not opt in: https://code.visualstudio.com/docs/editing/workspaces/workspace-trust
  - Trusting a parent folder trusts all its subfolders; trust is revoked from the Workspace Trust editor; in a multi-root workspace an untrusted added folder puts the whole workspace in Restricted Mode (checked 2026-10-19).

**IntelliJ Platform**
- PSI is the platform's syntactic and semantic code model: https://plugins.jetbrains.com/docs/intellij/psi.html
- File-based indexes return files. Stub indexes are built over serialized stub trees and return PSI elements: https://plugins.jetbrains.com/docs/intellij/indexing-and-psi-stubs.html
- `LocalInspectionTool` runs per-file inspections while editing, and `LocalQuickFix.applyFix()` changes the PSI: https://plugins.jetbrains.com/docs/intellij/code-inspections.html
- Each plugin gets its own class loader: https://plugins.jetbrains.com/docs/intellij/plugin-class-loaders.html. "Same JVM, no process isolation" is **unverified**; only a support-forum post says so.

**Visual Studio**
- Extensions run in an external ServiceHub process and talk to the IDE over RPC contracts provided as brokered services. If an extension crashes or hangs, VS does not crash: https://learn.microsoft.com/en-us/visualstudio/extensibility/visualstudio.extensibility/get-started/oop-extensibility-model-overview?view=visualstudio (updated 2026-04-24)
- A brokered-service moniker is name plus version. A breaking change adds a new version and keeps the old one: https://learn.microsoft.com/en-us/visualstudio/extensibility/how-to-provide-brokered-service
- Some features are still missing from the out-of-process model and fall back to in-process: https://learn.microsoft.com/en-us/visualstudio/extensibility/visualstudio.extensibility/visualstudio-extensibility?view=visualstudio

**Lapce** (v0.4.6, master)
- Plugins are WASI on wasmtime 14: https://github.com/lapce/lapce/blob/master/lapce-proxy/src/plugin/wasi.rs
- The plugin catalog, LSP and DAP live in `lapce-proxy`: https://github.com/lapce/lapce/blob/master/lapce-proxy/src/dispatch.rs. That the proxy runs on the remote side follows from the remote-development design (inference).

**Neovim** (v0.12.5, 2026-08-23)
- `vim.pack` is the built-in plugin manager, still marked experimental: https://github.com/neovim/neovim/blob/v0.12.5/runtime/lua/vim/pack.lua
- Its lockfile is `nvim-pack-lock.json` (option `'packlockfile'`), and it should be committed: https://github.com/neovim/neovim/blob/master/runtime/doc/options.txt
- Lazy loading is not built in.
- `lazy.nvim` (third party, v11.17.5) adds `lazy-lock.json` and loading on events, commands, filetypes and keys: https://github.com/folke/lazy.nvim/blob/main/README.md

**Lite XL** (v2.1.8)
- The C side is the SDL window and events, the renderer with a render cache and FreeType, and Lua APIs for system, process, regex, dirmonitor and UTF-8: https://github.com/lite-xl/lite-xl/tree/master/src/api, https://github.com/lite-xl/lite-xl/blob/master/src/main.c
- The document buffer is Lua (`data/core/doc/init.lua`), and so is the event loop (`core.run`, `core.step` in `data/core/init.lua`).
  - This corrects the brief: Lite XL's native core is rendering plus OS services, not buffer and loop. Kerf keeps all three in Rust by its own choice.
- Sizes: https://github.com/lite-xl/lite-xl/releases/tag/v2.1.8

**Pragtical** (v3.13.1, 2026-10-02)
- A fork of Lite XL: https://pragtical.dev/docs/intro
- Sizes: https://github.com/pragtical/pragtical/releases/tag/v3.13.1. The site says it takes about 10 MB installed: https://pragtical.dev/

**Zellij**
- Plugins are WebAssembly/WASI. When `update` returns `true`, Zellij calls `render(rows, cols)`: https://zellij.dev/documentation/plugin-lifecycle.html

**JetBrains Fleet**
- No longer available for download from 2025-12-22: https://blog.jetbrains.com/fleet/2025/12/the-future-of-fleet/

**Helix**
- No plugin system on master (25.07.1, last commit 2026-09-29): https://github.com/helix-editor/helix
- The Steel plugin PR is an open draft, not merged: https://github.com/helix-editor/helix/pull/8675

### Other sizes

The brief could not check these.

| Product | Platform | Size | Source |
| --- | --- | --- | --- |
| Kate 26.04.2 | Linux Flatpak | 8.8 MiB download, 20.6 MiB installed, KDE runtime not counted | https://flathub.org/api/v2/summary/org.kde.kate |
| Geany 2.1 | Windows x64 / macOS arm64 | 26.9 MiB installer / 28.5 MiB dmg | https://github.com/geany/geany/releases/tag/2.1 |
| Sublime Text 4215 | macOS / Linux x64 / Windows x64 | 50.8 / 21.3 / 21.0 MiB | https://download.sublimetext.com/ |
| GNU Emacs 31.1 | Windows x64 installer | 55 MB | https://ftp.gnu.org/gnu/emacs/windows/emacs-31/ |
| Visual Studio 2026 | Windows | 2.5–210 GB installed, typically 20–50 GB | https://learn.microsoft.com/en-us/visualstudio/releases/2026/vs-system-requirements |

There is no official Emacs binary for macOS or Linux, and Kate publishes no size for Windows or macOS.

## 3. What Kerf takes

1. **Zed:** versioned WASM interfaces, and an extension that hands the core the launch command for a language server and a debug adapter.
   - Unlike Zed, Kerf takes no `process:exec` or `download_file` host calls, and grants are per-plugin manifest entries, not a user-wide setting.
2. **VS Code:** capabilities and activation triggers declared in the manifest and read without running plugin code, lazy activation, and Workspace Trust.
3. **Processes stay in the core.** Language servers, debuggers and builds run as separate processes the core starts. The coding agent already works this way: a plugin declares `[[mcp]]` and `[[external_agents]]` processes in its manifest, the host spawns them under its sandbox policy, and each one is a separate approval line (section 4).
4. **IntelliJ:** inspections with quick fixes, exported by plugins over the core's index.
5. **Visual Studio:** core APIs as versioned RPC contracts. A crash in an extension never takes down the IDE.
6. **Lapce:** in remote development, the plugin host runs next to the code.
7. **Neovim:** a committed lockfile (`nvim-pack-lock.json`) and lazy loading (as in `lazy.nvim`). **Lite XL** shows how small a native core can be; Kerf's core also holds the buffer and the event loop, which Lite XL keeps in Lua.
8. **Zellij:** the plugin decides when it needs to redraw.

## 4. Our repositories

Commits at the time of checking: crates-packages `77ba2b1`, the coding agent `5a17a0ef`, weft `0f91f26`, slint-bindings `4d61045`, slint_dart `12de634`, slint-flutter `4807387`, the package manager `3ca16c0`, pkgmgr-registry `3c3eeb1`, the token CLI `c43c86d92`, runa `02f9cc1`, cross-code `3d19c35`, stator `18c278f`.

### Plugin host: `wasm-plugin-host` (listepo/crates-packages, v0.1.0, MIT OR Apache-2.0)

- It is extism 1.30 on wasmtime 43. It loads core WASM modules, not components (`wasm-plugin-host/Cargo.toml:21-33`, `src/host.rs`).
- It runs one worker thread per plugin with two bounded lanes: control 16 and events 256. A full lane answers `Busy`. A watchdog cancels a call at its deadline (`src/host.rs:25-27`, `:307-329`).
- WASI is off, extism HTTP is compiled out, and memory is capped (16 MiB default, 64 MiB max). There is no fuel metering; deadlines use timeouts (`src/host.rs:167-172`).
- It has no manifest or permission model; those live in the coding agent.
- Wasmtime uses Cranelift, a JIT, so its native code cannot run on iOS (section "Mobile" below); §8 runs it on wasmtime's Pulley interpreter instead.
- The coding agent does not depend on it yet. the coding agent keeps a near-identical host in `apps/agent/crates/agent-plugin/src/host.rs:124`.
- PR #42, "package layout for discovery, staging and removal", is open, mergeable and +1099/−5 (https://github.com/listepo/crates-packages/pull/42, 2026-10-09). It ports the coding agent's on-disk layout as a generic module:
  - user plugins live at `<home>/plugins/<id>/versions/<digest12>/`, with `current` and `previous`;
  - project plugins live at `<project>/.<app>/plugins/<id>/`;
  - the package digest is a SHA-256 of the tree;
  - staging is atomic and refuses symlinks.
- GitHub Actions is off in that repository, so CI did not run on it.

### Contract: the coding agent (pyrlyn/cox)

- **Licences:** `agent-plugin-api` and the SDK are MIT OR Apache-2.0 (`crates/agent-plugin-api/Cargo.toml:8`, `plugins/Cargo.toml:13`). The host crate `agent-plugin` is under the coding agent workspace licence, GPL-3.0-or-later or a royalty-free licence (`Cargo.toml:15`). Copying host code into Kerf is bound by that licence.
- **Manifest:** `plugin.toml` with `deny_unknown_fields` (`crates/agent-plugin-api/src/manifest.rs:19-60`).
  - Fields: `api = 1`, an `id` matching `^[a-z][a-z0-9-]{1,23}$`, `version`, `wasm`, `[limits]` (`memory_mib`, `call_ms`) and `[capabilities]`.
  - Declarative tables: `[[provider]]`, `[[models]]`, `[[mcp]]`, `[[external_agents]]`, `[[agents]]`.
  - JSON Schemas are committed in `docs/plugin.schema.json` and `docs/plugin-abi.schema.json`.
- **Permissions** (`docs/design/plugins.md:203`):
  - `events:`, `hooks:`, `tools:`, `invoke:`, `net:<host>`, `fs.read:`/`fs.write:` (rooted at `$WORKSPACE` or `$PLUGIN_DATA`), `decide:` and `ui.render:`;
  - the flags `wasi`, `context`, `kv`, `ui.status`, `ui.panel`, `ui.overlay`, `ui.commands` and `ui.keys`;
  - `model:`, `provider:`, `mcp:`, `agent:` and `subagent:`.
  - Grants are bound to the package digest, so changed bytes prompt again (`crates/agent-plugin/src/grant.rs:33-50`).
- **ABI:** `the coding agent:host/v1`, JSON over extism (`crates/agent-plugin/src/hostfn.rs:43-58`).
  - Guest exports: `agent_init`, `agent_on_event`, `agent_hook`, `agent_tool_call`, `agent_command`, `agent_key`, `agent_render`, `agent_render_item`, `agent_decide`, `agent_provider_stream`.
  - Host functions: `agent_log`, `agent_notify`, `agent_kv_*`, `agent_context`, `agent_invoke_tool`, `agent_model_call`, `agent_http`, `agent_output`, `agent_cancelled`, `agent_redraw`.
  - Each host function is checked against the grant and the calling export.
  - Three consecutive failures disable an export (fail-open).
- **No process spawning by plugins:** no host function spawns a process (`docs/design/plugins.md:413`), and WASI is off (`crates/agent-plugin/src/host.rs:136`). Processes a plugin declares in `[[mcp]]` or `[[external_agents]]` are spawned by the host under `sandbox::Policy` (`crates/agent-plugin/src/external_agent.rs:1-20`). Kerf's LSP and DAP entries follow the same pattern.
- **Install:** `the coding agent plugin list|install|enable|disable|update|remove|new|link` (`crates/agent/src/cli.rs:407-506`).
  - The source can be a local directory, an `https` `.tar.gz` with `--sha256`, or `git+<url>` with `--rev`.
  - The package is staged under its digest, then `current` is swapped and one `previous` is kept for rollback (`crates/agent-plugin/src/install.rs`).
- **SDK and templates:** the Rust SDK `agent-plugin-sdk` with a `register!` macro (`plugins/sdk/src/lib.rs`), Rust and Dart templates (`plugins/templates/`), and examples in Rust, Dart and Kotlin.
- **UI:** a closed `Widget` tree, TUI-oriented, capped at 512 nodes, depth 8 and 16 KiB of text, with style tokens instead of raw colours (`crates/agent-plugin-api/src/ui.rs:14-18`, `:63`).

### UI

- **weft** (listepo/weft): an open format for UI that AI agents read, write, validate and patch (`README.md:3`). It is strict XML-like markup plus canonical JSON, a closed component catalog and no code in documents (`SPEC.md:12-20`).
  - The core is Rust, with a WASM build (`crates/weft-wasm`) and a Slint generator (`crates/weft-slint`).
  - Status: prototype. Whether it can describe plugin UI in an IDE is **unverified**.
- **slint-bindings** (listepo/slint-bindings): embeds Slint UIs into native hosts (NSView or SwiftUI, WinUI 3) through a custom `slint::platform` with a software renderer and a C ABI (`README.md:1-13`). Status: M1 spike.
- **slint_dart** (listepo/slint_dart): a Slint ↔ Dart/Flutter toolkit: core, interpreter, typed generator, AOT compiler, Skia, testing (`README.md:5-25`).
- **slint-flutter** (listepo/slint-flutter): a smaller package of the same idea with a `SlintView` widget. How it relates to slint_dart is **unverified**.

### Marketplace: the package manager and pkgmgr-registry

- **the package manager** installs binaries straight from GitHub releases and checks their published checksums (`apps/pkgmgr/README.md:6-8`). It fits language servers and debug adapters, not plugin packages.
- **pkgmgr-registry:** one folder per package with a `the package manager.toml`. The only required field is `source = "github:owner/repo"`. Optional fields: `bin`, `provides` and `[asset]` globs (`packages/pkgmgr-registry/README.md:3-20`).
- A plugin registry for Kerf would need the coding agent package format (digest, grants), not the package manager format as is.

### Index: `the token CLI graph`

- It indexes tree-sitter tags (`tree-sitter` 0.25) into SQLite for 13 languages (`apps/tokcli/Cargo.toml:54`, `:141-155`).
- It has an optional LSP backend that runs rust-analyzer, clangd or typescript-language-server (`src/plugins/graph/lsp.rs:1-15`).
- **Correction to the brief:** reference recall is now 0.924 (97/105), not 0.351, as measured on 2026-10-04 (`apps/tokcli/research.md:148`, `tests/graph_truth.rs:238-245`).
  - 0.351 was the 2026-09-04 measurement. `README.md:405` and `docs/comparison.md:228` still quote it.
  - Definitions: 43/44 found, precision 1.000. The remaining misses are inside macro bodies.
  - The labelled set is small and Rust-heavy, so how well this generalises is **unverified**.
- It is still a start: macro bodies and type-resolved references need an LSP or Kerf's own semantic layer.

### AI

- **`the coding agent acp`:** an Agent Client Protocol server on stdio for editors. the coding agent can also act as an ACP client that drives external agents (`apps/agent/crates/agent/src/acp_cmd.rs:5-10`, `crates/agent-acp/src/lib.rs:6-8`).
- **runa:** a local LLM runner on llama.cpp with an OpenAI-compatible server (`/v1/chat/completions`, `/v1/messages`, `/v1/embeddings` and others, `apps/runa/crates/runa/src/serve.rs:139-144`).
  - It has no fill-in-the-middle or `/v1/completions` endpoint, so local code completion needs a new endpoint.

### Mobile: cross-code

- An Nx monorepo of NativeScript plugins wrapping WASM runtimes (`packages/cross-code/README.md:3-22`). Several of them work without a JIT: wasm3 (interpreter only), the WAMR interpreter, WasmKit (Swift) and Chicory (Java).
- They ship as NativeScript plugins, not as a Rust crate. A Rust core would embed the underlying runtime directly (for example wasm3 or WAMR, both in C).
- The repository does not document the iOS ban on JIT; that comes from Apple's rules, not checked here (**unverified**).

### Not a fit: stator

- An ahead-of-time compiler from TypeScript and JavaScript to native binaries (`apps/stator/README.md:5`). Its output is native code, which a WASM sandbox cannot confine.
- stator itself recommends a WASM plugin system or QuickJS-NG for extensible scripting (`apps/stator/NICHE.md:30-31`).

## 5. Open questions

- **Plugin ABI.** Core modules with JSON over extism (the coding agent, `wasm-plugin-host`) or WIT components (Zed, Lapce via WASI)? Taking the coding agent "as is" means core modules; versioned WIT would be a new contract. Settled by the creator on 2026-10-09: the coding agent's ABI as is.
- **Licence.** Settled by the creator on 2026-10-09: the same terms as the coding agent (GPL-3.0-or-later or royalty-free, plus commercial), so host code may be taken from the coding agent.
- **UI.** Plugin UI could be the coding agent's `Widget` tree, extended for the IDE, or weft. The window: settled in T12 — native hosts over a C ABI, with Kerf's own GPU editor renderer (§7).
- **Mobile.** Wasmtime needs a JIT. iOS and Android need an interpreter backend behind the same host API. Settled in T15: wasmtime's Pulley interpreter (§8).

## 6. Remote development (T13)

How other editors split the UI from the side that holds the code, and what Kerf's UI ↔ host protocol takes. Checked on 2026-10-19.

| Product | What stays local | What runs next to the code | Wire | Version rule |
| --- | --- | --- | --- | --- |
| Zed (remote server) | UI, tree-sitter highlighting, language models, unsaved changes | Project files, language servers, tasks, terminals; locally installed extensions are propagated to the server ([docs](https://zed.dev/docs/remote-development)) | Protobuf `Envelope`s, each behind a `u32` length prefix, read with no size cap (`crates/remote/src/protocol.rs`, [zed@20fa2fa8](https://github.com/zed-industries/zed/blob/20fa2fa80949/crates/remote/src/protocol.rs)); one SSH ControlMaster multiplexes the protocol, terminals and tasks ([docs](https://zed.dev/docs/remote-development)) | The server binary must match the client version exactly; Zed re-downloads or uploads it on mismatch ([docs](https://zed.dev/docs/remote-development)) |
| VS Code (Remote - SSH) | UI and UI extensions (themes, snippets, keymaps) | VS Code Server with workspace extensions, terminals, debugging ([remote extensions](https://code.visualstudio.com/api/advanced-topics/remote-extensions), [Remote - SSH](https://code.visualstudio.com/docs/remote/ssh)) | All traffic through the SSH tunnel ([Remote - SSH](https://code.visualstudio.com/docs/remote/ssh)); commands are the bridge between the two sides, arguments go through `JSON.stringify` ([remote extensions](https://code.visualstudio.com/api/advanced-topics/remote-extensions)) | Server must match the client version exactly ([remote extensions](https://code.visualstudio.com/api/advanced-topics/remote-extensions)) |
| the coding agent (`the coding agent acp`) | The ACP client (Zed, JetBrains) | The agent: `agent-core` folds `Submission`s into `Event`s; `the coding agent acp` maps that one stream to ACP (`apps/agent/docs/design/protocol.md`, the coding agent@df04e1fe) | ACP: JSON-RPC 2.0 over stdio, one message per line, no embedded newlines ([transports](https://agentclientprotocol.com/protocol/transports)); the coding agent uses the `agent-client-protocol` 2.0 crate (`apps/agent/Cargo.toml`) | `protocolVersion` is one integer, the major; an agent that does not support the client's major answers its latest, and the client should close ([initialization](https://agentclientprotocol.com/protocol/initialization)) |

What Kerf takes:

1. **Split.** Zed's: the host owns buffers, the semantic layer, and later the plugin host and language servers; the UI only renders and sends intent. Unlike Zed, Kerf's tree-sitter layer runs on the host, because `kerf-semantic` is the index plugins and inspections query (`docs/architecture.md` §2.1).
2. **Wire.** ACP's: JSON-RPC 2.0, one JSON object per line, over any byte stream (stdio of `ssh host kerf-host`). Line framing needs no length header to trust, and JSON stays readable in a log. Kerf does not take Zed's protobuf: Kerf's other contracts (the coding agent ABI, inspections) are JSON already.
3. **Versioning.** ACP's integer major plus a minor for additions. Unlike Zed and VS Code, Kerf does not demand an exact build match: a minor bump keeps the connection; an unknown major is refused with a typed error and the connection ends.
4. **Caps.** Zed reads a length prefix of up to 4 GiB into memory. Kerf caps every line before it is buffered, caps the requests queued per read, and caps replies too.

Crates for the wire (registry API, checked 2026-10-19):

| Crate | Version | Verdict |
| --- | --- | --- |
| [`tokio-util`](https://crates.io/crates/tokio-util) `LinesCodec` | 0.7.20 | **Taken** for framing. `new_with_max_length` errors on a longer line, then discards it up to the next newline and goes on ([source](https://github.com/tokio-rs/tokio/blob/tokio-util-0.7.20/tokio-util/src/codec/lines_codec.rs)). Its `Decoder` works on a `BytesMut` without a runtime, so the host stays synchronous. Already in the monorepo (the coding agent). |
| [`jsonrpsee-types`](https://crates.io/crates/jsonrpsee-types) | 0.26.1 | **Taken** for JSON-RPC 2.0 envelopes (`Request`, `Response`, `ErrorObject`, `Id`). Depends only on `http`, `serde`, `serde_json`, `thiserror`. |
| [`lsp-server`](https://crates.io/crates/lsp-server) | 0.10.0 | Rejected: `read_msg_text` resizes its buffer to any `Content-Length` and reads header lines without a limit ([source](https://github.com/rust-lang/rust-analyzer/blob/master/lib/lsp-server/src/msg.rs)), so a hostile peer chooses the allocation. |
| [`jsonrpsee`](https://crates.io/crates/jsonrpsee) | 0.26.1 | Rejected: an async client/server framework on tokio; the host needs only envelopes. |
| [`jsonrpc-core`](https://crates.io/crates/jsonrpc-core) | 18.0.0 | Rejected: last release 2021-07-20. |

## 7. UI shell (T12)

Checked on 2026-10-19. Repository facts are at slint-bindings `4d61045` (which vendors slint_dart under `flutter/`); crate facts come from the crates.io API on that date.

### Direction (the creator, 2026-10-20): Slint for the editor

The creator changed the editor surface on 2026-10-20: the code editor is a **Slint** component embedded in the native host through **slint-bindings** (`EmbeddedHost<C: ComponentHandle>` in `slint-bindings-core`, `crates/slint-bindings-core/src/host.rs` at slint-bindings `9c1b353`), not Kerf's own GPU renderer. The native window, menus, input and the "Look" rules below stay. The GPU renderer (`kerf-render`) is removed once the Slint surface replaces it (plan.md T16). The "Editor renderer" and "Licence" subsections below record the 2026-10-19 choice that this replaces; the dependency and licence facts for Slint are in "Slint dependency and licence (T16.1)" at the end of this section.

slint-bindings renders on the CPU into a host-owned RGBA buffer and calls itself an "M1 spike" (`README.md` at `9c1b353`); its GPU path is its own milestone M3 (`plan.md` at `9c1b353`).

### Direction (the creator, 2026-10-19)

Kerf uses **native controls** and **its own GPU renderer for the code editor**:

- The window, menus, panels and dialogs are the platform's own: AppKit/SwiftUI on macOS, WinUI 3 (C#) on Windows, GTK 4 on Linux.
- Each native host talks to the Rust core through a **C ABI**, the pattern slint-bindings uses (`sb_*`, `README.md`).
- The editor surface is drawn by Kerf in Rust on the GPU, inside a native view, the way Zed draws its own editor.
- **macOS first.** Windows and Linux hosts are separate tasks.

This replaces the first comparison in this section (Slint from Rust against Flutter through slint_dart): both put a non-native toolkit around the whole window.

### Why not the toolkit paths

- **Slint from Rust** (winit backend): its own window with Slint widgets, not the platform's controls.
- **Slint through slint-bindings**: native host, but the embedded surface is Slint, rendered on the CPU into a host-owned RGBA buffer (`README.md`, "How a frame happens"). The creator chose a Kerf renderer without Slint.
- **Flutter through slint_dart**: Flutter owns the window and draws its own widgets; Dart and the Flutter SDK join the toolchain (slint_dart `README.md`; [Flutter Linux setup](https://docs.flutter.dev/platform-integration/linux/setup) adds GTK 3 and a C++ toolchain).

### Editor renderer

| Option | Version (crates.io) | Notes |
| --- | --- | --- |
| `wgpu` | 30.0.1, 2026-08-22 | Metal on macOS, D3D12 on Windows, Vulkan/GL on Linux; a surface is created from a raw window handle ([`SurfaceTargetUnsafe::RawHandle`](https://docs.rs/wgpu/30.0.1/wgpu/enum.SurfaceTargetUnsafe.html)), which on macOS is the host's `NSView`. |
| `cosmic-text` | 0.19.0, 2026-04-22 | Shaping, line layout and font fallback, the same results on every OS. |
| `glyphon` | 0.12.0, 2026-07-09 | Glyph atlas and text pipeline for wgpu on top of cosmic-text; depends on `wgpu ^30` and `cosmic-text ^0.19` (its 0.12.0 dependency list), so the three move together. |
| `vello` | 0.11.0, 2026-10-02 | Linebender's 2D renderer. Its README calls the compute renderer "experimental" and the GPU sparse-strips renderer one that becomes primary "as it matures" ([vello README](https://github.com/linebender/vello)); not yet. |
| `gpui` | 0.2.2, 2025-10-22 | Zed's framework owns its platform layer and window; it is not a surface to put inside another app's view. |

**Choice:** `wgpu` + `cosmic-text` + `glyphon`, in a Rust crate that implements `kerf_core::Render` and draws into the surface of the native view. Text, cursor and edits stay in `kerf-core` (`Editor`, T12.1). The native host owns input and forwards it: on macOS an `NSView` that adopts `NSTextInputClient`, so IME and dead keys come from the system.

Open, checked when the renderer task runs (**unverified** here): which software adapter CI runners offer for an offscreen render test (WARP on Windows, lavapipe on Linux, Metal on the macOS arm64 runner), and how a WinUI `SwapChainPanel` hosts a wgpu surface.

### Look: Xcode on macOS, Visual Studio on Windows

The creator's rule (2026-10-19): the code editor looks native — on macOS like Xcode's editor, on Windows like Visual Studio's. The renderer therefore hard-codes no font, colour or caret; the native host hands it a theme taken from the platform, and draws the platform's own pieces where the platform offers them.

| | macOS (Xcode) | Windows (Visual Studio) |
| --- | --- | --- |
| Font | SF Mono, which Apple ships for "coding environments like Xcode" ([Apple Fonts](https://developer.apple.com/fonts/)) | Cascadia Mono, the Visual Studio 2022 default ([Fonts and colors](https://learn.microsoft.com/en-us/visualstudio/ide/how-to-change-fonts-and-colors-in-visual-studio?view=vs-2022)) |
| Syntax colours | The user's Xcode theme, read at run time: Xcode 27.1 bundles `Default (Light).xccolortheme`, `Default (Dark).xccolortheme` and others under `DVTUserInterfaceKit.framework/Versions/A/Resources/FontAndColorThemes/` (observed on this machine, Xcode 27.1 build 27A9275; the file format and the user theme folder `~/Library/Developer/Xcode/UserData/FontAndColorThemes` are **unverified** — Apple does not document them). Kerf never copies Apple's files; without Xcode it falls back to its own theme. | Visual Studio's Light and Dark themes; where they are read from is **unverified** and belongs to the Windows host task. |
| Caret | The system's own `NSTextInsertionIndicator` view over the GPU surface ("a view that represents the insertion indicator in text", macOS 14.0+, [AppKit](https://developer.apple.com/documentation/appkit/nstextinsertionindicator)) | Drawn from the WinUI theme resources (Windows host task). |
| Selection | `NSColor.selectedTextBackgroundColor` ("the color to use for the background of selected text", [AppKit](https://developer.apple.com/documentation/appkit/nscolor/selectedtextbackgroundcolor)) | WinUI theme resources (Windows host task). |
| Light/Dark | Follows the system appearance and switches the Xcode Light/Dark theme with it. | Follows the Windows theme. |
| Key behaviour | `NSTextInputClient` with `doCommandBySelector:`, so the Cocoa text bindings Xcode users know (Option-arrows, Emacs-style Control keys) and IME come from the system. | Visual Studio's shortcuts, mapped in the host. |

### Licence

wgpu and cosmic-text are `MIT OR Apache-2.0`, glyphon `MIT OR Apache-2.0 OR Zlib` (crates.io metadata of the versions above), which fits all of Kerf's licences. (Superseded on 2026-10-20: Slint is back, see the next subsection.)

### Slint dependency and licence (T16.1)

Checked on 2026-10-20 against slint-bindings `9c1b353e114c83d5cb03cb61d504a166ed49efa1` (2026-10-18, `main`), Slint `v1.18.1` (tag commit `372cf0e`) and the crates.io API.

**The dependency.**

- `slint-bindings-core` is not on crates.io: the workspace sets `publish = false` and `license = "Apache-2.0"` ([`Cargo.toml`](https://github.com/pyrlyn/slint-bindings/blob/9c1b353e114c83d5cb03cb61d504a166ed49efa1/Cargo.toml)). Kerf takes it as a Cargo git dependency pinned by `rev`, like `wasm-plugin-host` and `agent-plugin-api`.
- It path-depends on two crates inside the same repository: `slint-embed` (`flutter/packages/slint/rust/embed`) and `slint-dart-interpreter` (`flutter/packages/slint_build/interpreter`, which inherits its versions from `flutter/Cargo.toml`). Cargo finds both in the git checkout: `cargo metadata` (cargo 1.99.0) on a probe crate and on this workspace resolved all three to `git+https://github.com/pyrlyn/slint-bindings?rev=9c1b353…`. Cargo's rule for this: a git dependency's packages are searched for in the whole repository ([The Cargo Book, "Specifying dependencies from git repositories"](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#specifying-dependencies-from-git-repositories), checked 2026-10-20). Both crates are Apache-2.0 (`flutter/packages/slint/LICENSE`, `flutter/packages/slint_build/LICENSE` at that commit).
- Exact pins it forces ([`Cargo.toml`](https://github.com/pyrlyn/slint-bindings/blob/9c1b353e114c83d5cb03cb61d504a166ed49efa1/Cargo.toml) lines 23–33): `slint = "=1.18.1"` (`default-features = false`, `std`, `compat-1-18`, `renderer-software`), `slint-interpreter = "=1.18.1"`, `i-slint-core = "=1.18.1"` (`image-decoders`, `svg`). Kerf therefore pins `slint` and `slint-build` to `=1.18.1` too; a Slint upgrade means moving the `rev` first.
- On macOS and Windows it also turns on `slint`'s `renderer-femtovg-wgpu` and `unstable-wgpu-30` and adds `pollster = "0.4.0"` ([`crates/slint-bindings-core/Cargo.toml`](https://github.com/pyrlyn/slint-bindings/blob/9c1b353e114c83d5cb03cb61d504a166ed49efa1/crates/slint-bindings-core/Cargo.toml)). So `wgpu 30.0.1` stays in Kerf's macOS and Windows dependency tree through `slint-bindings-core` even after T16.3 drops Kerf's own `wgpu`, `glyphon` and `pollster`; `slint-interpreter` and the Slint compiler are in every build for the same reason.
- `EmbeddedHost::new` installs the embedding platform for the calling thread and checks the scale; `render` refuses a buffer shorter than `width × height × 4` ([`crates/slint-bindings-core/src/host.rs`](https://github.com/pyrlyn/slint-bindings/blob/9c1b353e114c83d5cb03cb61d504a166ed49efa1/crates/slint-bindings-core/src/host.rs)).

**The licence.** `slint` 1.18.1 and `slint-build` 1.18.1 are `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0` (crates.io API, versions published 2026-09-21).

- **Kerf under GPL-3.0-or-later.** Slint is taken under GPL-3.0-only. GPL-3.0-or-later code may be combined with GPL-3.0-only code; the combined binary is then distributed under GPL-3.0. Kerf's own sources keep their "or later" grant.
- **Kerf under its royalty-free or commercial licence.** Slint is taken under the [Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0](https://github.com/slint-ui/slint/blob/v1.18.1/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md) (identical to `LICENSES/LicenseRef-Slint-Royalty-free-2.0.md` in the 1.18.1 crate). It allows distributing Slint as part of a desktop, mobile or web application, modified or not (§1), on one attribution condition (§2): either the `AboutSlint` widget in an About screen reachable from the application's top-level menu (or the splash screen when there is none), or the "Made with Slint" badge on a public page, preferably the download page. Limits (§3): not alone outside an application, not in embedded systems, not in an application that exposes Slint's APIs, and licence notices in Slint's sources stay. The paid Slint Software License 3.0 is not needed for Kerf's desktop app.
- **What follows for Kerf.** The macOS app shows `AboutSlint` in its About window, or the download page carries the badge; that belongs to the host task that adds the About window. Kerf's C ABI and plugin API must not hand Slint's own API (components, `.slint` source, properties) to plugins or other programs. Kerf's licences cannot sublicense Slint: anyone who builds on Kerf under Kerf's royalty-free or commercial licence takes Slint under one of Slint's own licences. None of this blocks T16.

## 8. Mobile plugin backend (T15)

Checked on 2026-10-20. Kerf's plugins are extism modules (the coding agent's ABI, §4) loaded by `wasm-plugin-host`. On iOS an app cannot map writable and executable memory: the entitlement that allows `MAP_JIT` lists macOS only (https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.cs.allow-jit, platforms: macOS 10.7+). So wasmtime's Cranelift output cannot run there, and the backend has to interpret.

### Candidates

| Candidate | State | Fit for Kerf's extism plugins |
| --- | --- | --- |
| wasmtime Pulley | Ships inside wasmtime 49 (feature `pulley`, `Config::target("pulley64")`); wasmtime calls it "a portable implementation of executing WebAssembly code" that runs wherever wasmtime builds, and uses it when Cranelift has no native backend (https://docs.wasmtime.dev/stability-platform-support.html, 2026-10-20) | Same engine extism already runs: extism 1.41.0 `PluginBuilder::with_wasmtime_config` keeps the caller's target and overrides only epoch interruption, debug info, coredump, profiler and the tail-call, function-references, GC and exception proposals (extism 1.41.0 `src/plugin.rs:58-70`, `src/plugin_builder.rs:195-209`). No new dependency, no second host. |
| wasmi | v2.0.0, 2026-09-01 (https://github.com/wasmi-labs/wasmi/releases) | Pure-Rust interpreter, but extism has no wasmi host: extism 1.41.0 depends on wasmtime (`Cargo.toml`). Kerf would re-implement extism's kernel linking, memory and host-function ABI on wasmi. `plugwright-wasmi` (pyrlyn/crates `rust/plugwright/crates/plugwright-wasmi`, 8056429) speaks plugwright's own `pw_*` ABI, not extism's. |
| WAMR | WAMR-2.4.5, 2026-06-29 (https://github.com/bytecodealliance/wasm-micro-runtime/releases) | C runtime with an interpreter; the same extism re-hosting as wasmi, plus a C build for each mobile target. |
| wasm3 | README: "Wasm3 will enter a minimal maintenance phase"; no releases listed (https://github.com/wasm3/wasm3, 2026-10-20) | Unmaintained by Kerf's rule; excluded. |

### Choice

Pulley, through feature `pulley` of the shared host (pyrlyn/crates T20.6, pyrlyn/crates#107; the same feature on its successor `plugwright-extism`). With it on, `PluginHost::load` passes extism a `wasmtime::Config` whose target is `pulley64` (`pulley32` on 32-bit targets). The host API is unchanged. `kerf-plugin` forwards it as feature `interpreter`, and iOS and Android targets turn it on through a target-specific dependency (`crates/kerf-plugin/Cargo.toml`).

Evidence, on this Mac (aarch64-apple-darwin):

- Kerf's T6 tests (`crates/kerf-plugin/tests/plugins.rs`, unchanged) pass with `--features interpreter`; CI runs them on every target.
- The shared host's whole suite passes with `pulley` on, including the deadline (epoch interruption), the memory cap and a module that uses exception handling; a test asserts that the engine built from the config `is_pulley()`.
- `cargo check -p kerf-plugin` succeeds for `aarch64-apple-ios` and for `aarch64-linux-android` (NDK 30.0.15729638), with `pulley` on through the target dependency.
- A scratch host on extism 1.41.0 + wasmtime 49.0.3: a 50-million-iteration loop took 14.7 ms native and 252.8 ms on Pulley (release build), about 17× slower.

### Limits

- Speed: interpreted code is roughly an order of magnitude slower than Cranelift's (measured above); wasmtime states Pulley "will never be as fast as native Cranelift backends" (platform-support page above). Kerf plugins answer short calls, and their deadlines and the call budget apply as on desktop.
- Load time: Cranelift still compiles each module, to Pulley bytecode, on the device at every load, because the host disables extism's compilation cache. Precompiling (`Engine::precompile_module`) is not wired; it would need a signed cache, since loading precompiled code is `unsafe`.
- Both mobile builds take the interpreter. Whether Android apps may map executable memory for a JIT was not checked against a primary source (**unverified**); the interpreter is the safe default there and the target line in `kerf-plugin/Cargo.toml` is the one place to change.
- Not yet run on a device or simulator: Kerf has no iOS or Android host. Only `cargo check` for both targets and the desktop tests above.
- wasmtime lists iOS and Android as supported but "less well tested" (platform-support page above).
### Linux host (T17)

Checked on 2026-10-20. The creator's choices that day: the host is Rust on gtk4-rs calling `kerf-ffi`'s C functions (not C over `kerf.h`), and the look is GNOME Text Editor's on plain GTK 4, without libadwaita.

**Bindings and GTK version.**

- `gtk4` 0.11.5 (2026-09-20, MIT, rust-version 1.92; crates.io API) from [gtk-rs/gtk4-rs](https://github.com/gtk-rs/gtk4-rs). Its `gtk4-sys` 0.11.5 asks pkg-config for GTK ≥ 4.0 and raises the floor per feature (`v4_2` … `v4_24`, `[package.metadata.system-deps]` in the crate's `Cargo.toml`).
- `ubuntu-latest` is Ubuntu 24.04 ([actions/runner-images README](https://github.com/actions/runner-images), main on 2026-10-20), whose `gtk4` source package is 4.14.5 in noble-updates (Launchpad API, `getPublishedSources`). Kerf turns on `v4_14` and nothing newer, so the host builds on the CI runner and on the Ubuntu 24.04 Lima VM.
- The rejected alternative, C with Meson over `kerf.h`, needs manual memory management and a second build tool; gtk4-rs keeps the host in the Cargo workspace and still crosses only the C ABI.

**Showing the frame.** `gdk_memory_texture_new(width, height, format, bytes, stride)` makes a texture from a blob of pixels ([GdkMemoryTexture.new](https://docs.gtk.org/gdk4/ctor.MemoryTexture.new.html), GTK docs 4.24.1), and `GDK_MEMORY_R8G8B8A8_PREMULTIPLIED` is 4 bytes, red, green, blue, alpha, premultiplied ([GdkMemoryFormat](https://docs.gtk.org/gdk4/enum.MemoryFormat.html)), exactly what `kerf_view_read_rgba` writes. The widget appends it in its `snapshot` at its logical size, so at scale 2 the 2× frame maps pixel for pixel. A test reads the texture back with `gdk_texture_download`, whose data is `CAIRO_FORMAT_ARGB32` ([Texture.download](https://docs.gtk.org/gdk4/method.Texture.download.html)): premultiplied BGRA bytes on little-endian machines.

**Input.** A `GtkIMMulticontext` switches between input methods and is what GtkText and GtkTextView use ([IMMulticontext](https://docs.gtk.org/gtk4/class.IMMulticontext.html)). `gtk_event_controller_key_set_im_context` lets the key controller feed it ([EventControllerKey.set_im_context](https://docs.gtk.org/gtk4/method.EventControllerKey.set_im_context.html)); `::commit` brings "a complete input sequence", a single character or the end of a composition ([IMContext::commit](https://docs.gtk.org/gtk4/signal.IMContext.commit.html)); `set_cursor_location` tells the input method where the caret is, relative to the widget ([IMContext.set_cursor_location](https://docs.gtk.org/gtk4/method.IMContext.set_cursor_location.html)). With `set_use_preedit(FALSE)` the input method shows its feedback itself, for example in a window of its own ([IMContext.set_use_preedit](https://docs.gtk.org/gtk4/method.IMContext.set_use_preedit.html)): the host commits text as typed, as the macOS host does, until the core has a marked range.

**The caret.** GTK 4 has no caret widget, and `gtk_snapshot_render_insertion_cursor` is deprecated since 4.10 ([Snapshot.render_insertion_cursor](https://docs.gtk.org/gtk4/method.Snapshot.render_insertion_cursor.html)). The host therefore draws the caret over the frame with GTK's own rules, read from `GtkSettings` at run time:

- width `height × gtk-cursor-aspect-ratio + 1` (`stem_width` in [`gtk/gtkrenderlayout.c`](https://gitlab.gnome.org/GNOME/gtk/-/blob/4.14.2/gtk/gtkrenderlayout.c), 4.14.2); the ratio defaults to 0.04 ([gtk-cursor-aspect-ratio](https://docs.gtk.org/gtk4/property.Settings.gtk-cursor-aspect-ratio.html));
- GtkTextView's blink: the `gtk-cursor-blink-time` cycle (default 1200 ms, [gtk-cursor-blink-time](https://docs.gtk.org/gtk4/property.Settings.gtk-cursor-blink-time.html)) split evenly into visible, fading out, hidden, fading in; after an edit the first fade waits half a cycle; it stops after `gtk-cursor-blink-timeout` seconds (default 10, "reset after each user interaction", [gtk-cursor-blink-timeout](https://docs.gtk.org/gtk4/property.Settings.gtk-cursor-blink-timeout.html)) (`blink_alpha`, `add_blink_timeout`, `blink_cb` in [`gtk/gtktextview.c`](https://gitlab.gnome.org/GNOME/gtk/-/blob/4.14.2/gtk/gtktextview.c), 4.14.2);
- no caret while text is selected: GtkTextLayout adds no cursor when the buffer has a selection (`gtk/gtktextlayout.c` line 1880 at 4.14.2).

**The look: GNOME Text Editor.**

| | Value | Source |
| --- | --- | --- |
| Font | GNOME's `org.gnome.desktop.interface monospace-font-name`, default `'Adwaita Mono 11'` (it was `'Source Code Pro 10'` in 46.0); GNOME Text Editor uses it while its `use-system-font` is on (default true), else its `custom-font`, default `'Monospace 11'` | [gsettings-desktop-schemas](https://gitlab.gnome.org/GNOME/gsettings-desktop-schemas/-/blob/main/schemas/org.gnome.desktop.interface.gschema.xml.in) (main and 46.0); [gnome-text-editor 49.0 schema](https://gitlab.gnome.org/GNOME/gnome-text-editor/-/blob/49.0/data/org.gnome.TextEditor.gschema.xml) |
| Line height | `line-height` default 1.2, a factor on the font's own line (the host uses Pango's ascent plus descent) | gnome-text-editor 49.0 schema |
| Colours | style scheme default `'Adwaita'`. GtkSourceView 5.18.0 `Adwaita.xml`: text `dark_3` #504E55 on `light_1` #FFFFFF, cursor `dark_1` #77767B; `Adwaita-dark.xml`: text `light_5` #C0BFBC on `libadwaita-dark` #1D1D20, cursor `light_5` | gnome-text-editor 49.0 schema; [GtkSourceView styles](https://gitlab.gnome.org/GNOME/gtksourceview/-/tree/5.18.0/data/styles) |
| Selection | the schemes set none, so GTK's theme decides: `$selected_text_bg_color` is #3584E4 at 0.3 alpha (light) and `darken(#3584e4, 20%)` = #15539E at 0.8 alpha (dark) | [`gtk/theme/Default/_colors.scss`](https://gitlab.gnome.org/GNOME/gtk/-/blob/4.14.2/gtk/theme/Default/_colors.scss), 4.14.2 (darken computed in HSL) |
| Light or dark | GTK 4.14 has no public "is the theme dark" call; `gtk-interface-color-scheme` exists only from 4.20 ([Settings:gtk-interface-color-scheme](https://docs.gtk.org/gtk4/property.Settings.gtk-interface-color-scheme.html)). The host reads the widget's foreground colour (`gtk_widget_get_color`, 4.10+, [Widget.get_color](https://docs.gtk.org/gtk4/method.Widget.get_color.html)) and takes light text for a dark theme | — |
| Window | header bar with Open and a main menu; GNOME's shortcuts (Ctrl+O, Ctrl+Z, Shift+Ctrl+Z, Ctrl+A, Ctrl+W, Ctrl+Q) | [GNOME HIG, keyboard](https://developer.gnome.org/hig/reference/keyboard.html) (**unverified**: not re-read on this date) |

Pango sizes are points; GTK lays out at 96 dpi, so the host converts with ×96/72 (**unverified** for displays that set `gtk-xft-dpi`).

**Tests on a display.** `gtk::init` calls `gtk_init_check` and returns an error instead of exiting when there is no windowing system ([docs.rs gtk4 0.11.5 `init`](https://docs.rs/gtk4/0.11.5/gtk4/fn.init.html)). CI runs Xvfb on the Linux runner; locally the test runs in a Lima VM from `template:ubuntu-24.04`, the same GTK 4.14 as CI.
