// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Kerf's core without UI: the text buffer, replace-edits with undo and redo,
//! and the edit-event type. Everything a plugin, a language server or a
//! repository writes later is untrusted; this crate is the trusted boundary
//! that turns those bytes into one owned buffer with a hard-capped undo stack.
//!
//! The public surface is [`TextBuffer`] (reads and edits), [`EditEvent`] (what
//! T2.2 and T3 consume) and [`Error`]. No rope type is exported.

// Unit tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod buffer;
mod error;
mod event;

pub use buffer::TextBuffer;
pub use error::Error;
pub use event::EditEvent;
