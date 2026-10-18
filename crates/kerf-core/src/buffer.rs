// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The text buffer: a rope-backed sequence of bytes with replace-edits and
//! undo/redo. Owns the rope and the history stacks; every applied edit leaves
//! through [`EditEvent`] so callers never see the rope type.

use std::ops::Range;

use ropey::Rope;

use crate::error::Error;
use crate::event::EditEvent;

/// Undo depth. Edits can arrive from plugins and language-server replies, so
/// history is capped; the oldest entry is dropped when the cap is hit.
const MAX_UNDO: usize = 1_000;

/// A text buffer over a rope, with undo and redo.
#[derive(Debug)]
pub struct TextBuffer {
    text: Rope,
    /// How to walk back each applied edit, newest on top.
    undo: Vec<Replace>,
    /// How to re-apply each undone edit, newest on top.
    redo: Vec<Replace>,
}

/// One replace-edit: drop the bytes in `deleted`, insert `inserted` instead.
/// Undo history stores the inverse of what the caller applied, so undoing is
/// just another replace — no full-buffer snapshots.
#[derive(Debug, Clone)]
struct Replace {
    deleted: Range<usize>,
    inserted: String,
}

impl TextBuffer {
    /// An empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: Rope::new(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// A buffer holding `s`.
    #[must_use]
    pub fn with_text(s: &str) -> Self {
        Self {
            text: Rope::from_str(s),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// Length of the whole buffer in bytes.
    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.text.len_bytes()
    }

    /// `true` when the buffer holds no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.len_bytes() == 0
    }

    /// Number of lines. A trailing newline opens a final empty line
    /// (ropey's counting: `"a\n"` has two lines).
    #[must_use]
    pub fn len_lines(&self) -> usize {
        self.text.len_lines()
    }

    /// The whole buffer as a `String`.
    #[must_use]
    pub fn to_text(&self) -> String {
        self.text.to_string()
    }

    /// Bytes in `range`, as owned text. `range` must sit on UTF-8 character
    /// boundaries.
    pub fn slice(&self, range: Range<usize>) -> Result<String, Error> {
        self.check_range(&range)?;
        let start = self.text.byte_to_char(range.start);
        let end = self.text.byte_to_char(range.end);
        Ok(self.text.slice(start..end).to_string())
    }

    /// One line by 0-based index, including its line ending if present.
    pub fn line(&self, line_idx: usize) -> Result<String, Error> {
        if line_idx >= self.text.len_lines() {
            return Err(Error::OutOfBounds {
                start: line_idx,
                end: line_idx + 1,
                len: self.text.len_lines(),
            });
        }
        Ok(self.text.line(line_idx).to_string())
    }

    /// Replace the bytes in `deleted` with `inserted`, record the inverse on
    /// the undo stack, clear the redo stack, and return the event describing
    /// the edit in pre-edit coordinates.
    pub fn apply(&mut self, deleted: Range<usize>, inserted: &str) -> Result<EditEvent, Error> {
        let event = self.event_for(&deleted, inserted.len())?;
        let inverse = self.apply_replace(Replace {
            deleted,
            inserted: inserted.to_owned(),
        });
        self.push_undo(inverse);
        self.redo.clear();
        Ok(event)
    }

    /// Undo the last applied edit and return the event for the inverse edit
    /// that was applied to get here. `None` when the undo stack is empty.
    pub fn undo(&mut self) -> Option<EditEvent> {
        let entry = self.undo.pop()?;
        let event = self.event_for(&entry.deleted, entry.inserted.len()).ok()?;
        let inverse = self.apply_replace(entry);
        self.redo.push(inverse);
        Some(event)
    }

    /// Redo the last undone edit and return the event for the re-applied edit.
    /// `None` when the redo stack is empty.
    pub fn redo(&mut self) -> Option<EditEvent> {
        let entry = self.redo.pop()?;
        let event = self.event_for(&entry.deleted, entry.inserted.len()).ok()?;
        let inverse = self.apply_replace(entry);
        self.push_undo(inverse);
        Some(event)
    }

    /// Number of edits that can still be undone.
    #[must_use]
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Number of edits that can still be redone.
    #[must_use]
    pub fn redo_depth(&self) -> usize {
        self.redo.len()
    }

    /// Build the public event for a replace about to be applied.
    fn event_for(&self, deleted: &Range<usize>, inserted_len: usize) -> Result<EditEvent, Error> {
        self.check_range(deleted)?;
        let start_line = self.text.byte_to_line(deleted.start);
        let start_col = deleted.start - self.text.line_to_byte(start_line);
        Ok(EditEvent {
            deleted: deleted.clone(),
            inserted_len,
            start_line,
            start_col,
        })
    }

    /// Apply one replace and return its inverse (how to put the old text back).
    fn apply_replace(&mut self, edit: Replace) -> Replace {
        let start_char = self.text.byte_to_char(edit.deleted.start);
        let end_char = self.text.byte_to_char(edit.deleted.end);
        let removed = self.text.slice(start_char..end_char).to_string();
        self.text.remove(start_char..end_char);
        if !edit.inserted.is_empty() {
            self.text.insert(start_char, &edit.inserted);
        }
        Replace {
            deleted: edit.deleted.start..edit.deleted.start + edit.inserted.len(),
            inserted: removed,
        }
    }

