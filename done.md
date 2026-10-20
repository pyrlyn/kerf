# Done

### T1. Architecture document

The contracts every other task builds on, in `docs/architecture.md`. Done when the creator approves the document. The creator delegated the open points; the decisions are in §8 of the document.

It covers:

- the core boundary (rendering, buffer, event loop, semantic layer) and the crate map;
- the plugin lifecycle on cox's ABI as is (`cox:host/v1`, extism, `wasm-plugin-host`): manifest read without running code, lazy activation, crash and disable;
- the IDE permission set and the manifest tables for language servers and debug adapters, which are the input for T4.1 and T4.2;
- how the core starts, supervises, caps and stops LSP and DAP processes;
- Workspace Trust;
- interface versioning.

Plan:

1. Read `research.md`, cox's `docs/design/plugins.md`, `crates/cox-plugin-api/src/manifest.rs` and `wasm-plugin-host` (with crates-packages#42).
2. Write `docs/architecture.md`. Every borrowed idea links to its `research.md` section.
3. List the open points for the creator at the end of the document.
4. Verify: every permission and manifest table named in the document maps to a cox concept or is marked new.

### T4. IDE permissions in `cox-plugin-api`

Split when claimed (500-line task cap). Add the IDE permission set and the manifest tables for language servers and debug adapters, as T1 defines them, to cox's `cox-plugin-api`, its JSON Schemas and its docs. The plugin declares a process; the host spawns it, as with `[[mcp]]` today.

The work happens in the cox repository and gets a matching task in cox's `plan.md` when it starts. Blocked on T1 only for the final names; the permission families are already in `research.md` (buffer, LSP, DAP, index, terminal, git) and can be implemented against that list.

#### T4.1. IDE permission set

The buffer, LSP, DAP, index, terminal and git permission classes in `cox-plugin-api`, with validation and schema.

#### T4.2. LSP and DAP manifest tables

`[[language_servers]]` and `[[debug_adapters]]` (names as T1 settles them) in `PluginManifest`, JSON Schema, and the docs. The plugin declares the command; the host spawns it.

#### T4 outcome

MiMo's [pyrlyn/cox#367](https://github.com/pyrlyn/cox/pull/367) had already landed both subtasks in cox with names that differ from the first draft of `docs/architecture.md` §4: `[capabilities.ide]` with `ide.*` grant lines, and the LSP/DAP tables keyed by `languages` (plus `globs`, `env`, `config`). Those names were kept, so T6 needed no change. [pyrlyn/cox#385](https://github.com/pyrlyn/cox/pull/385) (cox T380) tightened least privilege: `ide.buffer` is `"read"` | `"edit"` and `ide.git` is `"read"` | `"write"` (stage and commit only), with grant lines `ide.buffer.read`, `ide.buffer.edit`, `ide.git.read`, `ide.git.write`; a language server needs `languages` or `globs` and a debug adapter needs `languages`. `docs/architecture.md` §3.4, §4, §7 and §8 decisions 6, 8 and 9 now describe what cox has.

Tests (in cox): `ide_caps_parse_per_family_and_validate`, `ide_buffer_and_git_refuse_a_flag_or_an_unknown_level`, `language_server_and_debug_adapter_need_something_to_route_on`, `widening_ide_buffer_or_git_needs_approval`, plus #367's table, schema and API-index drift tests; `cargo test -p cox-plugin-api -p cox-plugin`, workspace clippy `-D warnings` and `fmt --check` green.

### T2. Core skeleton

Split when claimed (500-line task cap). The first crate, `crates/kerf-core`, with no UI. The three subtasks together are done when `cargo test`, `cargo clippy -- -D warnings` and `cargo fmt --check` pass locally and in CI.

### T2.1. Text buffer, edits, undo and redo

- the text buffer, on a maintained rope crate chosen against the alternatives;
- edits with undo and redo, emitting edit events (byte ranges and positions) that the semantic layer (T3) can consume.

Plan:

1. Pick the rope crate: compare maintained options on crates.io and in `rust.md`, and give the reason in the commit.
2. Write the buffer, edits, undo and redo with tests.
3. Define the edit-event type so T2.2 and T3 can consume it without depending on the buffer internals.

### T2.2. Event loop with bounded queues

- a single-threaded event loop with bounded queues;
- back-pressure when a queue is full.

Plan:

1. Write the event loop and bounded queues against T2.1's edit events (or a stand-in type if T2.1 is not merged yet).
2. Tests for back-pressure and for a full queue answering rather than growing.

### T2.3. CI and toolchain bookkeeping

- a GitHub Actions workflow for fmt, clippy and test on Linux x86_64, macOS arm64 and Windows x86_64;
- `toolchain.md` and `rust.md` rows for every crate T2 introduces.

Plan:

1. Add the workflow (Linux x86_64, macOS arm64, Windows x86_64 — no Intel Mac).
2. Add the crates to `toolchain.md`, and to `rust.md` if they are new.

### T12. UI shell

