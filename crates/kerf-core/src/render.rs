// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The rendering contract (`docs/architecture.md` §2.1): the core decides
//! what is shown, a shell crate draws it. Keeping the trait here is what lets
//! `kerf-core` stay free of any UI toolkit.

use std::ops::Range;

use crate::buffer::TextBuffer;

/// Everything one redraw shows, borrowed from the core for the call.
#[derive(Debug, Clone, Copy)]
pub struct Frame<'a> {
    /// The buffer being edited.
    pub buffer: &'a TextBuffer,
    /// Cursor as a byte offset; always on a character boundary.
    pub cursor: usize,
    /// 0-based line of the cursor.
    pub line: usize,
    /// Cursor column in bytes from the start of its line.
    pub col: usize,
    /// The selection's fixed end, a byte offset on a character boundary;
    /// equal to `cursor` when nothing is selected.
    pub anchor: usize,
}

impl Frame<'_> {
    /// The selected bytes in buffer order; `None` when nothing is selected.
    #[must_use]
    pub fn selection(&self) -> Option<Range<usize>> {
        span(self.anchor, self.cursor)
    }
}

/// The bytes between two offsets in either order; `None` when they meet.
pub(crate) fn span(a: usize, b: usize) -> Option<Range<usize>> {
    (a != b).then(|| a.min(b)..a.max(b))
}

/// A surface that draws a [`Frame`]. The shell implements it for its toolkit;
/// tests implement it to record what would have been drawn.
pub trait Render {
    /// Why a draw failed. A GPU renderer can run out of atlas space or lose
    /// its device; a recorder never fails and uses `Infallible`.
    type Error;

    /// Draw `frame`, replacing whatever was drawn before.
    fn render(&mut self, frame: &Frame<'_>) -> Result<(), Self::Error>;
}
