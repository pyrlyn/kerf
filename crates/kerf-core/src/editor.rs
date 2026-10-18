// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! One buffer with a cursor, and the edits typing makes. Shells only map
//! their input to these calls, so every shell (the window, a remote client,
//! tests) edits the same way.
//!
//! Steps are whole characters, not grapheme clusters; that is enough for the
//! first window and keeps the core free of a segmentation table.

use std::ops::Range;

use crate::buffer::TextBuffer;
use crate::error::Error;
use crate::event::EditEvent;
use crate::render::{Frame, span};

/// A buffer and the cursor that edits it.
#[derive(Debug, Default)]
pub struct Editor {
    buffer: TextBuffer,
    /// Byte offset. Only this type moves it, and only to character
    /// boundaries, so edits at the cursor never fail on a split character.
    cursor: usize,
    /// The selection's fixed end, kept to the same boundaries as `cursor`;
    /// equal to it when nothing is selected, so there is no third state.
    anchor: usize,
}

impl Editor {
    /// An editor over `buffer`, cursor at the start.
    #[must_use]
    pub fn new(buffer: TextBuffer) -> Self {
        Self {
            buffer,
            cursor: 0,
            anchor: 0,
        }
    }

    /// The buffer, read-only: every edit goes through the editor so the
    /// cursor stays valid.
    #[must_use]
    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    /// Cursor as a byte offset.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The selection's fixed end as a byte offset; equal to the cursor when
    /// nothing is selected.
    #[must_use]
    pub fn anchor(&self) -> usize {
        self.anchor
    }

    /// The selected bytes in buffer order; `None` when nothing is selected.
    #[must_use]
    pub fn selection(&self) -> Option<Range<usize>> {
        span(self.anchor, self.cursor)
    }

    /// Select from `anchor` to `cursor` (equal offsets select nothing). Both
    /// come from a shell, so both are checked before either is kept.
    pub fn select(&mut self, anchor: usize, cursor: usize) -> Result<(), Error> {
        self.buffer
            .check_range(&(anchor.min(cursor)..anchor.max(cursor)))?;
        (self.anchor, self.cursor) = (anchor, cursor);
        Ok(())
    }

    /// Replace the selection, or insert at the cursor, with `text` and move
    /// the cursor past it. One edit, so one undo restores the selected text.
    pub fn insert(&mut self, text: &str) -> Result<EditEvent, Error> {
        let range = self.selection().unwrap_or(self.cursor..self.cursor);
        let event = self.buffer.apply(range.clone(), text)?;
        self.place(range.start + text.len());
        Ok(event)
    }

    /// Delete the selection, or the character before the cursor. `None` at
    /// the start with nothing selected.
    pub fn backspace(&mut self) -> Option<EditEvent> {
        let range = match self.selection() {
            Some(range) => range,
            None => self.buffer.prev_boundary(self.cursor)?..self.cursor,
        };
        let event = self.buffer.apply(range.clone(), "").ok()?;
        self.place(range.start);
        Some(event)
    }

    /// Undo the last edit and put the cursor after the restored text, where
    /// the user was when they made that edit.
    pub fn undo(&mut self) -> Option<EditEvent> {
        let event = self.buffer.undo()?;
        self.place(event.inserted().end);
        Some(event)
    }

    /// Redo the last undone edit; the cursor lands after it.
    pub fn redo(&mut self) -> Option<EditEvent> {
        let event = self.buffer.redo()?;
        self.place(event.inserted().end);
        Some(event)
    }

    /// Move the cursor one character left; `false` at the start. With a
    /// selection the cursor goes to its start instead, as on every desktop.
    pub fn move_left(&mut self) -> bool {
        if let Some(range) = self.selection() {
            self.place(range.start);
            return true;
        }
        let moved = self.select_left();
        self.anchor = self.cursor;
        moved
    }

    /// Move the cursor one character right; `false` at the end. With a
    /// selection the cursor goes to its end.
    pub fn move_right(&mut self) -> bool {
        if let Some(range) = self.selection() {
            self.place(range.end);
            return true;
        }
        let moved = self.select_right();
        self.anchor = self.cursor;
        moved
    }

    /// Move the cursor one character left and keep the anchor, growing or
    /// shrinking the selection; `false` at the start.
    pub fn select_left(&mut self) -> bool {
        self.buffer
            .prev_boundary(self.cursor)
            .map(|at| self.cursor = at)
            .is_some()
    }

    /// Move the cursor one character right and keep the anchor; `false` at
    /// the end.
    pub fn select_right(&mut self) -> bool {
        self.buffer
            .next_boundary(self.cursor)
            .map(|at| self.cursor = at)
            .is_some()
    }