    /// Record an undo step, dropping the oldest when history is at the cap.
    fn push_undo(&mut self, entry: Replace) {
        if self.undo.len() == MAX_UNDO {
            self.undo.remove(0);
        }
        self.undo.push(entry);
    }

    /// Reject a range that is reversed, past the end, or off a character
    /// boundary. Ropey would panic on any of these.
    fn check_range(&self, range: &Range<usize>) -> Result<(), Error> {
        let len = self.text.len_bytes();
        if range.start > range.end || range.end > len {
            return Err(Error::OutOfBounds {
                start: range.start,
                end: range.end,
                len,
            });
        }
        self.check_boundary(range.start)?;
        self.check_boundary(range.end)?;
        Ok(())
    }

    /// A UTF-8 continuation byte (`0b10xxxxxx`) never starts a character.
    fn check_boundary(&self, byte_idx: usize) -> Result<(), Error> {
        let len = self.text.len_bytes();
        if byte_idx > len {
            return Err(Error::OutOfBounds {
                start: byte_idx,
                end: byte_idx,
                len,
            });
        }
        if byte_idx == 0 || byte_idx == len {
            return Ok(());
        }
        if self.text.byte(byte_idx) & 0xC0 == 0x80 {
            return Err(Error::NotCharBoundary(byte_idx));
        }
        Ok(())
    }
}

