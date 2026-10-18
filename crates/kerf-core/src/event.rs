//! Edit events shared by the buffer (T2.1) and the semantic layer (T3).
//!
//! Only the shape lives here — the rope buffer itself is T2.1's. Keeping the
//! event independent of buffer internals lets T3 re-parse from events without
//! depending on how the buffer stores text.

use std::ops::Range;

/// One buffer edit, in **byte** coordinates of the buffer **before** the edit.
///
/// Coordinate system:
///
/// - `deleted` is a byte range in the pre-edit buffer. A pure insert is an
///   empty range (`start == end`) at the insertion point; a pure delete has
///   `inserted_len == 0`.
/// - `inserted_len` is the byte length of the text that replaces `deleted`.
///   After the edit the document holds `inserted_len` bytes where `deleted`
///   used to be; later offsets in the same edit are not implied (one event is
///   one edit).
///
/// Byte offsets, not chars or line/column: the buffer is a byte store and the
/// incremental parser (T3) applies the same ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditEvent {
    /// Byte range removed, measured before the edit is applied.
    pub deleted: Range<usize>,
    /// Byte length of the inserted replacement text.
    pub inserted_len: usize,
}

#[cfg(test)]
mod tests {
    use super::EditEvent;

    #[test]
    fn insert_is_empty_deleted_range() {
        let ev = EditEvent {
            deleted: 4..4,
            inserted_len: 3,
        };
        assert_eq!(ev.deleted.start, ev.deleted.end);
        assert_eq!(ev.inserted_len, 3);
    }

    #[test]
    fn delete_is_zero_inserted_len() {
        let ev = EditEvent {
            deleted: 2..7,
            inserted_len: 0,
        };
        assert_eq!(ev.deleted.len(), 5);
        assert_eq!(ev.inserted_len, 0);
    }
}
