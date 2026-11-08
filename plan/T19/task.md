---
status: todo
priority: P1
complexity: 5
readiness: 9%
agent:
---

# T19. macOS IDE views per Figma

The IDE screens of the Figma file `gNfemQ9NFKFNT2fifxb6sm` (page "Kerf IDE · macOS dark", frames 01 to 14) drawn by the macOS host in AppKit, built from the page "UI Kit" and fed by T18's C ABI (on the creator's word, 2026-11-08: macOS only; Linux and Windows get their own tasks later). The UI Kit holds Kerf's own components plus the general ones merged from the Pyrlyn macOS kit (`h9sViI6qnym6iDdXDZqxcq`), all bound to the "Kerf color" and "Kerf size" variables. Screens with no Rust logic yet (Debugger, Remote host, Settings) get the model and its `kerf_*` C ABI in the same subtask, as T18 did. Each subtask lands as pull requests of at most 500 lines; views are checked by offscreen AppKit tests in `swift test`.
