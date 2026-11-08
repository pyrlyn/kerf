// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Why a completion did not arrive. Every failure is a value the editor can
//! show or drop; none of them is fatal to the editor.

/// What can go wrong when building a request or reading its reply.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The cursor or a context range is not valid in the buffer.
    #[error(transparent)]
    Buffer(#[from] kerf_core::Error),
    /// A [`Config`](crate::Config) field is out of range.
    #[error("invalid completion config: {0}")]
    Config(&'static str),
    /// The whole request took longer than the configured timeout.
    #[error("completion request timed out")]
    Timeout,
    /// The reply body is larger than the configured limit.
    #[error("completion reply is over the {limit}-byte limit")]
    ReplyTooLarge {
        /// The configured limit in bytes.
        limit: u64,
    },
    /// The endpoint answered with a non-success status.
    #[error("completion endpoint answered HTTP {status}: {message}")]
    Status {
        /// HTTP status code.
        status: u16,
        /// The start of the error body, for the user to read.
        message: String,
    },
    /// The request did not complete: connection refused, DNS, TLS.
    #[error("completion request failed: {0}")]
    Transport(String),
    /// The reply is not the JSON shape `/v1/infill` answers with.
    #[error("completion reply is malformed: {0}")]
    BadReply(String),
    /// The completion text is longer than the configured cap.
    #[error("completion is {len} bytes, over the {limit}-byte cap")]
    TooLong {
        /// Length of the completion in bytes.
        len: usize,
        /// The configured cap in bytes.
        limit: usize,
    },
    /// The worker thread could not start or is gone.
    #[error("completion worker is not running: {0}")]
    Worker(String),
}