Split when claimed (500-line task cap). The first window. The creator's direction (`research.md` §7): native controls in a native host per platform, talking to the Rust core through a C ABI, and Kerf's own GPU renderer for the editor surface inside a native view. The editor looks native: like Xcode's on macOS and like Visual Studio's on Windows (`research.md` §7, "Look"); the host supplies the font, colours and caret, the renderer hard-codes none. macOS first; Windows and Linux hosts are in `roadmap.md`. Rendering is a trait `kerf-core` defines (`docs/architecture.md` §2.1). Plugin UI and redraw-on-request come in later tasks. The subtasks together are done when the macOS app opens a file, shows it in the GPU-drawn editor view inside a native AppKit window, takes typing, undo and redo through `kerf-core`, and headless tests cover that path.

### T12.1. Rendering trait, editor and toolkit choice

- `research.md` §7: the UI shell direction with primary sources — first Slint through slint-bindings against Flutter through slint_dart, then the creator's choice of native hosts over a C ABI with Kerf's own GPU editor renderer (`wgpu` + `cosmic-text` + `glyphon`).
- `kerf-core`: the `Render` trait and the `Frame` the core hands it, and an `Editor` (buffer + cursor) that turns typing, backspace, cursor moves, undo and redo into `TextBuffer` edits.

Plan:

