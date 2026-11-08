---
status: todo
priority: P1
complexity: 5
readiness: 0%
agent:
---

# T19.5. Diagnostics, inline completion and quick fixes

Screens 04 to 06: diagnostics from `kerf-lsp` as line tints, issue badges and the issue navigator; inline completion from `kerf-complete` as ghost text accepted with Tab; inspections from `kerf-inspect` with a quick-fix popover. Adds `kerf_diagnostics_*`, `kerf_completion_*` and `kerf_inspect_*` to `kerf-ffi`. Done when tests show a diagnostic, accept a completion and apply a quick fix through the C ABI and the view.
