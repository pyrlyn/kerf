// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Kerf's core without UI: the text buffer, the editor cursor, the rendering
//! trait a shell implements, replace-edits with undo and redo,
//! edit events, bounded queues, the single-threaded event loop and the
//! Workspace Trust check every process spawn passes.
//!
//! Everything a plugin, a language server or a repository writes later is
//! untrusted; this crate is the trusted boundary that turns those bytes into
//! one owned buffer with a hard-capped undo stack and queues that never grow
//! past their cap.
//!
//! The public surface is [`TextBuffer`] (reads and edits), [`Editor`] (a
//! buffer with a cursor), [`Render`] / [`Frame`] (what a shell draws),
//! [`EditEvent`] (what the semantic layer consumes), [`BoundedQueue`] / [`EventLoop`],
//! [`TrustStore`] / [`SpawnPermit`], [`read_text`] (a capped text-file read),
//! and [`Error`]. No rope type is exported.

// Unit tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod buffer;
mod editor;
mod error;
pub mod event;
pub mod event_loop;
mod file;
pub mod queue;
pub mod render;
pub mod trust;

pub use buffer::TextBuffer;
pub use editor::Editor;
pub use error::Error;
pub use event::EditEvent;
pub use event_loop::{DEFAULT_CAPACITY, EventLoop};
pub use file::{ReadError, read_text};
pub use queue::{BoundedQueue, QueueFull};
pub use render::{Frame, Render};
pub use trust::{
    MAX_TRUST_FILE_BYTES, MAX_TRUSTED_ROOTS, SpawnKind, SpawnPermit, TrustError, TrustFileError,
    TrustStore,
};