1. Compare the paths in `research.md` §7 (slint-bindings and slint_dart READMEs at `4d61045`, Slint, Flutter, wgpu and vello docs, crates.io metadata).
2. `crates/kerf-core/src/render.rs`: `Render`, `Frame` (buffer, cursor byte, line, column).
3. `crates/kerf-core/src/editor.rs`: `Editor`; `TextBuffer::line_col`, `prev_boundary`, `next_boundary` for whole-character steps.
4. Verify: unit tests for each editor operation and a recording `Render`; `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.

### T12.2. GPU editor renderer

A crate that implements `kerf_core::Render` on `wgpu` + `cosmic-text` + `glyphon` (`research.md` §7): the buffer text, a surface size and scale factor, and a `Theme` the host passes in (font family and size, background, text and selection colours, syntax colours). It reports the caret rectangle so a host can draw the system caret; it draws its own only when the host asks. Done when a test lays out a frame and checks the caret position without a GPU, and a test renders a frame offscreen and reads pixels back on the adapter the CI runner offers (or says in the card why a runner cannot).

Plan:

1. `crates/kerf-render` (row in `docs/architecture.md` §2.2); `wgpu`, `glyphon` (and its `cosmic-text`) in `[workspace.dependencies]`, rows in `toolchain.md`.
2. `Theme` (font family, size, line height, colours) and `Layout`: shapes the frame's text with `cosmic-text` at the host's scale factor and returns the caret rectangle in physical pixels. No GPU, unit-tested: caret on line 2, column after a wide character, scale 2.
3. `GpuRenderer` on a `wgpu::Device`/`Queue` and a target texture format: clears to the background, draws the text with `glyphon`, and a caret quad only when `Theme::draw_caret` is set. `impl Render` draws into a surface; an offscreen `render_to(&TextureView)` serves tests and hosts that own the texture.
4. Offscreen test: render into a texture, copy it to a buffer, read pixels back, check background and text colours. Skips with a message when no adapter exists; CI installs Mesa's lavapipe on Linux, Windows has WARP, macOS arm64 has Metal.
5. fmt, clippy `-D warnings`, tests; CI green on all three runners.

Outcome:

- `crates/kerf-render`: `Theme`, `Size`, `Rect`; `Layout` (GPU-free, returns the caret rectangle); `GpuRenderer::draw` into any texture view; `Offscreen`, a renderer with its own texture that implements `Render`. The caret is a solid custom glyph through `glyphon`'s pipeline, drawn only with `draw_caret`.
- Layout lines are built from the core's lines, split the way ropey splits them (`\n\r`, U+2028, NEL), so `Frame::line` indexes the layout; `cosmic-text` alone splits only on CR and LF. Host numbers (scale, sizes, caret width) are clamped, since they cross the C ABI.
- `Render` gained `type Error`: a GPU draw can fail (atlas full, evicted glyph).
- Left out: selection and syntax colours, because `Frame` carries no selection or highlight spans yet (noted on T12.4). The `wgpu` surface over an `NSView` comes with the C ABI in T12.3, where the host handle exists.
- Tests: six layout tests without a GPU, and `tests/offscreen.rs`, which renders on the machine's adapter and reads pixels back (background, text, caret on and off). CI installs Mesa's lavapipe on Linux and sets `KERF_REQUIRE_GPU`, so a runner without an adapter fails instead of skipping.

### T12.3. C ABI for native hosts

A `cdylib`/`staticlib` crate with a generated C header: open a file into an `Editor`, attach the renderer to a host view (an `NSView` on macOS), resize, forward text input, keys, undo and redo, render. Every pointer and length from the host is checked; a panic never crosses the boundary. Done when a Rust end-to-end test drives the whole path through the C functions only.

Plan:

1. `kerf_core::read_text(path, cap)`: the capped UTF-8 file read `kerf-remote` and the trust file each have a copy of; both switch to it.
2. `crates/kerf-ffi` (`cdylib` + `staticlib` + `rlib`, row in `docs/architecture.md` §2.2): opaque `KerfEditor` (open a file, insert UTF-8, commands Backspace/Left/Right/Undo/Redo, copy the text out) and `KerfView` (offscreen view for tests and thumbnails; `kerf_view_attach_appkit` on macOS makes a `wgpu` surface over the host's `NSView`; resize, set theme, render, caret rectangle). Every call checks null pointers, lengths and UTF-8, returns a `KerfStatus`, and runs inside `catch_unwind`.
3. `include/kerf.h` generated by `cbindgen`; a test fails when the committed header is stale.
4. `tests/c_abi.rs`: open a temp file, type, backspace, undo, redo, render offscreen, read the caret and pixels, all through the `extern "C"` functions; null, oversize and invalid UTF-8 inputs get error statuses. The `NSView` path needs a real AppKit view on the main thread, so T12.4's Swift test drives it.
5. fmt, clippy `-D warnings`, tests; CI green.

Outcome:

- `kerf_core::read_text` replaces the capped UTF-8 reads in `kerf-remote` and the trust file.
- `crates/kerf-ffi` (`staticlib`, `cdylib`, `rlib`): `KerfEditor` (new, open, insert, commands Backspace/Left/Right/Undo/Redo, cursor, copy text) and `KerfView` (offscreen, `kerf_view_attach_appkit` on macOS, resize, set theme, render, caret rectangle, read RGBA from an offscreen view). Every function returns a `KerfStatus`; null pointers, oversize input, invalid UTF-8 and unknown commands get their own status, and a panic becomes `KERF_STATUS_PANIC`.
- `kerf-render`'s `Offscreen` gained `resize` and `read_rgba`, so the pixel readback lives in one place for the Rust test and the C ABI.
- `include/kerf.h` is generated by `cbindgen` 0.29.3 (extism pins it) and compiles as C11 and C++17; `tests/header.rs` fails when it is stale (`KERF_BLESS=1` rewrites it).
- Tests: `tests/c_abi.rs` drives open, type, backspace, undo, redo, render, caret and pixels through the C functions only, plus a test for a broken host's inputs; a unit test turns a panic into a status. The `NSView` surface is not exercised here: it needs a live AppKit view, which T12.4's Swift test provides.

### T12.4. macOS host

A SwiftPM app in the repository: an AppKit window, the Edit menu (Undo, Redo) and an `NSView` that adopts `NSTextInputClient` and hosts the renderer through the C ABI. It looks like Xcode's editor: SF Mono, the user's Xcode theme when Xcode is installed (Kerf's own otherwise), `NSTextInsertionIndicator` as the caret, Light/Dark with the system. Selection is T12.5. Done when `swift build` and `swift test` pass in CI on macOS arm64 and a test drives open, type, undo and redo through the view.

Plan:

1. `macos/` SwiftPM package (`swift-tools-version` 6.2, macOS 14 for `NSTextInsertionIndicator`): `CKerf`, a system-library module over `crates/kerf-ffi/include/kerf.h` (no copy); `KerfKit`, the library the app and tests share; `Kerf`, the executable. It links the static library `cargo build -p kerf-ffi` leaves in `target/debug`, since SwiftPM cannot build Cargo crates.
2. `KerfKit.Editor`: a Swift owner of `KerfEditor` (open, insert, command, text, cursor) that turns `KerfStatus` into thrown errors.
3. `KerfKit.EditorView`: a layer-backed, flipped `NSView` that attaches a `KerfView` with `kerf_view_attach_appkit`, resizes it at the backing scale, renders after every change, adopts `NSTextInputClient` (`insertText`, `doCommand(by:)` for delete, left, right, newline, tab), answers `undo:`/`redo:` from the Edit menu, and moves an `NSTextInsertionIndicator` to the caret rectangle. Marked (IME) text is committed as typed; showing it inline needs a marked range in the core.
4. `KerfKit.XcodeTheme`: Xcode's current Light or Dark theme name from its defaults, the theme file from the user folder or Xcode's bundle (read at run time, never copied), its background, plain text colour and font; the font family is the one in the font file, the name `cosmic-text`'s font database sees. Without Xcode, Kerf's own Light and Dark themes. The view reloads it when the appearance changes.
5. `Kerf` app: window, App menu (Quit), File > Open, Edit menu (Undo, Redo); opens the path given on the command line.
6. Tests (`swift test`): open a temp file, type, delete, undo and redo through `EditorView`, render, caret indicator placed; the theme parser on a fixture theme written by the test.
7. CI: a `macos-host` job on `xcode-27` (setup-xcode, mise Rust, `cargo build -p kerf-ffi`, `swift build`, `swift test`). Rows in `toolchain.md` and `docs/architecture.md`.

Outcome:

- `macos/`: `CKerf` (system module over `crates/kerf-ffi/include/kerf.h`), `KerfKit` (`Editor`, `EditorView`, `XcodeTheme`) and the `Kerf` app (File > Open, Edit > Undo and Redo, a path on the command line). No Swift package dependencies.
- `EditorView` renders after every change and on resize, scale and appearance changes; the caret is the system `NSTextInsertionIndicator` at Kerf's caret rectangle, and `selectedRange` converts Kerf's UTF-8 cursor to UTF-16. Key bindings past delete, left, right, newline and tab wait for core support; IME composition is committed as typed.
- `XcodeTheme`: Xcode's `XCFontAndColorCurrentTheme`/`XCFontAndColorCurrentDarkTheme`, the theme from the user folder or Xcode's bundle, background, plain text colour and font; anything missing falls back to Kerf's Light and Dark themes. The font family comes from the font's `name` table, since CoreText's name for the system monospace (".AppleSystemUIFontMonospaced") is in no file and Kerf's font database would miss it. Font weight is not passed yet (`Theme` has none).
- Fix found by the host test: a hidden window skips frames (wgpu reports `Occluded`), and the caret stayed where the last drawn frame had it. `GpuRenderer::lay_out` now updates the caret on skipped frames.
- Tests: `swift test` types, deletes, undoes and redoes through the view in a window that is not on screen, checks the caret and the UTF-16 cursor, follows Light/Dark, and parses fixture themes the test writes (Kerf never copies Apple's files). CI: the `macos-host` job on `xcode-27` builds `kerf-ffi`, then runs `swift-format lint --strict`, `swift build` and `swift test`.

### T12.5. Editor selection

Selection from the core to the screen: a selection range in `Editor` (anchor and cursor, extended by Shift-moves and replaced by typing), carried in `Frame`; a selection colour in `Theme` that `kerf-render` fills behind the selected text; C ABI calls to set and read the selection; the macOS host passes `NSColor.selectedTextBackgroundColor` (or the Xcode theme's selection colour) and maps Shift-arrows and `selectedRange`/`setSelectedRange` in `NSTextInputClient`. Done when core tests cover selection edits and undo, the offscreen render test finds the selection colour behind the selected text, and the Swift test selects and replaces text through the view.

Plan:

1. `kerf-core`: `Editor` keeps an anchor next to the cursor (equal when nothing is selected); `select_left`/`select_right` move the cursor and keep the anchor, plain moves collapse a selection to its edge, `insert` and `backspace` replace it in one buffer edit, so one undo restores it; `select(anchor, cursor)` checks both offsets. `Frame` carries the anchor and answers `selection()`.
2. `kerf-render`: `Theme.selection`; `Layout` turns the selected bytes into one rectangle per line span with `cosmic-text`'s `LayoutRun::highlight`, and a selected line break fills the rest of its row. `GpuRenderer` draws them as solid custom glyphs, which glyphon draws before the text.
3. `kerf-ffi`: `KERF_COMMAND_SELECT_LEFT`/`SELECT_RIGHT`, `kerf_editor_selection` and `kerf_editor_select` (UTF-8 offsets; `KERF_STATUS_OUT_OF_RANGE` past the end or inside a character), `KerfTheme.selection`; header re-blessed.
4. `macos/`: `moveLeftAndModifySelection:`/`moveRightAndModifySelection:`, `selectedRange` and `setSelectedRange` in UTF-16, `insertText`'s replacement range, Select All in the Edit menu, no system caret while something is selected; the selection colour is the Xcode theme's `DVTSourceTextSelectionColor`, else `NSColor.selectedTextBackgroundColor` for the appearance.

Outcome:

- Tests: core unit tests for Shift-moves, collapsing, replace and delete with undo, and refused offsets; a layout test for the rectangles across a line break and an empty line; an offscreen GPU test that finds the selection colour behind the selected letters, the letters drawn over it and none beside it; the C ABI test selects with commands and offsets, replaces and undoes, and checks a broken host's offsets and null pointers; the Swift test selects with `setSelectedRange` and Shift-Left, replaces, deletes, undoes, replaces through `insertText`'s range and selects all, with the caret hidden during a selection; theme tests read the Xcode selection colour and the system fallback.
- Steps are whole characters, as for the cursor; word, line and mouse selection, and copy and paste, come with the commands that need them.

### T3. Semantic layer: incremental parse and symbol index

`crates/kerf-semantic`. Done when an edit re-parses only the changed file incrementally, the index updates only that file's symbols, and a test measures definition and reference recall on a labelled Rust fixture.

It holds:

- tree-sitter parsing per file, with incremental re-parse from edit events (the same shape as T2.1's);
- a symbol index (definitions and references) from tree-sitter tags queries, updated per file.

Rust is the first language. Scope stays within the 500-line task cap; more languages and LSP merging are later tasks.

Plan:

1. Reuse check, done: rtok's tags code (`src/plugins/read/outline.rs`) runs `TagsContext::generate_tags`, which always parses the whole file and takes no existing tree, so it cannot be incremental. The only overlap is a few lines of extra Rust query. Write the minimum here; nothing in rtok changes.
2. Edits come in as `kerf_core::EditEvent` plus the post-edit text. T2 has merged, so a second edit-event type would duplicate it. `kerf-semantic` depends on `kerf-core` for that type only, never on the buffer.
3. `crates/kerf-semantic`:
   - `document.rs`: source and tree per file; `EditEvent` → `InputEdit`, validated against both texts; incremental re-parse with the old tree.
   - `index.rs`: per-file definitions and references from `tree_sitter_rust::TAGS_QUERY` (plus path-qualified calls), run with a `QueryCursor` on the existing tree; hard caps on file size, symbols per file and files.
   - `lib.rs`: `Semantic` with `open`, `edit`, `close` and lookups by name.
4. Tests: insert, edit and delete keep the index right; an incremental re-parse matches a fresh parse and reports changed ranges only around the edit; editing one file leaves the other files' entries untouched; bad edits and over-cap input are refused.
5. Labelled fixture `tests/fixtures/recall.rs` with `recall.labels`; a recall test that prints definition and reference recall and fails below the threshold. Report the numbers in the commit.
6. `tree-sitter` and `tree-sitter-rust` rows in `toolchain.md`; `rust.md` already lists both.
7. Verify: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, then CI on the PR.

Result: definition recall 1.000 and reference recall 1.000 on `tests/fixtures/recall.rs` (20 definitions, 27 references, no unlabelled hits). The upstream tags query alone reaches 0.750 and 0.519; the extra patterns cover path-qualified and turbofish calls, calls inside macro arguments, `impl Trait for Type`, trait method signatures, enum variants, `const` and `static`.

### T6. Plugin host on `wasm-plugin-host`

`crates/kerf-plugin`: load, activate and call WASM plugins through the shared `wasm-plugin-host` crate on cox's ABI (`cox:host/v1`, extism), not a third copy of cox's host. Manifest read without running code, lazy activation, a plugin crash disables the plugin and never the IDE (`docs/architecture.md`). The package layout depends on crates-packages#42. Done when a test plugin loads, activates on its trigger, answers a call, and a trapping plugin is disabled while the host keeps running.

Plan:

1. Depend on `wasm-plugin-host` and `cox-plugin-api` as git dependencies pinned by `rev` (`pyrlyn/crates`, `pyrlyn/cox`): neither is on crates.io, and the crates.io name `wasm_plugin_host` belongs to an unrelated crate, so the registry route in `rust.md` is closed.
2. `crates/kerf-plugin`: `manifest` (the one module that reads `plugin.toml` into cox's `PluginManifest` with `toml`, then `validate`), `trigger` (activation triggers derived from `[[language_servers]]` and `[[debug_adapters]]`, read without running code), `host` (discover through `wasm-plugin-host`, a gate for siblings T7/T10, lazy load and `cox_init` on the first matching trigger, calls through the control or event lane, three consecutive failures or a failed `cox_init` disable the plugin).
3. Tests with WAT fixtures: a manifest is read without loading wasm, a plugin activates only on its trigger, answers a call, a trapping plugin is disabled while another plugin keeps answering, project plugins are refused by the default gate, the notice list is capped.
4. Verify: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`. Rows in `toolchain.md`.