impl From<&str> for TextBuffer {
    fn from(s: &str) -> Self {
        Self::with_text(s)
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use super::{MAX_UNDO, TextBuffer};
    use crate::error::Error;

    #[test]
    fn insert_places_text_at_the_byte_offset() {
        let mut buf = TextBuffer::with_text("ac");
        buf.apply(1..1, "b").expect("insert");
        assert_eq!(buf.to_text(), "abc");
    }

    #[test]
    fn replace_swaps_the_deleted_range() {
        let mut buf = TextBuffer::with_text("hello world");
        buf.apply(6..11, "kerf").expect("replace");
        assert_eq!(buf.to_text(), "hello kerf");
    }

    #[test]
    fn delete_removes_the_range() {
        let mut buf = TextBuffer::with_text("keep drop keep");
        buf.apply(5..9, "").expect("delete");
        assert_eq!(buf.to_text(), "keep  keep");
    }

    #[test]
    fn undo_restores_previous_text() {
        let mut buf = TextBuffer::with_text("before");
        buf.apply(0..6, "after").expect("replace");
        assert_eq!(buf.to_text(), "after");
        buf.undo().expect("undo");
        assert_eq!(buf.to_text(), "before");
    }

    #[test]
    fn redo_reapplies_the_undone_edit() {
        let mut buf = TextBuffer::with_text("a");
        buf.apply(1..1, "b").expect("insert");
        buf.undo().expect("undo");
        assert_eq!(buf.to_text(), "a");
        buf.redo().expect("redo");
        assert_eq!(buf.to_text(), "ab");
    }

    #[test]
    fn undo_then_edit_clears_redo() {
        let mut buf = TextBuffer::with_text("x");
        buf.apply(1..1, "y").expect("first edit");
        buf.undo().expect("undo");
        assert_eq!(buf.redo_depth(), 1);
        buf.apply(0..0, "z").expect("second edit");
        assert_eq!(buf.redo_depth(), 0);
        assert!(buf.redo().is_none());
        assert_eq!(buf.to_text(), "zx");
    }

    #[test]
    fn undo_on_empty_history_returns_none() {
        let mut buf = TextBuffer::with_text("static");
        assert!(buf.undo().is_none());
        assert_eq!(buf.to_text(), "static");
    }

    #[test]
    fn redo_on_empty_history_returns_none() {
        let mut buf = TextBuffer::with_text("static");
        assert!(buf.redo().is_none());
        assert_eq!(buf.to_text(), "static");
    }

    #[test]
    fn event_reports_deleted_range_and_inserted_len() {
        let mut buf = TextBuffer::with_text("abcdef");
        let event = buf.apply(2..4, "XYZ").expect("replace");
        assert_eq!(event.deleted, 2..4);
        assert_eq!(event.inserted_len, 3);
        assert_eq!(event.inserted(), 2..5);
    }

    #[test]
    fn event_line_and_col_match_the_pre_edit_start() {
        let mut buf = TextBuffer::with_text("ab\ncd\nef");
        // 'd' is line 1, column 1 (0-based).
        let event = buf.apply(4..5, "DD").expect("replace");
        assert_eq!(event.start_line, 1);
        assert_eq!(event.start_col, 1);
        assert_eq!(event.deleted, 4..5);
    }

    #[test]
    fn pure_insert_event_has_empty_deleted_range() {
        let mut buf = TextBuffer::with_text("hi");
        let event = buf.apply(2..2, "!").expect("insert");
        assert_eq!(event.deleted, 2..2);
        assert_eq!(event.inserted_len, 1);
    }

    #[test]
    fn undo_event_describes_the_inverse_edit() {
        let mut buf = TextBuffer::with_text("abcd");
        buf.apply(1..3, "X").expect("replace");
        let event = buf.undo().expect("undo");
        // Undo removes "X" (1 byte at offset 1) and restores "bc" (2 bytes).
        assert_eq!(event.deleted, 1..2);
        assert_eq!(event.inserted_len, 2);
    }

    #[test]
    fn apply_rejects_range_past_the_end() {
        let mut buf = TextBuffer::with_text("abc");
        let err = buf.apply(2..4, "z").expect_err("out of bounds");
        assert!(matches!(err, Error::OutOfBounds { .. }));
    }

    #[test]
    fn apply_rejects_reversed_range() {
        let mut buf = TextBuffer::with_text("abc");
        // Built by hand: the literal `2..1` is a clippy lint, not this API.
        let reversed = Range { start: 2, end: 1 };
        let err = buf.apply(reversed, "z").expect_err("reversed");
        assert!(matches!(err, Error::OutOfBounds { .. }));
    }

    #[test]
    fn apply_rejects_mid_character_start() {
        // "é" is 2 bytes; offset 1 splits it.
        let mut buf = TextBuffer::with_text("é");
        let err = buf.apply(1..1, "x").expect_err("mid character");
        assert!(matches!(err, Error::NotCharBoundary(1)));
    }

    #[test]
    fn apply_rejects_mid_character_end() {
        let mut buf = TextBuffer::with_text("aé");
        // "aé" is a(1) + é(2) = 3 bytes; range 0..2 ends inside "é".
        let err = buf.apply(0..2, "x").expect_err("mid character end");
        assert!(matches!(err, Error::NotCharBoundary(2)));
    }

    #[test]
    fn slice_on_char_boundaries_returns_utf8_text() {
        // "naïve café": naïve is 6 bytes (ï is 2), café is 6 bytes (é is 2).
        let buf = TextBuffer::with_text("naïve café");
        assert_eq!(buf.slice(0..6).expect("slice"), "naïve");
        assert_eq!(buf.slice(6..12).expect("slice"), " café");
    }

    #[test]
    fn slice_rejects_mid_character_range() {
        let buf = TextBuffer::with_text("é");
        let err = buf.slice(0..1).expect_err("mid character");
        assert!(matches!(err, Error::NotCharBoundary(1)));
    }

    #[test]
    fn apply_on_unicode_text_keeps_surrounding_bytes_intact() {
        let mut buf = TextBuffer::with_text("héllo wörld");
        // Replace "ö" (2 bytes) with "oe".
        let ö = "héllo wörld".find('ö').expect("char present");
        buf.apply(ö..ö + 2, "oe").expect("replace");
        assert_eq!(buf.to_text(), "héllo woerld");
    }

    #[test]
    fn undo_restores_unicode_text_exactly() {
        let original = "日本語テキスト";
        let mut buf = TextBuffer::with_text(original);
        let start = "日本語テキスト".find('テ').expect("char present");
        buf.apply(start..start + 3, "文").expect("replace");
        buf.undo().expect("undo");
        assert_eq!(buf.to_text(), original);
    }

    #[test]
    fn line_returns_text_with_its_ending() {
        // A trailing newline opens a final empty line (ropey's counting).
        let buf = TextBuffer::with_text("one\ntwo\n");
        assert_eq!(buf.len_lines(), 3);
        assert_eq!(buf.line(0).expect("line"), "one\n");
        assert_eq!(buf.line(1).expect("line"), "two\n");
        assert_eq!(buf.line(2).expect("line"), "");
    }

    #[test]
    fn line_out_of_range_is_an_error() {
        let buf = TextBuffer::with_text("only");
        assert!(matches!(buf.line(1), Err(Error::OutOfBounds { .. })));
    }

    #[test]
    fn empty_buffer_round_trips_through_apply_and_undo() {
        let mut buf = TextBuffer::new();
        assert!(buf.is_empty());
        buf.apply(0..0, "hello").expect("insert");
        assert_eq!(buf.to_text(), "hello");
        buf.undo().expect("undo");
        assert!(buf.is_empty());
    }

    #[test]
    fn undo_history_is_capped() {
        let mut buf = TextBuffer::new();
        for _ in 0..MAX_UNDO + 10 {
            buf.apply(0..0, "x").expect("insert");
        }
        assert_eq!(buf.undo_depth(), MAX_UNDO);
    }

    #[test]
    fn chained_undo_and_redo_restore_each_state() {
        let mut buf = TextBuffer::with_text("base");
        buf.apply(4..4, "1").expect("edit 1");
        buf.apply(5..5, "2").expect("edit 2");
        assert_eq!(buf.to_text(), "base12");
        buf.undo().expect("undo 2");
        assert_eq!(buf.to_text(), "base1");
        buf.undo().expect("undo 1");
        assert_eq!(buf.to_text(), "base");
        buf.redo().expect("redo 1");
        buf.redo().expect("redo 2");
        assert_eq!(buf.to_text(), "base12");
    }
}
