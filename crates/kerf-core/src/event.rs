// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The edit-event type the event loop (T2.2) and the semantic layer (T3)
//! consume. It is deliberately self-contained: no rope type crosses this
//! boundary, so a consumer never depends on buffer internals.

use std::ops::Range;

/// One applied replace-edit.
///
/// # Coordinate system
///
/// Offsets are **byte** offsets into the buffer — not chars, not UTF-16 units.
///
/// - [`deleted`](Self::deleted) is the byte range the edit removed, in the
///   buffer **before** the edit (pre-edit coordinates). `deleted.len()` is the
///   number of bytes removed.
/// - [`inserted_len`](Self::inserted_len) is the byte length of the text that
///   replaced it. After the edit that text occupies
///   `deleted.start .. deleted.start + inserted_len` (post-edit coordinates).
/// - [`start_line`](Self::start_line) and [`start_col`](Self::start_col)
///   locate `deleted.start` in the **pre-edit** buffer: 0-based line index and
///   0-based byte column within that line. They exist so the semantic layer
///   can map an edit to a line without re-scanning.
///
/// Undo and redo return the same shape: the event describes the edit that was
/// applied to reach the new state, not the logical "user typed" action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditEvent {
    /// Byte range removed, in pre-edit coordinates.
    pub deleted: Range<usize>,
    /// Byte length of the inserted text.
    pub inserted_len: usize,
    /// 0-based line of `deleted.start`, pre-edit.
    pub start_line: usize,
    /// 0-based byte column of `deleted.start` within `start_line`, pre-edit.
    pub start_col: usize,
}

impl EditEvent {
    /// Byte range the inserted text occupies after the edit (post-edit).
    #[must_use]
    pub fn inserted(&self) -> Range<usize> {
        self.deleted.start..self.deleted.start + self.inserted_len
    }
}

#[cfg(test)]
mod tests {
    use super::EditEvent;

    #[test]
    fn inserted_range_starts_where_the_deletion_did() {
        let event = EditEvent {
            deleted: 4..7,
            inserted_len: 2,
            start_line: 0,
            start_col: 4,
        };
        assert_eq!(event.inserted(), 4..6);
    }

    #[test]
    fn pure_insert_has_empty_deleted_range() {
        let event = EditEvent {
            deleted: 3..3,
            inserted_len: 5,
            start_line: 1,
            start_col: 2,
        };
        assert_eq!(event.deleted.len(), 0);
        assert_eq!(event.inserted(), 3..8);
    }
}