Result: `crates/kerf-plugin` (`Plugins`, `Trigger`, `Gate`, `Manifest`). Tests: `crates/kerf-plugin/tests/plugins.rs` and unit tests in its modules.

### T8. LSP results in the semantic layer

Merge language-server answers into `kerf-semantic` where tree-sitter misses (macro bodies, type-resolved references), tagged by source so tree-sitter results stay the base. Add at least one more tree-sitter language. Done when the recall test on the labelled fixture improves with a scripted language server and a second language has its own fixture.

Split when claimed (500-line task cap); both subtasks land in one PR, in order.

#### T8.1. Merge API and a second grammar in `kerf-semantic`

1. Tag every `Symbol` with its source (`TreeSitter` or `LanguageServer`). `Semantic::merge_lsp` takes `gen-lsp-types` document symbols and reference locations (`lsp-types` has had no release since 2024-06; `gen-lsp-types` is generated from the upstream schema and used by rust-analyzer and ruff), maps UTF-16 or UTF-8 positions to bytes, keeps only answers that land on an identifier with the claimed name, never replaces a tree-sitter symbol, caps answers per file, and drops a file's LSP symbols on its next edit (their positions are stale).
2. Python grammar and tags query from `tree-sitter-python`, picked by file extension; `tests/fixtures/recall.py` with its own labels and recall test.
3. The Rust fixture gains what tree-sitter cannot see (an item a macro defines, a function named as a value); the recall test reports tree-sitter alone and tree-sitter plus a scripted answer file `tests/fixtures/recall.lsp.json`.
4. Unit tests for hostile answers: out of range, wrong name, split UTF-8, over the cap.

