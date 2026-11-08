---
status: todo
priority: P2
complexity: 5
readiness: 0%
agent:
---

# T19.9. Debugger

Screen 11 with its model: `kerf-debug`, a Debug Adapter Protocol client started only after the trust check (breakpoints, threads and call stack, variables, continue, step over, into and out, stop; every queue capped), `kerf_debug_*` in `kerf-ffi`, and the debug bar, call stack and variables views with the execution line in the editor. Done when tests drive a stub adapter through the model, the C ABI and the views.
