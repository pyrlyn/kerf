// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! One buffer with a cursor, and the edits typing makes. Shells only map
//! their input to these calls, so every shell (the window, a remote client,
//! tests) edits the same way.
//!
//! Steps are whole characters, not grapheme clusters; that is enough for the
//! first window and keeps the core free of a segmentation table.

use crate::buffer::TextBuffer;
use crate::error::Error;
use crate::event::EditEvent;
use crate::render::Frame;

/// A buffer and the cursor that edits it.
#[derive(Debug, Default)]
pub struct Editor {
    buffer: TextBuffer,
    /// Byte offset. Only this type moves it, and only to character
    /// boundaries, so edits at the cursor never fail on a split character.
    cursor: usize,
}

impl Editor {
    /// An editor over `buffer`, cursor at the start.
    #[must_use]
    pub fn new(buffer: TextBuffer) -> Self {
        Self { buffer, cursor: 0 }
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

    /// Insert `text` at the cursor and move the cursor past it.
    pub fn insert(&mut self, text: &str) -> Result<EditEvent, Error> {
        let event = self.buffer.apply(self.cursor..self.cursor, text)?;
        self.cursor += text.len();
        Ok(event)
    }

    /// Delete the character before the cursor. `None` at the start.
    pub fn backspace(&mut self) -> Option<EditEvent> {
        let start = self.buffer.prev_boundary(self.cursor)?;
        let event = self.buffer.apply(start..self.cursor, "").ok()?;
        self.cursor = start;
        Some(event)
    }

    /// Undo the last edit and put the cursor after the restored text, where
    /// the user was when they made that edit.
    pub fn undo(&mut self) -> Option<EditEvent> {
        let event = self.buffer.undo()?;
        self.cursor = event.inserted().end;
        Some(event)
    }

    /// Redo the last undone edit; the cursor lands after it.
    pub fn redo(&mut self) -> Option<EditEvent> {
        let event = self.buffer.redo()?;
        self.cursor = event.inserted().end;
        Some(event)
    }

    /// Move the cursor one character left; `false` at the start.
    pub fn move_left(&mut self) -> bool {
        self.buffer
            .prev_boundary(self.cursor)
            .map(|at| self.cursor = at)
            .is_some()
    }

    /// Move the cursor one character right; `false` at the end.
    pub fn move_right(&mut self) -> bool {
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
        }
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
