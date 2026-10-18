// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The text a completion request carries: what is before and after the
//! cursor, capped so a large file never becomes a large request.

use std::ops::Range;

use kerf_core::{Error as BufferError, TextBuffer};

/// The text before and after the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// Text before the cursor: its tail, at most the prefix cap.
    pub prefix: String,
    /// Text after the cursor: its head, at most the suffix cap.
    pub suffix: String,
}

impl Context {
    /// The context around byte offset `cursor` in `buffer`. The prefix keeps
    /// the bytes nearest the cursor, the suffix likewise; a cut that would
    /// split a UTF-8 character moves toward the cursor, so either side may
    /// come out up to three bytes short of its cap.
    pub fn around(
        buffer: &TextBuffer,
        cursor: usize,
        max_prefix_bytes: usize,
        max_suffix_bytes: usize,
    ) -> Result<Self, BufferError> {
        let start = cursor.saturating_sub(max_prefix_bytes);
        let prefix = slice_snapped(buffer, start..cursor, Side::Start)?;
        let end = cursor
            .saturating_add(max_suffix_bytes)
            .min(buffer.len_bytes().max(cursor));
        let suffix = slice_snapped(buffer, cursor..end, Side::End)?;
        Ok(Self { prefix, suffix })
    }
}

/// Which end of a range may move to reach a character boundary.
#[derive(Clone, Copy)]
enum Side {
    Start,
    End,
}

/// `range` from the buffer, moving the free end toward the cursor until it
/// sits on a character boundary. The cursor end never moves: a cursor inside
/// a character is the caller's bug and comes back as the buffer's error.
fn slice_snapped(
    buffer: &TextBuffer,
    mut range: Range<usize>,
    side: Side,
) -> Result<String, BufferError> {
    loop {
        match buffer.slice(range.clone()) {
            Err(BufferError::NotCharBoundary(at)) if range.start < range.end => match side {
                Side::Start if at == range.start => range.start += 1,
                Side::End if at == range.end => range.end -= 1,
                _ => return Err(BufferError::NotCharBoundary(at)),
            },
            other => return other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_buffers_go_whole() {
        let buf = TextBuffer::with_text("fn main() {}\n");
        let ctx = Context::around(&buf, 11, 1024, 1024).unwrap();
        assert_eq!(ctx.prefix, "fn main() {");
        assert_eq!(ctx.suffix, "}\n");
    }

    #[test]
    fn caps_keep_the_text_nearest_the_cursor() {
        let buf = TextBuffer::with_text("0123456789abcdefghij");
        let ctx = Context::around(&buf, 10, 4, 3).unwrap();
        assert_eq!(ctx.prefix, "6789");
        assert_eq!(ctx.suffix, "abc");
    }

    #[test]
    fn cursor_at_either_end() {
        let buf = TextBuffer::with_text("abc");
        let start = Context::around(&buf, 0, 8, 8).unwrap();
        assert_eq!((start.prefix.as_str(), start.suffix.as_str()), ("", "abc"));
        let end = Context::around(&buf, 3, 8, 8).unwrap();
        assert_eq!((end.prefix.as_str(), end.suffix.as_str()), ("abc", ""));
        let empty = Context::around(&TextBuffer::new(), 0, 8, 8).unwrap();
        assert_eq!((empty.prefix.as_str(), empty.suffix.as_str()), ("", ""));
    }

    #[test]
    fn a_cut_inside_a_character_moves_toward_the_cursor() {
        // "é" and "ж" are two bytes, "😀" four: caps of 1 and 3 bytes land
        // inside one of them.
        let buf = TextBuffer::with_text("aé|ж😀b");
        let cursor = "aé".len();
        let ctx = Context::around(&buf, cursor, 1, 1).unwrap();
        assert_eq!((ctx.prefix.as_str(), ctx.suffix.as_str()), ("", "|"));
        let ctx = Context::around(&buf, cursor + 1, 1, 3).unwrap();
        assert_eq!((ctx.prefix.as_str(), ctx.suffix.as_str()), ("|", "ж"));
        let ctx = Context::around(&buf, cursor + 3, 3, 3).unwrap();
        assert_eq!((ctx.prefix.as_str(), ctx.suffix.as_str()), ("|ж", ""));
    }

    #[test]
    fn a_bad_cursor_is_refused() {
        let buf = TextBuffer::with_text("é");
        assert_eq!(
            Context::around(&buf, 1, 8, 8),
            Err(BufferError::NotCharBoundary(1))
        );
        assert!(matches!(
            Context::around(&buf, 9, 8, 8),
            Err(BufferError::OutOfBounds { .. })
        ));
    }
}
