// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Kerf's inspections, as in IntelliJ: a plugin export reads one file and its
//! slice of the core's index and returns diagnostics with quick fixes.
//!
//! - [`contract`]: the versioned JSON the `kerf_inspect` export takes and
//!   returns.
//! - [`run`]: builds the input from a [`kerf_core::TextBuffer`] and a
//!   [`kerf_semantic::Semantic`], calls an [`Inspection`], and checks the
//!   reply. Plugin output is untrusted: its size, every list and string, and
//!   every span are capped or checked, and a bad reply is refused whole.
//! - [`Report::apply_fix`]: one fix as one undoable buffer edit.
//!
//! The plugin host is behind [`Inspection`], so this crate needs no WASM.

// Tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod contract;
mod error;
mod runner;

pub use error::{CallError, Error};
pub use runner::{
    Inspection, MAX_CODE_BYTES, MAX_DIAGNOSTICS, MAX_EDITS, MAX_FIX_TEXT_BYTES, MAX_FIXES,
    MAX_MESSAGE_BYTES, MAX_REPLY_BYTES, MAX_TITLE_BYTES, Report, run,
};
