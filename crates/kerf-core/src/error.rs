// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The one error type for buffer edits. Kept apart from the buffer so callers
//! can match on a stable enum without importing rope internals.

/// What can go wrong when applying a replace-edit or reading a range.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// A byte range is reversed or extends past the buffer.
    #[error("byte range {start}..{end} is out of bounds (buffer is {len} bytes)")]
    OutOfBounds {
        /// First byte of the rejected range.
        start: usize,
        /// One past the last byte of the rejected range.
        end: usize,
        /// Buffer length in bytes at the time of the call.
        len: usize,
    },
    /// A byte offset falls inside a multi-byte UTF-8 character.
    #[error("byte offset {0} is not a UTF-8 character boundary")]
    NotCharBoundary(usize),
}