#### T8.2. LSP client with a scripted server in `crates/kerf-lsp`

1. JSON-RPC over stdio with `Content-Length` framing, as `docs/architecture.md` §5 says: 4 MiB message cap (kill the process on abuse), a bounded inbound queue that drops notifications with a counter, an 8 KiB stderr ring, a request timeout, and `shutdown`/`exit` with a 2 s budget before kill.
2. The process starts through a small `Spawn` trait, so T7's trust check can wrap it later.
3. A scripted fake server binary replays `recall.lsp.json`; an end-to-end test runs it through the client into `kerf-semantic` and checks the same recall gain.
4. Verify: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`; numbers in the commits.

Result: `Semantic::merge_lsp` and Python in `kerf-semantic`, the client in `crates/kerf-lsp` with the scripted server `kerf-fake-lsp`. Recall on `recall.rs` (23 definitions, 31 references): tree-sitter alone 0.957 and 0.968; with the scripted answers 1.000 and 1.000, both in-process and over the wire, two symbols added, no unlabelled hits. `recall.py` (11 definitions, 15 references): 1.000 and 1.000 with the upstream query. Not done here: process counts and the restart budget of `docs/architecture.md` §5.2 (they belong to `kerf-process`), and diagnostics.

### T11. Language-server and debug-adapter binaries through ketch

Install and update language-server and debug-adapter binaries from GitHub releases with checksums, through ketch. Done when a test installs a pinned binary from a local fixture release, rejects a bad checksum, and the core resolves the installed path.

Plan:

1. Study ketch (read-only): library crate or CLI only, how it verifies checksums, how it can be pointed at a local release.
2. Add a crate: a pinned binary spec (repo, tag, per-target asset and sha256, binary name), install through ketch into a Kerf-owned root, and `resolve` returning the installed binary path the core spawns.
3. Treat every download as untrusted: cap the download and the installed size, hold the asset to the pinned sha256 before anything is unpacked, refuse a binary that resolves outside the root.
4. Tests against a local fixture release (no network): install a pinned binary, reject a bad checksum, resolve the path, refuse oversize downloads and installs.
5. Verify with `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`; update `toolchain.md`.

Result: `crates/kerf-binaries` drives the `ketch` CLI. `ketch-core` is a library but unpublished, pulls SQLite, octocrab/tokio and signature stacks, and is GPL-only, which would remove Kerf's royalty-free licence option. `Installer::install` writes a one-entry `ketch.lock` for the host target and runs `ketch sync`, the only CLI path that holds a download to a caller-chosen digest; ketch refuses a mismatch before unpacking. Downloads are capped through `KETCH_MAX_DOWNLOAD_MIB`; ketch has no unpack cap, so Kerf measures the installed tree and uninstalls it when over its cap. Six integration tests run the real ketch (from `mise.toml`, also installed in CI) against a loopback stand-in for the GitHub API.

### T9. Inspections with quick fixes

A plugin export that runs over the core's index and returns diagnostics with edits, as in IntelliJ. The contract (JSON in and out, versioned), a core-side runner that validates and caps what a plugin returns, and applying a fix as one undoable edit. Done when a test inspection reports a problem on a fixture and its fix applies and undoes.

Plan:

1. Reuse check: `kerf-core` already has the undoable replace (`TextBuffer::apply`) and `kerf-semantic` the per-file symbols (`FileSymbols`); cox's `cox-plugin-api` `abi.rs` sets the JSON style (`api` major in the input, no `deny_unknown_fields`, caps as consts). Nothing to extract.
2. New crate `crates/kerf-inspect`:
   - `contract.rs`: `InspectIn` (`api`, file path and text, the file's index slice) and `InspectOut` (diagnostics with severity, byte range, message and fixes made of edits), serde JSON in cox's ABI style; export name `kerf_inspect`.
   - `runner.rs`: an `Inspection` trait (JSON bytes in, bytes out) so T6's plugin host plugs in later; the runner builds the input from `TextBuffer` and `Semantic`, caps the reply size before parsing, then validates and caps diagnostics, fixes, edits, text lengths, ranges (in bounds, on character boundaries, edits sorted and not overlapping). A bad reply is a typed error and no diagnostics.
   - Applying a fix: its edits collapse into one replace over their span, so it is one `TextBuffer` undo step and one `EditEvent`; it refuses a buffer that changed under the span since the run.
3. Tests with an in-process fake inspection: a problem reported on a fixture, the fix applies and undoes, the index re-reads the edit; oversized, malformed, out-of-range, overlapping and over-cap replies are rejected; a stale fix is refused.
4. `serde` and `serde_json` rows in `toolchain.md`.
5. Verify: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.

Result: `crates/kerf-inspect` with the `kerf_inspect` contract (`api = 1`), the runner behind the `Inspection` trait for T6 to implement, and `Report::apply_fix`. 14 integration tests on `tests/fixtures/deprecated.rs`.

### T14. Local code completion

A fill-in-the-middle endpoint in runa, used by the editor. Chat and agents go through `cox acp`, not here. The runa side is a matching task in runa's `plan.md`; the Kerf side is a completion client with a timeout, a size cap and cancellation on the next edit. Done when Kerf's client passes tests against a fake endpoint and runa's endpoint has its own tests.

Plan:

1. runa (own repo, own worktree and PR): task P20.1 in runa's `plan.md`, a `POST /v1/infill` with llama-server's `/infill` fields (`input_prefix`, `input_suffix`, `n_predict`; reply `content`) built on the model's own FIM tokens. A model without FIM tokens gets a 400, never a chat-template imitation.
2. `crates/kerf-complete`: blocking `ureq` client (no async runtime in Kerf), run on a worker thread so the event loop only polls. Context is the prefix and suffix around the cursor read from `kerf-core`'s `TextBuffer`, each capped in bytes and snapped to char boundaries.
3. Hard caps on everything runa returns: a request timeout, a response body cap, a completion length cap, JSON shape checked. A `Completer` holds at most one request and cancels it on the next `EditEvent`; a cancelled or stale result is never delivered.
4. Tests against a fake endpoint (`httpmock`): happy path, timeout, size cap, cancel on edit, malformed reply. fmt, clippy `-D warnings`, test.
5. Crate map row in `docs/architecture.md`, `toolchain.md` rows for `ureq` and `httpmock`.

Result: runa [pyrlyn/runa#101](https://github.com/pyrlyn/runa/pull/101) adds `POST /v1/infill` (runa P20.1). `crates/kerf-complete` has `Context::around` (prefix tail and suffix head from a `TextBuffer`, snapped to character boundaries), `Client` (timeout, reply byte limit, completion cap, `content` shape check, no redirects or env proxy, API key redacted in `Debug`) and `Completer` (one worker thread, generation counter, `on_edit` cancels, only the newest queued request is sent). 8 unit tests and 9 tests against `httpmock`.

### T13. Remote development

The plugin host and the language servers run next to the code; the UI stays local. First slice: a versioned UI ↔ host protocol and a headless host process that owns buffers and the semantic layer, over any byte stream (stdio, SSH). Every message and queue from the remote side is capped. Done when tests drive open, edit, undo and a symbol query through an in-memory transport and the protocol refuses an unknown major version.

Execution plan:

1. `research.md` §6: how Zed's remote server, VS Code Remote - SSH and `cox acp` split UI and host, with primary sources; pick the wire format and the framing crate.
2. New crate `crates/kerf-remote` (crate map row in `docs/architecture.md`): JSON-RPC 2.0, one JSON object per line (ACP's stdio framing). Framing through `tokio-util`'s `LinesCodec` with a maximum line length, envelopes through `jsonrpsee-types`; no hand-written framing.
3. `initialize` carries the protocol `{ major, minor }`; an unknown major is refused with a typed error and the connection ends. Methods: `buffer/open|read|edit|undo|redo|close`, `index/lookup`.
4. Headless `Host` owns `kerf-core` buffers and `kerf-semantic` under a workspace root. Sans-IO `Connection` (bytes in, bytes out) with caps on message size (both directions), inbound queue depth, open buffers and buffer size; `serve(Read, Write)` drives it over any byte stream; `kerf-host` binary serves stdio, which SSH carries.
5. Tests through an in-memory transport: open, edit, undo, symbol query, unknown major refused, oversized message refused, full queue answers busy.
6. Verify: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`.

