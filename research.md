# Kerf — research

How existing IDEs and editors split the core from plugins, and what Kerf takes from each. All sources were checked on 2026-10-09 unless a version is given. Facts without a primary source are marked **unverified**.

## 1. Conclusion

- **Plugin contract:** take cox's (`cox-plugin-api`, `cox-plugin`) as is: manifest, ABI, grants, install flow, SDK, templates. Extend the permission set for the IDE: buffer, LSP, DAP, index, terminal, git.
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
  - Unlike cox, these grants are a user setting, not permissions declared in each extension's manifest.
- "Extensions cannot draw custom UI": **unverified** as a statement. The docs list only languages, debuggers, themes, icon themes, snippets, MCP servers and slash commands, and the WIT has no UI interface.
- Zed implements the DAP client: https://zed.dev/docs/debugger
- Sizes are release assets, compressed: `Zed-aarch64.dmg` 118,562,674 B and `zed-linux-x86_64.tar.gz` 125,626,819 B (https://github.com/zed-industries/zed/releases/tag/v1.23.2). The installed size is **unverified**.

**VS Code**
- `contributes` and `activationEvents` are static JSON in `package.json`: https://code.visualstudio.com/api/references/contribution-points, https://code.visualstudio.com/api/references/activation-events
  - Since 1.74, contributed commands, languages and views activate the extension without an explicit event.
  - That the manifest is read without running extension code follows from the format being static JSON; the docs do not say it outright (inference).
- Workspace Trust: Restricted Mode disables debugging and the extensions that did not opt in: https://code.visualstudio.com/docs/editing/workspaces/workspace-trust

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
3. **Processes stay in the core.** Language servers, debuggers and builds run as separate processes the core starts. cox already works this way: a plugin declares `[[mcp]]` and `[[external_agents]]` processes in its manifest, the host spawns them under its sandbox policy, and each one is a separate approval line (section 4).
4. **IntelliJ:** inspections with quick fixes, exported by plugins over the core's index.
5. **Visual Studio:** core APIs as versioned RPC contracts. A crash in an extension never takes down the IDE.
6. **Lapce:** in remote development, the plugin host runs next to the code.
7. **Neovim:** a committed lockfile (`nvim-pack-lock.json`) and lazy loading (as in `lazy.nvim`). **Lite XL** shows how small a native core can be; Kerf's core also holds the buffer and the event loop, which Lite XL keeps in Lua.
8. **Zellij:** the plugin decides when it needs to redraw.

## 4. Our repositories

Commits at the time of checking: crates-packages `77ba2b1`, cox `5a17a0ef`, weft `0f91f26`, slint-bindings `4d61045`, slint_dart `12de634`, slint-flutter `4807387`, ketch `3ca16c0`, ketch-registry `3c3eeb1`, rtok `c43c86d92`, runa `02f9cc1`, cross-code `3d19c35`, stator `18c278f`.

### Plugin host: `wasm-plugin-host` (listepo/crates-packages, v0.1.0, MIT OR Apache-2.0)

- It is extism 1.30 on wasmtime 43. It loads core WASM modules, not components (`wasm-plugin-host/Cargo.toml:21-33`, `src/host.rs`).
- It runs one worker thread per plugin with two bounded lanes: control 16 and events 256. A full lane answers `Busy`. A watchdog cancels a call at its deadline (`src/host.rs:25-27`, `:307-329`).
- WASI is off, extism HTTP is compiled out, and memory is capped (16 MiB default, 64 MiB max). There is no fuel metering; deadlines use timeouts (`src/host.rs:167-172`).
- It has no manifest or permission model; those live in cox.
- Wasmtime uses Cranelift, a JIT, so it cannot run on iOS (section "Mobile" below).
- cox does not depend on it yet. cox keeps a near-identical host in `apps/cox/crates/cox-plugin/src/host.rs:124`.
- PR #42, "package layout for discovery, staging and removal", is open, mergeable and +1099/−5 (https://github.com/listepo/crates-packages/pull/42, 2026-10-09). It ports cox's on-disk layout as a generic module:
  - user plugins live at `<home>/plugins/<id>/versions/<digest12>/`, with `current` and `previous`;
  - project plugins live at `<project>/.<app>/plugins/<id>/`;
  - the package digest is a SHA-256 of the tree;
  - staging is atomic and refuses symlinks.
- GitHub Actions is off in that repository, so CI did not run on it.

### Contract: cox (pyrlyn/cox)

- **Licences:** `cox-plugin-api` and the SDK are MIT OR Apache-2.0 (`crates/cox-plugin-api/Cargo.toml:8`, `plugins/Cargo.toml:13`). The host crate `cox-plugin` is under the cox workspace licence, GPL-3.0-or-later or a royalty-free licence (`Cargo.toml:15`). Copying host code into Kerf is bound by that licence.
- **Manifest:** `plugin.toml` with `deny_unknown_fields` (`crates/cox-plugin-api/src/manifest.rs:19-60`).
  - Fields: `api = 1`, an `id` matching `^[a-z][a-z0-9-]{1,23}$`, `version`, `wasm`, `[limits]` (`memory_mib`, `call_ms`) and `[capabilities]`.
  - Declarative tables: `[[provider]]`, `[[models]]`, `[[mcp]]`, `[[external_agents]]`, `[[agents]]`.
  - JSON Schemas are committed in `docs/plugin.schema.json` and `docs/plugin-abi.schema.json`.
- **Permissions** (`docs/design/plugins.md:203`):
  - `events:`, `hooks:`, `tools:`, `invoke:`, `net:<host>`, `fs.read:`/`fs.write:` (rooted at `$WORKSPACE` or `$PLUGIN_DATA`), `decide:` and `ui.render:`;
  - the flags `wasi`, `context`, `kv`, `ui.status`, `ui.panel`, `ui.overlay`, `ui.commands` and `ui.keys`;
  - `model:`, `provider:`, `mcp:`, `agent:` and `subagent:`.
  - Grants are bound to the package digest, so changed bytes prompt again (`crates/cox-plugin/src/grant.rs:33-50`).
- **ABI:** `cox:host/v1`, JSON over extism (`crates/cox-plugin/src/hostfn.rs:43-58`).
  - Guest exports: `cox_init`, `cox_on_event`, `cox_hook`, `cox_tool_call`, `cox_command`, `cox_key`, `cox_render`, `cox_render_item`, `cox_decide`, `cox_provider_stream`.
  - Host functions: `cox_log`, `cox_notify`, `cox_kv_*`, `cox_context`, `cox_invoke_tool`, `cox_model_call`, `cox_http`, `cox_output`, `cox_cancelled`, `cox_redraw`.
  - Each host function is checked against the grant and the calling export.
  - Three consecutive failures disable an export (fail-open).
- **No process spawning by plugins:** no host function spawns a process (`docs/design/plugins.md:413`), and WASI is off (`crates/cox-plugin/src/host.rs:136`). Processes a plugin declares in `[[mcp]]` or `[[external_agents]]` are spawned by the host under `sandbox::Policy` (`crates/cox-plugin/src/external_agent.rs:1-20`). Kerf's LSP and DAP entries follow the same pattern.
- **Install:** `cox plugin list|install|enable|disable|update|remove|new|link` (`crates/cox/src/cli.rs:407-506`).
  - The source can be a local directory, an `https` `.tar.gz` with `--sha256`, or `git+<url>` with `--rev`.
  - The package is staged under its digest, then `current` is swapped and one `previous` is kept for rollback (`crates/cox-plugin/src/install.rs`).
- **SDK and templates:** the Rust SDK `cox-plugin-sdk` with a `register!` macro (`plugins/sdk/src/lib.rs`), Rust and Dart templates (`plugins/templates/`), and examples in Rust, Dart and Kotlin.
- **UI:** a closed `Widget` tree, TUI-oriented, capped at 512 nodes, depth 8 and 16 KiB of text, with style tokens instead of raw colours (`crates/cox-plugin-api/src/ui.rs:14-18`, `:63`).

### UI

- **weft** (listepo/weft): an open format for UI that AI agents read, write, validate and patch (`README.md:3`). It is strict XML-like markup plus canonical JSON, a closed component catalog and no code in documents (`SPEC.md:12-20`).
  - The core is Rust, with a WASM build (`crates/weft-wasm`) and a Slint generator (`crates/weft-slint`).
  - Status: prototype. Whether it can describe plugin UI in an IDE is **unverified**.
- **slint-bindings** (listepo/slint-bindings): embeds Slint UIs into native hosts (NSView or SwiftUI, WinUI 3) through a custom `slint::platform` with a software renderer and a C ABI (`README.md:1-13`). Status: M1 spike.
- **slint_dart** (listepo/slint_dart): a Slint ↔ Dart/Flutter toolkit: core, interpreter, typed generator, AOT compiler, Skia, testing (`README.md:5-25`).
- **slint-flutter** (listepo/slint-flutter): a smaller package of the same idea with a `SlintView` widget. How it relates to slint_dart is **unverified**.

### Marketplace: ketch and ketch-registry

- **ketch** installs binaries straight from GitHub releases and checks their published checksums (`apps/ketch/README.md:6-8`). It fits language servers and debug adapters, not plugin packages.
- **ketch-registry:** one folder per package with a `ketch.toml`. The only required field is `source = "github:owner/repo"`. Optional fields: `bin`, `provides` and `[asset]` globs (`packages/ketch-registry/README.md:3-20`).
- A plugin registry for Kerf would need the cox package format (digest, grants), not the ketch format as is.

### Index: `rtok graph`

- It indexes tree-sitter tags (`tree-sitter` 0.25) into SQLite for 13 languages (`apps/rtok/Cargo.toml:54`, `:141-155`).
- It has an optional LSP backend that runs rust-analyzer, clangd or typescript-language-server (`src/plugins/graph/lsp.rs:1-15`).
- **Correction to the brief:** reference recall is now 0.924 (97/105), not 0.351, as measured on 2026-10-04 (`apps/rtok/research.md:148`, `tests/graph_truth.rs:238-245`).
  - 0.351 was the 2026-09-04 measurement. `README.md:405` and `docs/comparison.md:228` still quote it.
  - Definitions: 43/44 found, precision 1.000. The remaining misses are inside macro bodies.
  - The labelled set is small and Rust-heavy, so how well this generalises is **unverified**.
- It is still a start: macro bodies and type-resolved references need an LSP or Kerf's own semantic layer.

### AI

- **`cox acp`:** an Agent Client Protocol server on stdio for editors. cox can also act as an ACP client that drives external agents (`apps/cox/crates/cox/src/acp_cmd.rs:5-10`, `crates/cox-acp/src/lib.rs:6-8`).
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

- **Plugin ABI.** Core modules with JSON over extism (cox, `wasm-plugin-host`) or WIT components (Zed, Lapce via WASI)? Taking cox "as is" means core modules; versioned WIT would be a new contract. Settled by the creator on 2026-10-09: cox's ABI as is.
- **Licence.** Settled by the creator on 2026-10-09: the same terms as cox (GPL-3.0-or-later or royalty-free, plus commercial), so host code may be taken from cox.
- **UI.** Plugin UI could be cox's `Widget` tree, extended for the IDE, or weft. The window could be Slint through slint-bindings or Flutter through slint_dart.
- **Mobile.** Wasmtime needs a JIT. iOS and Android need an interpreter backend behind the same host API.
