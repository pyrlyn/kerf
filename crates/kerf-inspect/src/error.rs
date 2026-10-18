// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Why an inspection run or a quick fix was refused. A refused reply yields no
//! diagnostics at all: half of an untrusted reply is not trusted either.

use std::path::PathBuf;

/// What a plugin call returns when it fails before replying (trap, timeout,
/// busy lane). Boxed so the plugin host's own error type converts with `?`.
pub type CallError = Box<dyn std::error::Error + Send + Sync>;

/// Errors from [`run`](crate::run) and [`Report::apply_fix`](crate::Report::apply_fix).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The file has no entry in the semantic index.
    #[error("{path} is not open in the semantic index")]
    NotOpen {
        /// The file asked for.
        path: PathBuf,
    },
    /// The input could not be encoded.
    #[error("cannot encode the inspection input: {0}")]
    Encode(#[source] serde_json::Error),
    /// The plugin call failed before it replied.
    #[error("inspection call failed: {0}")]
    Call(#[source] CallError),
    /// The reply is bigger than [`MAX_REPLY_BYTES`](crate::MAX_REPLY_BYTES).
    #[error("inspection reply is {len} bytes, more than {max}")]
    ReplyTooLarge {
        /// Reply size.
        len: usize,
        /// The cap.
        max: usize,
    },
    /// The reply is not JSON of the contract's shape.
    #[error("inspection reply is malformed: {0}")]
    Malformed(#[source] serde_json::Error),
    /// A list in the reply is longer than its cap.
    #[error("{what}: {len} entries, more than {max}")]
    TooMany {
        /// Which list.
        what: &'static str,
        /// Its length.
        len: usize,
        /// The cap.
        max: usize,
    },
    /// A string in the reply is longer than its cap.
    #[error("{what}: {len} bytes, more than {max}")]
    TooLong {
        /// Which string.
        what: &'static str,
        /// Its length.
        len: usize,
        /// The cap.
        max: usize,
    },
    /// A string shown to the user is empty or holds control characters.
    #[error("{what} is empty or holds control characters")]
    BadText {
        /// Which string.
        what: &'static str,
    },
    /// A span is reversed or reaches past the file.
    #[error("span {start}..{end} is out of bounds (file is {len} bytes)")]
    OutOfBounds {
        /// First byte.
        start: usize,
        /// One past the last byte.
        end: usize,
        /// File length.
        len: usize,
    },
    /// A span splits a UTF-8 character.
    #[error("byte offset {0} is not a UTF-8 character boundary")]
    NotCharBoundary(usize),
    /// A fix has no edits.
    #[error("a fix has no edits")]
    EmptyFix,
    /// Two edits of one fix overlap.
    #[error("edits {first}..{first_end} and {second}..{second_end} overlap")]
    Overlap {
        /// Start of the earlier edit.
        first: usize,
        /// End of the earlier edit.
        first_end: usize,
        /// Start of the later edit.
        second: usize,
        /// End of the later edit.
        second_end: usize,
    },
    /// No fix at that index.
    #[error("no fix {fix} on diagnostic {diagnostic}")]
    NoSuchFix {
        /// Diagnostic index.
        diagnostic: usize,
        /// Fix index.
        fix: usize,
    },
    /// The buffer changed under the fix since the inspection ran.
    #[error("the buffer changed since the inspection ran; run it again")]
    Stale,
    /// The buffer refused the edit.
    #[error(transparent)]
    Buffer(#[from] kerf_core::Error),
}
