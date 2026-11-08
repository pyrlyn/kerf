# T17.2. Editor view widget

`EditorView`, a `GtkWidget` subclass: `kerf_view_*` draws into a `GdkMemoryTexture` shown in the widget's snapshot; keys through a key controller with a `GtkIMMulticontext` (commits as typed, no inline preedit); GTK's caret over the frame with GtkTextView's blink, hidden over a selection; the look re-applied on theme and font changes. Done when tests under Xvfb type through the input method and keys, select, undo, redo and read the frame's pixels and the caret back.
