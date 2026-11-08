# Kerf

https://github.com/pyrlyn/kerf

An IDE with a small Rust core and sandboxed WASM plugins. Language servers and debug adapters run as separate processes the core starts. See `research.md`.

## Notes

- One folder per task: `T<id>/task.md` is the card, `T<id>/done.md` closes it. The active tasks: `for d in plan/*/; do [ -f "$d/done.md" ] || echo "${d%/}"; done`.
- The tasks closed before the move from `plan.md`, `todo.md` and `done.md` keep their old `done.md` entry: the card and plan in `task.md` without front matter (the old files kept none for closed tasks), the result in `done.md` with the pull requests from the git history.
- T4 was done in cox (pyrlyn/cox#367, pyrlyn/cox#385); its `done.md` records the names Kerf took from there.
- T17.2 and T17.3 reached `main` together in #38 and close with the move; #37 and #41 were closed without merging.
- T19 had no row of its own in `plan.md`: its front matter takes the highest priority and complexity of its subtasks, and its readiness counts T19.1, the one closed subtask of eleven.
