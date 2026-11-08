# T12.5. Editor selection: done

Outcome:

- Tests: core unit tests for Shift-moves, collapsing, replace and delete with undo, and refused offsets; a layout test for the rectangles across a line break and an empty line; an offscreen GPU test that finds the selection colour behind the selected letters, the letters drawn over it and none beside it; the C ABI test selects with commands and offsets, replaces and undoes, and checks a broken host's offsets and null pointers; the Swift test selects with `setSelectedRange` and Shift-Left, replaces, deletes, undoes, replaces through `insertText`'s range and selects all, with the caret hidden during a selection; theme tests read the Xcode selection colour and the system fallback.
- Steps are whole characters, as for the cursor; word, line and mouse selection, and copy and paste, come with the commands that need them.

Commits: pyrlyn/kerf#29 (`d28109a`).
