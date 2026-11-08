# T17. Linux host on GTK 4

The Linux host for the UI shell (from `roadmap.md`, approved with T12's native direction): a GTK 4 window over T12's C ABI with the Slint editor surface from T16, looking like GNOME Text Editor. Done when the Linux app opens a file, shows it in the Slint-drawn view, takes typing, selection, undo and redo through `kerf-core`, takes input-method text through GTK, draws a GTK-style caret, and headless tests of that path run in CI on `ubuntu-latest`.

The creator's choices (2026-10-20): the host is Rust on gtk4-rs calling `kerf-ffi`'s C functions; the look is GNOME Text Editor's on plain GTK 4 (no libadwaita).

Split when claimed (500-line task cap) into three stacked pull requests. Research for all three is in `research.md` §7, "Linux host (T17)". Locally the Linux tests run in a Lima VM from `template:ubuntu-24.04` (GTK 4.14, as on the CI runner).
