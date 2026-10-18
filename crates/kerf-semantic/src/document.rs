// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! One open file: its text and its syntax tree, re-parsed incrementally.

use std::ops::Range;

use kerf_core::EditEvent;
use tree_sitter::{InputEdit, Parser, Point, Tree};

use crate::{Error, MAX_FILE_BYTES};

/// Text and tree of one file. The tree always belongs to `source`.
#[derive(Debug)]
pub(crate) struct Document {
    pub(crate) source: String,
    pub(crate) tree: Tree,
}

impl Document {
    pub(crate) fn parse(parser: &mut Parser, source: String) -> Result<Self, Error> {
        check_len(&source)?;
        let tree = parser.parse(&source, None).ok_or(Error::Parse)?;
        Ok(Self { source, tree })
    }

    /// Re-parses from the old tree and returns the byte ranges whose syntax
    /// changed. On error the document is unchanged.
    pub(crate) fn edit(
        &mut self,
        parser: &mut Parser,
        event: &EditEvent,
        new_source: String,
    ) -> Result<Vec<Range<usize>>, Error> {
        check_len(&new_source)?;
        let input = input_edit(&self.source, &new_source, event)?;
        // Edit a copy so a failed parse cannot leave a tree that no longer
        // matches `self.source`; a clone is a refcount bump, not a deep copy.
        let mut old = self.tree.clone();
        old.edit(&input);
        let tree = parser.parse(&new_source, Some(&old)).ok_or(Error::Parse)?;
        let changed = old.changed_ranges(&tree).map(|r| r.start_byte..r.end_byte);
        let changed = changed.collect();
        self.source = new_source;
        self.tree = tree;
        Ok(changed)
    }
}

fn check_len(source: &str) -> Result<(), Error> {
    if source.len() > MAX_FILE_BYTES {
        return Err(Error::FileTooLarge {
            len: source.len(),
            max: MAX_FILE_BYTES,
        });
    }
    Ok(())
}

/// Turns a core edit event into tree-sitter's edit. Only the edited slices
/// are scanned, so the cost follows the edit, not the file.
fn input_edit(old: &str, new: &str, event: &EditEvent) -> Result<InputEdit, Error> {
    let bad = |reason| Err(Error::BadEdit { reason });
    let Range { start, end } = event.deleted.clone();
    if start > end || end > old.len() {
        return bad("deleted range is outside the old text");
    }
    let Some(new_end) = start.checked_add(event.inserted_len) else {
        return bad("inserted length overflows");
    };
    if old.len() - (end - start) + event.inserted_len != new.len() {
        return bad("new text length does not follow from the edit");
    }
    if !old.is_char_boundary(start) || !old.is_char_boundary(end) || !new.is_char_boundary(new_end)
    {
        return bad("edit splits a UTF-8 character");
    }
    // The core gives the start position; check the column against the line
    // break it implies, which costs one line's bytes at most.
    let Some(line_start) = start.checked_sub(event.start_col) else {
        return bad("start column is past the start byte");
    };
    // Bytes, not `str`: a wrong column may land inside a UTF-8 character.
    let bytes = old.as_bytes();
    let col_ok = !bytes[line_start..start].contains(&b'\n')
        && (line_start == 0 || bytes[line_start - 1] == b'\n');
    if !col_ok {
        return bad("start column does not match the text");
    }
    let start_position = Point {
        row: event.start_line,
        column: event.start_col,
    };
    Ok(InputEdit {
        start_byte: start,
        old_end_byte: end,
        new_end_byte: new_end,
        start_position,
        old_end_position: advance(start_position, &old[start..end]),
        new_end_position: advance(start_position, &new[start..new_end]),
    })
}

/// The position just past `text` when it starts at `from`.
fn advance(from: Point, text: &str) -> Point {
    match text.rfind('\n') {
        Some(last) => Point {
            row: from.row + text.matches('\n').count(),
            column: text.len() - last - 1,
        },
        None => Point {
            row: from.row,
            column: from.column + text.len(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(deleted: Range<usize>, inserted_len: usize, line: usize, col: usize) -> EditEvent {
        EditEvent {
            deleted,
            inserted_len,
            start_line: line,
            start_col: col,
        }
    }

    #[test]
    fn positions_follow_newlines_in_both_texts() {
        let old = "ab\ncd\nef";
        let new = "ab\ncX\nY\nZf";
        // Replace "d\ne" (4..7, from line 1 column 1) with "X\nY\nZ".
        let edit = input_edit(old, new, &event(4..7, 5, 1, 1)).unwrap();
        assert_eq!(edit.start_position, Point { row: 1, column: 1 });
        assert_eq!(edit.old_end_position, Point { row: 2, column: 1 });
        assert_eq!(edit.new_end_position, Point { row: 3, column: 1 });
        assert_eq!((edit.old_end_byte, edit.new_end_byte), (7, 9));
    }

    #[test]
    fn same_line_insert_moves_only_the_column() {
        let edit = input_edit("abc", "abXYc", &event(2..2, 2, 0, 2)).unwrap();
        assert_eq!(edit.new_end_position, Point { row: 0, column: 4 });
        assert_eq!(edit.old_end_position, edit.start_position);
    }

    #[test]
    fn mismatched_edits_are_refused() {
        let cases = [
            ("abc", "abc", event(2..9, 0, 0, 2)),
            ("abc", "abcd", event(1..1, 0, 0, 1)),
            ("é", "x", event(1..2, 0, 0, 1)),
            ("éa", "éXa", event(2..2, 1, 0, 1)),
            ("ab\ncd", "ab\nXcd", event(3..3, 1, 1, 2)),
            ("ab", "aXb", event(1..1, 1, 0, 5)),
            ("abc", "abc", event(1..1, usize::MAX, 0, 1)),
        ];
        for (old, new, ev) in cases {
            assert!(
                matches!(input_edit(old, new, &ev), Err(Error::BadEdit { .. })),
                "{old:?} -> {new:?} with {ev:?} was accepted"
            );
        }
    }
}
