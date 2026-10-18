// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The rendering contract (`docs/architecture.md` §2.1): the core decides
//! what is shown, a shell crate draws it. Keeping the trait here is what lets
//! `kerf-core` stay free of any UI toolkit.

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
}

/// A surface that draws a [`Frame`]. The shell implements it for its toolkit;
/// tests implement it to record what would have been drawn.
pub trait Render {
    /// Draw `frame`, replacing whatever was drawn before.
    fn render(&mut self, frame: &Frame<'_>);
}
