// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Remote development: the UI stays local, and a headless host next to the
//! code owns the buffers (`kerf-core`) and the semantic layer
//! (`kerf-semantic`). They talk JSON-RPC 2.0, one object per line, over any
//! byte stream — the host's stdio under `ssh`, a socket, or memory in tests.
//!
//! Bytes from the other side are untrusted: every line, the request queue,
//! the open buffers and every answer have a hard cap ([`Limits`]), each
//! refusal is a typed [`RemoteError`] with its own wire code, and an unknown
//! protocol major ends the connection.

// Tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod connection;
mod error;
mod host;
pub mod protocol;

pub use connection::Connection;
pub use error::RemoteError;
pub use host::Host;
pub use protocol::{Limits, PROTOCOL_MAJOR, PROTOCOL_MINOR, Version};