Result: `crates/kerf-remote` with protocol 1.0 (`initialize`, `buffer/open|read|edit|undo|redo|close`, `index/lookup`), a sans-IO `Connection`, the `Host`, and the `kerf-host` binary over stdio. Caps: 4 MiB per line in both directions (an over-cap answer becomes `REPLY_TOO_LARGE`), 64 queued requests (more answer `BUSY`), 256 buffers, 8 MiB per buffer, 10 000 symbols per lookup. 9 integration tests through an in-memory stream that hands out 7 bytes per read, plus one over the binary's stdio. About 565 lines of code without comments and blanks, over the 500-line cap: the caps and typed errors the card asks for are most of it, and the protocol cannot be tested apart from the host without a stub dispatcher that would add code.

### T7. Workspace Trust

An untrusted folder starts no language servers, debuggers or tasks until the user trusts it. A trust store in the core, a check every process spawn goes through, and denial as a typed error. Done when tests show a spawn refused in an untrusted folder and allowed after trust is granted.

Plan:

1. Add a `trust` module to `crates/kerf-core` (the card says "in the core"; `kerf-process` is still only proposed): an in-memory `TrustStore` of canonical folder roots, where a trusted root covers its subfolders (VS Code and Zed both do this).
2. One check, `TrustStore::authorize(cwd, kind)`, returns a `SpawnPermit` that only it can build, so a future `kerf-process` spawn takes the permit and cannot skip the check. Denial is a typed `TrustError`. Paths are canonicalized so `..` and symlinks cannot reach a trusted root's prefix.
3. `revoke` returns the roots it removed so a supervisor can stop their processes (§5.3). Hard cap on stored roots.
4. Persistence: a user-level `trust.toml` in the OS config directory (architecture decision 7), owned by one `trust::file` module with a committed schema.
5. Tests: spawn refused while untrusted, allowed after trust, refused after revoke; subfolder, sibling-prefix and `..` cases; the cap. Verify with `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.

Result: `kerf_core::trust` — `TrustStore` (`grant`, `revoke`, `is_trusted`, `authorize`), `SpawnPermit`, `SpawnKind`, `TrustError`, `MAX_TRUSTED_ROOTS`. Ten unit tests cover refusal before trust, a permit after trust, refusal after revoke, subfolders, a sibling sharing the prefix, `..`, a symlink out of the root, revoking a parent or a subfolder, a deleted root, unresolvable paths and the cap. Persistence followed architecture decision 7: `TrustStore::load` / `save` / `default_path` keep the roots in `kerf/trust.toml` under the OS config directory (`dirs`), with a committed JSON Schema checked by a test, a 4 MiB read cap, the 1024-root cap, absolute roots only, and an atomic write through `fs-kit`, which also gives canonical paths without the Windows `\\?\` prefix and case-folded prefix checks. Eight more tests cover the round trip, a missing file, a relative root, unknown keys and bad TOML, an oversized file, the root cap, the default path and the schema.

### T5. Return an error for the largest invalid line index

TextBuffer::line must reject every out-of-range index without panicking or wrapping its diagnostic range.

Execution plan: add a usize::MAX regression in buffer.rs; use the standard saturating integer operation for the diagnostic end; run the crate tests, clippy and formatting. Existing range checks remain authoritative; no shared helper or dependency is needed for a single integer operation.

Validation: the regression panicked on overflow before the fix. All 44 tests pass under nextest; clippy with -D warnings and formatting checks pass. Reused the standard usize::saturating_add operation and preserved ordinary diagnostic ranges.

### T10. Plugin lockfile

A committed lockfile of plugin ids, versions and package digests. Load refuses a plugin whose digest does not match. Done when tests cover write, read, a digest mismatch and a missing entry.

Result: crate `crates/kerf-plugin-lock` with `Lockfile::read`, `write`, `pin`, `unpin` and `verify`; schema in `docs/schema/kerf-plugins.lock.schema.json`. The plugin host (T6) calls `verify` with the digest it computes through `wasm-plugin-host`; that wiring belongs to T6.

Plan:

1. Reuse from cox: it has no lockfile, but it has the package digest (`cox-plugin` `package_digest`, the same function `wasm-plugin-host` carries: SHA-256 hex over the package tree). The lockfile stores that digest; it does not define a second one.
2. New crate `crates/kerf-plugin-lock`, the one module that reads and writes the lockfile: `format = 1` and a `[plugins.<id>]` table of `version` and `digest` (a table keyed by id makes a duplicate id a TOML error), TOML through `toml`, written atomically through `fs-kit`, sorted by id.
3. Validation in the types (`deny_unknown_fields`, a 64-hex digest newtype, `format` other than 1 refused); the file is repository input, so reads are capped at 1 MiB.
4. `Lockfile::verify(id, version, digest)` for the plugin host (T6) to call before it loads a package; mismatch and missing entry are typed errors.
5. JSON Schema committed under `docs/` and checked by a test through `config-schema`.
6. Tests (20): write, read, digest mismatch, version mismatch, missing entry, malformed TOML, unknown key, duplicate id, bad format, bad digest, oversized and non-UTF-8 file, stale schema. Verify with `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`.

### T16.2. Native host on the Slint view

`kerf-ffi`'s view handle draws through `kerf-slint` into a buffer the host owns, instead of a `wgpu` surface over the host's `NSView`. The macOS `EditorView` presents that buffer (as slint-bindings' `SlintNSView` does) on each display-link tick and keeps `NSTextInputClient`, `NSTextInsertionIndicator` and the Xcode theme. Done when the Rust end-to-end test over the C ABI and the headless Swift test open, type, select, undo and redo, and the header has no drift.

Plan:

1. `kerf-ffi`: `KerfView` wraps `kerf_slint::SlintView` and keeps its last frame; `kerf_view_new` replaces `kerf_view_new_offscreen` and `kerf_view_attach_appkit`; `kerf_view_render` draws into the kept frame, `kerf_view_read_rgba` copies it out, `kerf_view_caret` reports logical points. Drop `kerf-render`, `wgpu` and `pollster` from the crate.
2. Slint objects are `!Send`, so every view call checks the creating thread; `KERF_STATUS_GPU` becomes `KERF_STATUS_RENDER`, plus `KERF_STATUS_WRONG_THREAD`.
3. macOS: `EditorView` copies the frame after each render into a `CGImage` on its layer; the caret is placed in points; the package links the frameworks Slint's text stack needs instead of Metal.
4. Tests: the C ABI end-to-end test on the CPU view and a cross-thread test; the Swift test checks the frame on the layer. Header regenerated.

Result: `kerf_view_new`, `kerf_view_render` (`KERF_STATUS_NOTHING` when Slint repainted nothing), `kerf_view_read_rgba` (premultiplied RGBA8), `kerf_view_caret` in logical points; a view called or freed from another thread gets `KERF_STATUS_WRONG_THREAD` or is leaked rather than dropped there. `EditorView` renders on each edit, resize and appearance change instead of a display-link tick: the component has no animation and the caret is the system's, so nothing changes between edits. Three C ABI tests and eight Swift tests pass; `cargo clippy --workspace --all-targets -D warnings`, `cargo fmt --check` and `swift-format lint --strict` are clean.
