# Kerf — architecture

The contracts every other Kerf task builds on: the core boundary, the plugin
lifecycle on the coding agent's ABI, the IDE permission set and the LSP/DAP manifest tables
(T4.1 and T4.2), process supervision, Workspace Trust and interface
versioning.

Sources were checked on 2026-10-18 unless a version is given. Facts without a
primary source are marked **unverified**. Borrowed ideas link to
[research.md](../research.md). Approval is the creator's; unsettled decisions
are listed in [Open points for the creator](#open-points-for-the-creator).

## 1. Settled direction

Already fixed by the creator ([AGENTS.md](../AGENTS.md),
[research.md §1](../research.md#1-conclusion),
[research.md §5](../research.md#5-open-questions)):

- The plugin contract is the coding agent's — `agent-plugin-api`, `agent-plugin` — as is:
  manifest, ABI, grants, install flow, SDK. Kerf extends the permission set;
  it does not fork the contract.
- Plugin ABI: core WASM modules called with JSON through extism
  (`the coding agent:host/v1`), hosted by the shared `wasm-plugin-host` crate. No WIT
  components ([research.md §5](../research.md#5-open-questions), settled
  2026-10-09).
- Licence: `GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free`, plus a
  commercial licence — the same terms as the coding agent, so host code may be taken from
  The coding agent ([research.md §5](../research.md#5-open-questions), settled 2026-10-09).
- Plugins never spawn processes. A plugin declares a command; the core checks
  it against the plugin's permissions and runs it
  ([research.md §3](../research.md#3-what-kerf-takes) item 3).
- Everything a plugin, a language server, a debug adapter or a repository
  writes is untrusted input. Every queue and buffer it can grow has a hard cap.

Still open in [research.md §5](../research.md#5-open-questions): plugin UI
(the coding agent `Widget` tree or weft). It is not a contract T2–T4 need, so this
document does not settle it. The mobile plugin backend is settled in
[research.md §8](../research.md#8-mobile-plugin-backend-t15): wasmtime's
Pulley interpreter behind the same host API (`kerf-plugin` feature
`interpreter`, on by default for iOS and Android). The window is
settled in [research.md §7](../research.md#7-ui-shell-t12): native hosts over
a C ABI, with a Slint editor surface embedded through slint-bindings (the
creator, 2026-10-20; replaces Kerf's own GPU renderer, plan.md T16).

## 2. Core boundary and crate map

### 2.1 What the core owns

The core is the small Rust heart of the IDE. It owns, and nothing else may
own:

| Concern | Responsibility |
| --- | --- |
| Rendering | Drawing the editor surface and host UI. Plugin UI is data the core draws (the coding agent's closed `Widget` tree shape); plugins never supply drawing code. |
| Text buffer | Rope-backed document storage, edits, undo and redo (T2.1). |
| Event loop | A single-threaded loop with bounded queues and back-pressure (T2.2). |
| Semantic layer | tree-sitter parse per file, incremental re-parse, symbol index (T3). Language servers *add to* this layer; they never replace it ([research.md §1](../research.md#1-conclusion), [research.md §3](../research.md#3-what-kerf-takes) item 4). |

Everything else is either a sandboxed WASM plugin or a separate process the
core starts (language server, debug adapter, build/task). Language-server and
debug-adapter *clients* are host-side code — they are not plugins and run
outside the sandbox. That split follows Zed (extensions supply launch
commands; the IDE implements the LSP and DAP clients —
[research.md §2 Zed](../research.md#2-survey)) and the coding agent (processes a plugin
declares are spawned by the host under `sandbox::Policy` —
[research.md §3](../research.md#3-what-kerf-takes) item 3).

### 2.2 Crate map

One Cargo workspace; every crate lives in `crates/` and takes shared versions
from `[workspace.dependencies]` (AGENTS.md hard rules).

| Crate | Status | Holds |
| --- | --- | --- |
| `crates/kerf-core` | named in [plan.md](../plan.md) T2 | Text buffer, edit events, undo/redo, event loop with bounded queues. No UI toolkit binding — rendering is a trait the shell implements. |
| `crates/kerf-semantic` | named in [plan.md](../plan.md) T3 | tree-sitter incremental parse, symbol index. Depends on the edit-event type, not on buffer internals. Language-server answers merge in through `merge_lsp` (T8), tagged by source. |
| `crates/kerf-inspect` | named in [plan.md](../plan.md) T9 | Inspections: the versioned `kerf_inspect` JSON contract, a runner that caps and checks plugin replies, quick fixes as one undoable buffer edit. The plugin call is a trait the plugin host implements. |
| `crates/kerf-complete` | named in [plan.md](../plan.md) T14 | Local inline completion: the text around the cursor to runa's fill-in-the-middle endpoint (`/v1/infill`), with a timeout, a reply size cap, a completion length cap and cancellation on the next edit. One worker thread; the event loop only polls. Chat and agents are not here: they go through `the coding agent acp`. |
| `crates/kerf-remote` | [done.md](../done.md) T13 | Remote development: the versioned UI ↔ host protocol (JSON-RPC 2.0, one object per line) and the headless host that owns buffers and the semantic layer next to the code; `kerf-host` serves it over stdio, which SSH carries. Every line, queue, buffer and answer is capped. |
| `crates/kerf-slint` | named in [plan.md](../plan.md) T16.1 | The editor surface as a Slint component (`ui/editor.slint`), drawn on the CPU by `slint-bindings-core`'s `EmbeddedHost` into an RGBA buffer the host owns. Implements the core's `Render`, takes the host's theme and reports the caret rectangle in logical points for the system caret. Replaced the former GPU renderer `kerf-render` (T16). |
| `crates/kerf-ffi` | named in [plan.md](../plan.md) T12.3 | C ABI for the native hosts (`include/kerf.h`, generated by `cbindgen`): an editor handle over `kerf-core` and a view handle drawn by `kerf-slint` on the CPU into a frame the host copies out, used only from the thread that made it. Checks every pointer, length and UTF-8 string from the host and catches panics at the boundary. |
| `macos/` | named in [plan.md](../plan.md) T12.4, T16.2 | The macOS host, a SwiftPM package: `CKerf` (module over `kerf.h`), `KerfKit` (`Editor`, `EditorView`: an `NSView` that shows Kerf's frame as its layer's contents, with `NSTextInputClient`, the system caret and the Xcode theme), and the `Kerf` AppKit app with its File and Edit menus. Links `kerf-ffi`'s static library. |
| `crates/kerf-gtk` | named in [plan.md](../plan.md) T17 | The Linux host on GTK 4 (gtk4-rs), calling only `kerf-ffi`'s C functions: `Editor`, `EditorView` (a `GtkWidget` that shows Kerf's frame as a `GdkMemoryTexture`, takes keys and input-method text through `GtkIMMulticontext` and draws GTK's caret over the frame) and the app (`GtkApplication`, header bar with Open and a main menu, GNOME shortcuts). Looks like GNOME Text Editor: GNOME's monospace font, the Adwaita style schemes. Builds only on Linux. |
| `crates/kerf-process` | approved, created by its first task | Spawn, supervise, cap and stop LSP, DAP and task processes. The only crate that creates OS processes for those services. |
| `crates/kerf-lsp` | created in T8 | LSP client (JSON-RPC over stdio). Not a plugin. Until `kerf-process` exists it holds the per-server caps of §5.2 (message size, inbound queue, stderr ring, timeouts, shutdown budget) and starts processes only through its `Spawn` trait; process counts and the restart budget belong to `kerf-process`. |
| `crates/kerf-dap` | approved, created by its first task | DAP client on top of `kerf-process`. Not a plugin. |
| `crates/kerf-plugin` | created by T6 | WASM plugin host wiring: `wasm-plugin-host` + `agent-plugin-api` manifest/grant types + Kerf host functions. |

Shared engines Kerf depends on rather than copies:

- `wasm-plugin-host` (listepo/crates-packages, MIT OR Apache-2.0) — extism
  host, lanes, limits, package layout
  ([research.md §4](../research.md#4-our-repositories)).
- `agent-plugin-api` — manifest and ABI types (T4 extends it).
- The coding agent's `sandbox::Policy` shape for wrapping every spawned process
  ([research.md §3](../research.md#3-what-kerf-takes) item 3).

Workspace trust (§6) is a small state machine; it can live in `kerf-core` or a
later `kerf-trust` crate. Default: `kerf-core`, until it grows.

### 2.3 Core contracts (T2 and T3)

These are the types the tasks must agree on so `kerf-semantic` never depends
on buffer internals and LSP results can merge later
([ideas.md](../ideas.md), IntelliJ inspections over the index —
[research.md §3](../research.md#3-what-kerf-takes) item 4):

1. **Edit event.** A buffer edit emits an event with the old and new byte
   ranges and the line/column positions before and after. T2.1 defines it;
   T2.2 and T3 consume it. Shape is fixed in T2.1; exact fields are T2.1's
   to choose.
2. **Event loop.** Single-threaded. Every inbound queue is bounded and
   answers (back-pressure or drop with a counter) when full — it never grows
   without limit. Plugins and external processes only enqueue; the loop
   drains (T2.2).
3. **Semantic index.** Per-file parse and symbol table; an edit re-parses and
   re-indexes only the touched file (T3). LSP diagnostics and jump targets
   are *additional* sources merged into this index, not a parallel truth.

## 3. Plugin lifecycle (the coding agent ABI as-is)

Kerf loads plugins exactly as the coding agent does — `the coding agent:host/v1`, extism, one
`PluginHost` per plugin from `wasm-plugin-host`
([research.md §4](../research.md#4-our-repositories)). Kerf adds no second
ABI.

### 3.1 Package and manifest

- A plugin is a directory: `plugin.toml`, the `wasm` module it names, optional
  `bin/` for executables the manifest tables name
  (the coding agent `docs/design/plugins.md` §1).
- User plugins live at `<home>/plugins/<id>/versions/<digest12>/` with
  `current` and `previous`; project plugins at
  `<project>/.kerf/plugins/<id>/` and lose an id clash to the user plugin
  (`wasm-plugin-host` package layout; the coding agent §1).
- The package digest is a SHA-256 of the tree (manifest, wasm, `bin/`), so
  swapping a shipped binary cannot avoid a new approval
  ([research.md §4](../research.md#4-our-repositories)).
- `plugin.toml` is `deny_unknown_fields`. Fields: `api = 1`, `id`
  (`^[a-z][a-z0-9-]{1,23}$`), `version`, `wasm`, `[limits]` (`memory_mib`,
  `call_ms`), `[capabilities]`, and the declarative tables `[[provider]]`,
  `[[models]]`, `[[mcp]]`, `[[external_agents]]`, `[[agents]]`, `[[skills]]`
  (the coding agent `crates/agent-plugin-api/src/manifest.rs`). Kerf adds the tables in §4.

**Manifest is read without running code** — capabilities, activation triggers
and process declarations are static data (VS Code's `contributes` /
`activationEvents` shape — [research.md §2 VS Code](../research.md#2-survey),
[research.md §3](../research.md#3-what-kerf-takes) item 2). The WASM module is
not instantiated to answer "what does this plugin do?".

### 3.2 Grants

- A plugin runs only under a grant that names its package digest and its
  capabilities. Changed bytes or wider capabilities ask again
  (the coding agent `docs/design/plugins.md` §3; [research.md §4](../research.md#4-our-repositories)).
- The granted list is one line per capability (the coding agent §3). Kerf's IDE families
  join that list in §4.3.
- Headless and ACP never approve; only already-granted plugins load (the coding agent §3).
- **Project plugins are repository content, and repository content is
  untrusted.** A project plugin never loads until the user enables it for that
  repository (the coding agent §1). Cloning a repository never runs its plugins.

### 3.3 Load, lazy activation, crash and disable

| Stage | Behaviour |
| --- | --- |
| Discover | Read `plugin.toml`, check `api`, id and schema. No wasm. |
| Activate | Instantiate only when a trigger fires (§3.4). Then `agent_init` runs once; anything not granted is dropped with a notice (the coding agent §4). |
| Run | One worker thread per plugin, two bounded lanes: control 16, events 256. A full lane answers `Busy`. A watchdog cancels a call at its deadline. Memory cap 16 MiB default, 64 MiB max. WASI off; no extism HTTP ([research.md §4](../research.md#4-our-repositories), `wasm-plugin-host` `src/host.rs`). |
| Fail | A trap, timeout or malformed reply is a warning and an absence, never a crash of the IDE (the coding agent §6 D14, [AGENTS.md](../AGENTS.md)). Three consecutive failures disable that export for the session (the coding agent §4). |
| Disable / remove | `plugin disable` clears the grant; remove deletes the package and its kv, and revokes grants (the coding agent §1c). |

Per-call budgets (the coding agent §10): render 20 ms, event batch 50 ms, hooks and tools
at their own tighter caps, all under `limits.call_ms`.

### 3.4 Lazy activation

Activation triggers are declared in the manifest and read without running
plugin code ([research.md §3](../research.md#3-what-kerf-takes) item 2, VS
Code). Kerf derives the primary triggers from the declarative tables so a
language plugin loads when its language appears:

| Trigger source | Fires when |
| --- | --- |
| `[[language_servers]]` `languages` / `globs` | A buffer of that language, or a path matching a glob, opens (and Workspace Trust allows — §6). |
| `[[debug_adapters]]` `languages` | A debug session for that language starts. |
| `ui.commands` / `terminal.keys` | The user invokes the command or key. |
| `capabilities.tools` / `hooks` / `events` | First use of the tool, or the subscribed event fires — these have no static trigger of their own. |

An explicit `activates = [...]` list (VS Code's `activationEvents`) is
**new** and not in v1 (§8 decision 2).

### 3.5 No process spawning by plugins

No host function spawns a process (the coding agent §10). WASI is off. Processes a plugin
brings — `[[mcp]]`, `[[external_agents]]`, and Kerf's `[[language_servers]]`
and `[[debug_adapters]]` — are spawned by the *core* under the sandbox policy
after the grant is checked. A plugin returns (declares) the command; it never
runs it ([research.md §3](../research.md#3-what-kerf-takes) item 3,
[AGENTS.md](../AGENTS.md)).

## 4. IDE permission set and manifest tables (T4.1 and T4.2)

Permission families are settled in
[research.md §1](../research.md#1-conclusion): **buffer, LSP, DAP, index,
terminal, git**. Kerf extends the coding agent's permission set; it does not replace
The coding agent's existing families (`events`, `hooks`, `tools`, `invoke`, `net`, `fs`,
`decide`, `ui.*`, `model`, `kv`, `context`, …). A Kerf plugin may use both.

The contract lives in the coding agent's `agent-plugin-api` and was landed in two steps:
[pyrlyn/cox#367](https://github.com/pyrlyn) added
`[capabilities.ide]` and the two process tables, and
[pyrlyn/cox#385](https://github.com/pyrlyn) (the coding agent T380)
split `buffer` and `git` into read and write levels and made every process
entry routable. This section describes what the coding agent now has; the coding agent's
`docs/design/plugins.md` §16 and §7e are the field-level reference.

### 4.1 Families mapped to the coding agent concepts

All IDE classes sit in one the coding agent table, `[capabilities.ide]`, and every grant
line starts with `ide.`, so none of them re-binds an existing the coding agent name.

| Family | Manifest | Grant line | What it allows |
| --- | --- | --- | --- |
| buffer | `ide.buffer = "read"` or `"edit"` | `ide.buffer.read`; `"edit"` adds `ide.buffer.edit` | Read open buffer text, unsaved text included; `edit` also applies edits through the core's undoable edit path. Not `fs.*` (buffers may be unsaved) and not `context` (a session snapshot, the coding agent §5). |
| LSP | `ide.lsp = true`, plus `[[language_servers]]` entries | `ide.lsp`, plus one `lsp:<name> <command args>` per entry | `ide.lsp` lets the plugin's wasm act as a client of servers the core spawned; each `lsp:` line approves one process the core spawns. The line has the shape of the coding agent's `mcp:` line (the coding agent §3, §7c). |
| DAP | `ide.dap = true`, plus `[[debug_adapters]]` entries | `ide.dap`, plus one `dap:<name> <command args>` per entry | The same split for debug adapters. |
| index | `ide.index = true` | `ide.index` | Query Kerf's semantic layer (symbols, definitions, references). Read-only. Not `context`. |
| terminal | `ide.terminal = true` | `ide.terminal` | Open an integrated terminal the core owns and write input to it. Distinct from the coding agent's `terminal.keys` (leader keys, the coding agent §15.3); neither line covers the other. |
| git | `ide.git = "read"` or `"write"` | `ide.git.read`; `"write"` adds `ide.git.write` | `read`: `status`, `diff`, `log`. `write`: stage and commit only (§8 decision 6). The core runs `git`, never the plugin. |

A write level also lists its read line, so widening `read` to `edit` or
`write` adds a line and the digest-bound grant asks again. `buffer = true`,
`git = true` and any level word other than these are parse errors, never a
guess at a level.

### 4.2 Manifest tables

`[[language_servers]]` and `[[debug_adapters]]` are modelled on `[[mcp]]`
(the coding agent `McpDecl`, `manifest.rs`): a name, a command inside the package or on
PATH, and args. One approval line each.

```toml
api = 1
id = "rust-tools"
# ... standard fields ...

[capabilities.ide]
buffer = "read"      # or "edit"
index = true
git = "read"         # or "write": stage and commit only
terminal = true
lsp = true           # client of the servers below
dap = false

[[language_servers]]
name = "rust-analyzer"
languages = ["rust"]                     # activation + routing
globs = ["**/*.rs"]                      # optional path routing
command = "bin/rust-analyzer"            # package-relative or PATH program
args = []
env = { RUST_LOG = "error" }             # literals only; the child starts from an empty env
config = { checkOnSave = true }          # opaque, passed on initialize

[[debug_adapters]]
name = "codelldb"
languages = ["rust"]                     # activation
command = "bin/codelldb"                 # inside the package, or on PATH
args = ["--port", "0"]
```

Field notes (the coding agent `manifest.rs`, checked by `PluginManifest::validate`):

| Field | Kind | Rules |
| --- | --- | --- |
| `ide.buffer` | `"read"` \| `"edit"`, absent = none | `edit` implies `read`. |
| `ide.git` | `"read"` \| `"write"`, absent = none | `write` is the dangerous half: stage and commit only. |
| `ide.lsp`, `ide.dap`, `ide.index`, `ide.terminal` | `bool` | Absent and `false` are the same. |
| `[[language_servers]]` | `Vec<LanguageServerDecl>` | `name` unique and valid after `<id>-` prefixing; `command` required and non-empty; `env` keys are env var names; at least one of `languages` or `globs`. No URL transport — LSP is a process. |
| `[[debug_adapters]]` | `Vec<DebugAdapterDecl>` | Same name, command and `env` rules; `languages` non-empty. |

A package whose only content is these tables (with `[[mcp]]`, `[[agents]]`,
`[[skills]]`) needs no `wasm`. Workspace-root discovery (`root_markers`) is
not in the table yet; it is added by the first task that needs it.

Each entry is one approval line. An update that adds or widens a line asks
again (the coding agent §3, digest-bound grants).

### 4.3 How a plugin declares a language server / debug adapter

1. The plugin package declares `[[language_servers]]` / `[[debug_adapters]]`
   in `plugin.toml` (static; no code runs to read them).
2. Install/enable shows each command verbatim at approval — package-relative
   `bin/…` is covered by the package digest; a PATH program (`rust-analyzer`)
   is shown as text (the coding agent §7c).
3. When the trigger fires (§3.4) and Workspace Trust allows (§6), the **core**
   checks the grant line, wraps the argv in the sandbox policy and spawns the
   process. The plugin is not in the spawn path.
4. The plugin's wasm code may still do plugin things (diagnostics decorations
   via `ide.buffer`/`ide.index`, a status segment via `ui`) — it never
   proxies LSP traffic. The LSP client lives in `kerf-lsp`.

Runtime command resolution (Zed's `language-server-command` returns
`{command, args, env}` at run time —
[research.md §2 Zed](../research.md#2-survey)) is *not* in this contract.
v1's manifest command is the only source (§8 decision 3).

### 4.4 Host functions for the new families

Kerf-only host functions live in a **new namespace `kerf:host/v1`**, leaving
`the coding agent:host/v1` untouched (the same pattern as the coding agent's `the coding agent:desktop/v1` surface
namespace, the coding agent §15.7). Candidates (each checked against the grant and the
calling export, like every the coding agent host function):

| Host fn | Grant | Notes |
| --- | --- | --- |
| `kerf_buffer_read(id)` | `ide.buffer.read` | Current buffer text, capped. |
| `kerf_buffer_edit(id, edits)` | `ide.buffer.edit` | Goes through the core's undoable edit path; emits edit events. |
| `kerf_index_query(query)` | `ide.index` | Symbols/definitions/references from `kerf-semantic`. |
| `kerf_git_read(op, args)` | `ide.git.read` | Core runs `git`; args are a closed enum, not a shell string. |
| `kerf_git_write(op, args)` | `ide.git.write` | Stage and commit only; never `Command` with a free-form string. |
| `kerf_terminal_open/write/close` | `ide.terminal` | Session the core owns; output is untrusted and capped. |

Exact JSON shapes land with the task that builds `kerf:host/v1`; names above
are the contract's vocabulary.

## 5. LSP and DAP process supervision

The core is the only spawn point (§3.5). Supervision lives in
`kerf-process`; protocol clients are `kerf-lsp` / `kerf-dap`. Language
servers and debug adapters are **untrusted input** — every queue and buffer
they can grow has a hard cap (AGENTS.md hard rule).

### 5.1 Start

1. Trigger: matching language buffer opens, debug session starts, or the user
   asks (§3.4).
2. Workspace Trust check (§6). Untrusted folder: do not start.
3. Grant check: `lsp:<name> …` / `dap:<name> …` present for this plugin
   digest (§3.2).
4. `kerf-process` wraps argv in the sandbox policy (the coding agent `sandbox::Policy`
   shape — [research.md §3](../research.md#3-what-kerf-takes) item 3) and
   spawns stdio pipes. Environment is cleared except what the declaration
   needs (the coding agent clears env for MCP servers, the coding agent §7c).
5. One client instance per **(workspace root, server name)**. A second root
   gets its own instance; a second name is a second process.

### 5.2 Caps

| Queue / buffer | Cap | On overflow |
| --- | --- | --- |
| Outbound requests to the process | 64 in flight, 256 queued | Refuse new requests with a busy error; never unbounded. |
| Inbound message queue (stdout / DAP events) | 256 messages, 4 MiB total | Drop *notifications* oldest-first with a counter; responses for in-flight requests are never dropped silently — if they cannot be delivered, the client errors that request. |
| Single message size | 4 MiB | Treat as protocol abuse: kill the process, notice, fail open. |
| stderr ring | 8 KiB | Truncate; last chunk kept for the log. |
| Process count per workspace | 8 language servers, 4 debug adapters | Refuse the next start with a notice. |
| Restarts | 3 per hour per (root, name) | Give up; the IDE keeps editing without that server (fail open). |
| DAP stopped/continued event ring | 256 | Drop oldest with a counter. |
| Diagnostics / index updates applied per second | 200 files or 5 MiB of text | Coalesce and drop oldest; the core's own semantic layer stays authoritative. |

Every cap is a typed error in `kerf-process` (like `wasm-plugin-host`'s
`Busy` / `Deadline` / `MemoryCap`), not a silent grow. The numbers are
approved defaults for `kerf-process` (§8 decision 4).

### 5.3 Stop

Stop triggers: workspace close, trust revoked (§6), plugin disabled/removed,
user command, restart budget exhausted, or protocol abuse.

1. Graceful: LSP `shutdown` / `exit`, DAP disconnect. Budget: 2 s.
2. Then terminate the process tree (the sandbox wrap already confines it;
   termination is the supervisor's job).
3. Drop queues, emit one notice if the stop was not user-initiated.

A slow or dead server never blocks the event loop: replies arrive on a
`kerf-process` reader thread and are *enqueued* onto the loop's bounded
queues (§2.3 item 2).

### 5.4 Builds and tasks

Builds and tasks are also processes the core starts
([research.md §1](../research.md#1-conclusion)). They are user-initiated or
debug-initiated in v1; a plugin-initiated build is not in the permission
families (§8 decision 5). Same sandbox wrap, same caps table (queue/buffer
caps at least).

## 6. Workspace Trust

Taken from VS Code: Restricted Mode disables debugging and non-opted-in
extensions ([research.md §2 VS Code](../research.md#2-survey),
[research.md §3](../research.md#3-what-kerf-takes) item 2).

| State | LSP | DAP | Tasks/builds | Project plugins | User plugins |
| --- | --- | --- | --- | --- | --- |
| **Untrusted** | do not start | do not start | do not start | do not load (the coding agent §1 already requires a per-repo enable; untrusted never counts as enabled) | may load for UI-only contributions; their `[[language_servers]]` / `[[debug_adapters]]` do not spawn |
| **Trusted** | start on trigger + grant | start on trigger + grant | allowed | load after the per-repo enable | full, under their grants |

Rules:

- Trust is per **folder root**, decided by the user, persisted outside the
  repository (a repository file is untrusted input and must not grant trust).
- Cloning / opening a folder never trusts it and never starts a process from
  it.
- The trust prompt lists what would start: language servers, debug adapters,
  tasks, project plugins — the same "show the capabilities in words" idea as
  The coding agent grant dialogs (the coding agent §2).
- Trust can be revoked; revoking stops LSP/DAP/tasks in that root (§5.3).
- A trusted folder does not bypass grant checks: both conditions are required.

Implementation (T7): `kerf_core::trust`. `TrustStore` holds canonical folder
roots in memory; a trusted root covers its subfolders, as in VS Code and Zed
([research.md §2](../research.md#2-survey)). Every LSP, DAP and task spawn
calls `TrustStore::authorize(cwd, SpawnKind)` and gets a `SpawnPermit` or a
typed `TrustError`. The permit has no public constructor, so `kerf-process`
spawn functions take it by value and run the process in `permit.cwd()`.
`revoke` answers the removed roots so the supervisor can stop their processes
(§5.3). `TrustStore::load` / `save` keep the roots in `kerf/trust.toml` under
the OS config directory (`TrustStore::default_path`, decision 7), with a
committed JSON Schema (`crates/kerf-core/schema/trust.schema.json`), a 4 MiB
read cap, a 1024-root cap, absolute roots only and an atomic write.

## 7. Interface versioning

Following the coding agent's rules (the coding agent §4, §15.7) and the borrowed shapes
([research.md §3](../research.md#3-what-kerf-takes) item 5 — Visual Studio
versioned RPC contracts; [research.md §2 Zed](../research.md#2-survey) —
versioned WIT, which Kerf deliberately does *not* take):

1. **`the coding agent:host/v1` is not forked.** Kerf registers the same namespace and
   adds nothing to it for v1. Kerf-only host functions use
   **`kerf:host/v1`** (§4.4), the way the coding agent isolates `the coding agent:desktop/v1`.
2. **`api` is the ABI major** (`API_MAJOR = 1` in `agent-plugin-api`). A minor
   addition — a new optional export, host function or JSON field — keeps
   `api = 1`, because ABI types ignore unknown JSON fields both ways. A
   breaking change raises `api`; the host supports the current major and the
   previous one for one release (the coding agent §4).
3. **The manifest is the exception.** `deny_unknown_fields` means an older
   host refuses a package that uses a newer field (`[capabilities.ide]`,
   `[[language_servers]]`, …) and skips it with a notice. That strictness is
   a trust property (a misspelt capability must not grant less or more). An
   old manifest loads unchanged in a new host (the coding agent §15.7).
4. **Grants are per `(plugin id, scope, digest)`.** A capability list that is
   not the documented string array grants nothing (the coding agent §3). New grant lines
   (`ide.buffer.edit`, `lsp:…`) appear in that array. An older host that does not
   know a new manifest field refuses the package at parse time (point 3), so
   it never has to interpret a grant line it does not know. Wider
   capabilities re-ask.
5. **Cross-crate core APIs** (`kerf-core` → `kerf-semantic` edit events,
   `kerf-process` → `kerf-lsp`) follow Rust semver inside the workspace.
   They are not plugin ABI and need no `api` field.
6. **External protocols:** the LSP client speaks LSP 3.x and the DAP client
   speaks DAP 1.x; exact negotiated versions are implementation facts and
   **unverified** in this document.

## 8. Decisions on the open points

The creator delegated these to the agent that closed T1; each answer picks the
smaller option that keeps the settled direction (§1). Revisit any of them in a
new task, not by editing this list silently.

1. **Crate map: approved as split.** `kerf-process`, `kerf-lsp`, `kerf-dap`
   and `kerf-plugin` stay out of `kerf-core`, so the core never creates OS
   processes. A crate is created by the first task that needs it, never empty
   in advance.
2. **Activation triggers: declarative tables only.** No `activates = [...]`
   field in v1. Tools, hooks and events with no static trigger activate on
   first use (§3.4). Nothing new in `agent-plugin-api` for this.
3. **Runtime command resolution: manifest only.** No Zed-style export in v1,
   because "what a plugin can do is readable without running its code"
   (AGENTS.md). A later export must still match the granted `lsp:`/`dap:`
   line, and the core still spawns.
4. **Supervision caps: approved** as the defaults in §5.2. They become
   settings later; the hard upper bounds stay.
5. **Builds and tasks: user- and debug-initiated only.** No seventh family in
   v1.
6. **`ide.git.write`: stage and commit only.** Checkout, branch, reset and
   push are not grantable to plugins in v1; they change the working tree or
   the remote under the user. `ide.git.read` covers `status`, `diff`, `log`.
   (Revised by T4: the line names follow the coding agent's `ide.` table, §4.1.)
7. **Workspace Trust store:** a user-level file in the OS config directory,
   never in the repository. Multi-root workspaces trust **per root**, as in
   VS Code; trusting one root trusts nothing else.
8. **`kerf:host/v1`: confirmed.** IDE host functions live there, each gated
   by an `ide.*` grant line (§4.4); `the coding agent:host/v1` stays unchanged (§7
   point 1).
9. **Terminal: `ide.terminal` in the coding agent's `[capabilities.ide]`.** Revised by
   T4: the coding agent landed the IDE classes as one table of their own
   ([pyrlyn/cox#367](https://github.com/pyrlyn)), so the
   integrated terminal is `ide.terminal` rather than an `io` flag on
   `TerminalCaps`. It never covers the coding agent's `terminal.keys`, nor the reverse
   (§4.1).
10. **LSP merge points:** T8 defines the merge API in `kerf-semantic`, tagged
    by source, definitions and references first. Diagnostics are not index
    data and stay a separate stream. Done in T8:
    `Semantic::merge_lsp` adds `documentSymbol` definitions and `references`
    ranges tree-sitter missed, tagged `Source::LanguageServer`; an edit
    drops them.
