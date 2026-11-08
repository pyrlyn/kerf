// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The one error type of the semantic layer.

use std::path::PathBuf;

/// Why the semantic layer refused a file or an edit. A refused call leaves the
/// file's tree and symbols as they were.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// No grammar for this path's extension.
    #[error("no grammar for {path}")]
    UnsupportedLanguage {
        /// The refused path.
        path: PathBuf,
    },
    /// The text is over [`MAX_FILE_BYTES`](crate::MAX_FILE_BYTES).
    #[error("file is {len} bytes, the cap is {max}")]
    FileTooLarge {
        /// Byte length of the refused text.
        len: usize,
        /// The cap.
        max: usize,
    },
    /// [`MAX_FILES`](crate::MAX_FILES) files are already open.
    #[error("{max} files are already open")]
    TooManyFiles {
        /// The cap.
        max: usize,
    },
    /// The path was never opened, or was closed.
    #[error("{path} is not open")]
    NotOpen {
        /// The unknown path.
        path: PathBuf,
    },
    /// The edit event does not fit the old or the new text.
    #[error("edit does not match the text: {reason}")]
    BadEdit {
        /// What did not match.
        reason: &'static str,
    },
    /// The grammar or its query failed to load; a build problem, not input.
    #[error("grammar: {0}")]
    Grammar(String),
    /// A language server answered in a position encoding the layer does not
    /// speak (only UTF-8, UTF-16 and UTF-32 exist in LSP 3.17).
    #[error("unsupported position encoding {0:?}")]
    UnsupportedEncoding(String),
    /// A language server's answer is for an older text of the file.
    #[error("answer is for version {answered}, {path} is at {current}")]
    Stale {
        /// The file.
        path: PathBuf,
        /// The version the answer was asked for.
        answered: u64,
        /// The file's version now.
        current: u64,
    },
    /// tree-sitter returned no tree.
    #[error("parse produced no tree")]
    Parse,
}
