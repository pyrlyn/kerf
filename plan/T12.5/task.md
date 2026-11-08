# T12.5. Editor selection

Selection from the core to the screen: a selection range in `Editor` (anchor and cursor, extended by Shift-moves and replaced by typing), carried in `Frame`; a selection colour in `Theme` that `kerf-render` fills behind the selected text; C ABI calls to set and read the selection; the macOS host passes `NSColor.selectedTextBackgroundColor` (or the Xcode theme's selection colour) and maps Shift-arrows and `selectedRange`/`setSelectedRange` in `NSTextInputClient`. Done when core tests cover selection edits and undo, the offscreen render test finds the selection colour behind the selected text, and the Swift test selects and replaces text through the view.

Plan:

1. `kerf-core`: `Editor` keeps an anchor next to the cursor (equal when nothing is selected); `select_left`/`select_right` move the cursor and keep the anchor, plain moves collapse a selection to its edge, `insert` and `backspace` replace it in one buffer edit, so one undo restores it; `select(anchor, cursor)` checks both offsets. `Frame` carries the anchor and answers `selection()`.
2. `kerf-render`: `Theme.selection`; `Layout` turns the selected bytes into one rectangle per line span with `cosmic-text`'s `LayoutRun::highlight`, and a selected line break fills the rest of its row. `GpuRenderer` draws them as solid custom glyphs, which glyphon draws before the text.
3. `kerf-ffi`: `KERF_COMMAND_SELECT_LEFT`/`SELECT_RIGHT`, `kerf_editor_selection` and `kerf_editor_select` (UTF-8 offsets; `KERF_STATUS_OUT_OF_RANGE` past the end or inside a character), `KerfTheme.selection`; header re-blessed.
4. `macos/`: `moveLeftAndModifySelection:`/`moveRightAndModifySelection:`, `selectedRange` and `setSelectedRange` in UTF-16, `insertText`'s replacement range, Select All in the Edit menu, no system caret while something is selected; the selection colour is the Xcode theme's `DVTSourceTextSelectionColor`, else `NSColor.selectedTextBackgroundColor` for the appearance.
