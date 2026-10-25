// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Local inline completion: the text around the cursor goes to a
//! fill-in-the-middle endpoint (runa's `/v1/infill`, which takes llama-server's
//! `/infill` fields) and the middle comes back as a suggestion.
//!
//! - [`Context`]: the prefix and suffix around a cursor in a
//!   [`kerf_core::TextBuffer`], capped in bytes on character boundaries.
//! - [`Client`]: one blocking request. The reply is model output and so
//!   untrusted: the call has a whole-request timeout, the body a byte limit,
//!   the reply must be the expected JSON and the completion has its own cap.
//! - [`Completer`]: the event loop's handle. Requests run on one worker
//!   thread, the loop only polls, and any edit cancels the request in flight
//!   so a suggestion for stale text is never shown.
//!
//! Chat and agents are not here: they go through `the coding agent acp`.

// Tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod client;
mod completer;
mod context;
mod error;

pub use client::{Client, Config, MAX_N_PREDICT};
pub use completer::Completer;
pub use context::Context;
pub use error::Error;