    /// What a renderer draws for the current state.
    #[must_use]
    pub fn frame(&self) -> Frame<'_> {
        let (line, col) = self.buffer.line_col(self.cursor);
        Frame {
            buffer: &self.buffer,
            cursor: self.cursor,
            line,
            col,
            anchor: self.anchor,
        }
    }

    /// Put the cursor at `at` with nothing selected.
    fn place(&mut self, at: usize) {
        (self.anchor, self.cursor) = (at, at);
    }
}

#[cfg(test)]
mod tests {
    use super::Editor;
    use crate::buffer::TextBuffer;
    use crate::render::{Frame, Render};

    #[test]
    fn typing_inserts_at_the_cursor_and_advances_it() {
        let mut ed = Editor::new(TextBuffer::with_text("world"));
        ed.insert("hello ").unwrap();
        assert_eq!(ed.buffer().to_text(), "hello world");
        assert_eq!(ed.cursor(), 6);
    }

    #[test]
    fn backspace_removes_one_whole_character() {
        let mut ed = Editor::new(TextBuffer::new());
        ed.insert("aé").unwrap();
        ed.backspace().unwrap();
        assert_eq!(ed.buffer().to_text(), "a");
        assert_eq!(ed.cursor(), 1);
        ed.backspace().unwrap();
        assert!(ed.backspace().is_none());
    }

    #[test]
    fn undo_and_redo_move_the_cursor_with_the_text() {
        let mut ed = Editor::new(TextBuffer::with_text("ac"));
        assert!(ed.move_right());
        ed.insert("b").unwrap();
        ed.undo().unwrap();
        assert_eq!((ed.buffer().to_text().as_str(), ed.cursor()), ("ac", 1));
        ed.redo().unwrap();
        assert_eq!((ed.buffer().to_text().as_str(), ed.cursor()), ("abc", 2));
    }

    #[test]
    fn cursor_moves_stop_at_the_ends() {
        let mut ed = Editor::new(TextBuffer::with_text("日"));
        assert!(!ed.move_left());
        assert!(ed.move_right());
        assert_eq!(ed.cursor(), 3);
        assert!(!ed.move_right());
    }

    #[test]
    fn shift_moves_select_and_plain_moves_collapse() {
        let mut ed = Editor::new(TextBuffer::with_text("aéb"));
        assert!(ed.select_right());
        assert!(ed.select_right());
        assert_eq!(ed.selection(), Some(0..3));
        assert!(ed.select_left());
        assert_eq!(ed.selection(), Some(0..1));
        assert!(ed.move_right());
        assert_eq!((ed.cursor(), ed.selection()), (1, None));
        ed.select(3, 1).unwrap();
        assert!(ed.move_left());
        assert_eq!((ed.cursor(), ed.selection()), (1, None));
        assert!(ed.select_left());
        assert_eq!(ed.selection(), Some(0..1));
    }

    #[test]
    fn typing_and_backspace_replace_the_selection_in_one_undo_step() {
        let mut ed = Editor::new(TextBuffer::with_text("hello world"));
        ed.select(11, 6).unwrap();
        ed.insert("kerf").unwrap();
        assert_eq!(
            (ed.buffer().to_text().as_str(), ed.cursor()),
            ("hello kerf", 10)
        );
        assert_eq!(ed.selection(), None);
        ed.undo().unwrap();
        assert_eq!(ed.buffer().to_text(), "hello world");
        ed.select(0, 6).unwrap();
        ed.backspace().unwrap();
        assert_eq!((ed.buffer().to_text().as_str(), ed.cursor()), ("world", 0));
        ed.undo().unwrap();
        assert_eq!(ed.buffer().to_text(), "hello world");
    }

    #[test]
    fn a_selection_off_a_character_or_past_the_end_is_refused() {
        let mut ed = Editor::new(TextBuffer::with_text("é"));
        assert!(ed.select(0, 1).is_err());
        assert!(ed.select(0, 9).is_err());
        assert_eq!((ed.cursor(), ed.selection()), (0, None));
        ed.select(2, 0).unwrap();
        assert_eq!(ed.frame().selection(), Some(0..2));
    }

    /// Records what it was asked to draw, as a shell would.
    #[derive(Default)]
    struct Recorder(Vec<(String, usize, usize)>);

    impl Render for Recorder {
        type Error = std::convert::Infallible;

        fn render(&mut self, frame: &Frame<'_>) -> Result<(), Self::Error> {
            self.0.push((frame.buffer.to_text(), frame.line, frame.col));
            Ok(())
        }
    }

    #[test]
    fn frame_reports_the_cursor_line_and_column() {
        let mut ed = Editor::new(TextBuffer::new());
        ed.insert("one\ntwo").unwrap();
        let mut out = Recorder::default();
        out.render(&ed.frame()).unwrap();
        assert_eq!(out.0, vec![("one\ntwo".to_owned(), 1, 3)]);
    }
}
