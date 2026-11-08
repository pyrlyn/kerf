# T16. Slint editor surface

The creator's direction of 2026-10-20 (`research.md` §7): the editor surface is a Slint component, embedded in the native host through slint-bindings, instead of Kerf's own GPU renderer. The native window, menus, input (`NSTextInputClient`), the system caret and the Xcode look stay as T12 built them. The subtasks together are done when the macOS app opens a file, shows it in the Slint-drawn editor view, takes typing, selection, undo and redo through `kerf-core`, headless tests cover that path, and `kerf-render` with its GPU dependencies is gone.
