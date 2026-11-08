---
status: todo
priority: P1
complexity: 4
readiness: 0%
agent:
---

# T19.2. Window shell and chrome

The frames "Navigation and chrome" and "Surfaces and lists" as AppKit views (window toolbar with run destination pill and activity view, navigator tabs, tree rows, section labels, editor tabs, jump bar, terminal tabs, status bar, banner, sheet, popover, list and settings rows), assembled into the main window around the existing `EditorView`: navigator on the left, editor with tabs and jump bar, bottom panel, status bar. Done when a test builds the window for an opened file and finds every region at the frame's sizes.
