// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

use jsonrpsee_types::{ErrorObject, ErrorObjectOwned};

use crate::protocol::{Version, code};

/// Why a request was refused. Each variant maps to one wire code
/// ([`code`]), so the UI never parses messages.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RemoteError {
    /// The line is not JSON.
    #[error("not JSON: {0}")]
    Parse(String),
    /// JSON, but not a JSON-RPC 2.0 request.
    #[error("not a JSON-RPC 2.0 request")]
    InvalidRequest,
    /// No such method.
    #[error("unknown method `{0}`")]
    MethodNotFound(String),
    /// Params do not fit the method.
    #[error("invalid params: {0}")]
    InvalidParams(String),
    /// A line longer than the cap; it was discarded unread.
    #[error("message is longer than the {cap}-byte cap")]
    MessageTooLarge {
        /// [`Limits::max_message_bytes`](crate::Limits::max_message_bytes).
        cap: usize,
    },
    /// More requests in flight than the host queues.
    #[error("{cap} requests are already queued")]
    Busy {
        /// [`Limits::max_queued`](crate::Limits::max_queued).
        cap: usize,
    },
    /// The client's major is not this host's; the connection ends.
    #[error("protocol {}.{} is not supported; this host speaks {}.{}", got.major, got.minor, Version::CURRENT.major, Version::CURRENT.minor)]
    UnsupportedVersion {
        /// What the client sent.
        got: Version,
    },
    /// A request before `initialize`.
    #[error("`initialize` must come first")]
    NotInitialized,
    /// A second `initialize`.
    #[error("`initialize` was already accepted")]
    AlreadyInitialized,
    /// The answer would be longer than the message cap.
    #[error("reply is longer than the {cap}-byte cap")]
    ReplyTooLarge {
        /// [`Limits::max_message_bytes`](crate::Limits::max_message_bytes).
        cap: usize,
    },
    /// Absolute, empty, or with `.`/`..`: the root is a scope, so a path never
    /// leaves it.
    #[error("`{0}` is not a relative path inside the workspace root")]
    BadPath(String),
    /// No open buffer at this path.
    #[error("`{0}` is not open")]
    NotOpen(String),
    /// [`Limits::max_buffers`](crate::Limits::max_buffers) reached.
    #[error("{cap} buffers are already open")]
    TooManyBuffers {
        /// The cap.
        cap: usize,
    },
    /// The buffer would pass [`Limits::max_buffer_bytes`](crate::Limits::max_buffer_bytes).
    #[error("buffer would be over the {cap}-byte cap")]
    BufferTooLarge {
        /// The cap.
        cap: usize,
    },
    /// The edit range does not fit the buffer.
    #[error(transparent)]
    Buffer(#[from] kerf_core::Error),
    /// Reading the file failed, or it is not UTF-8.
    #[error("cannot read `{path}`: {source}")]
    Io {
        /// Relative path.
        path: String,
        /// The cause.
        source: std::io::Error,
    },
}

impl RemoteError {
    /// The wire code.
    #[must_use]
    pub fn code(&self) -> i32 {
        match self {
            Self::Parse(_) => code::PARSE,
            Self::InvalidRequest => code::INVALID_REQUEST,
            Self::MethodNotFound(_) => code::METHOD_NOT_FOUND,
            Self::InvalidParams(_) => code::INVALID_PARAMS,
            Self::MessageTooLarge { .. } => code::MESSAGE_TOO_LARGE,
            Self::Busy { .. } => code::BUSY,
            Self::UnsupportedVersion { .. } => code::UNSUPPORTED_VERSION,
            Self::NotInitialized | Self::AlreadyInitialized => code::HANDSHAKE,
            Self::ReplyTooLarge { .. } => code::REPLY_TOO_LARGE,
            Self::BadPath(_)
            | Self::NotOpen(_)
            | Self::TooManyBuffers { .. }
            | Self::BufferTooLarge { .. }
            | Self::Buffer(_)
            | Self::Io { .. } => code::BUFFER,
        }
    }

    /// The JSON-RPC error object. A refused version carries the host's
    /// version as data, as ACP answers with the version it supports.
    pub(crate) fn to_object(&self) -> ErrorObjectOwned {
        let data = matches!(self, Self::UnsupportedVersion { .. }).then_some(Version::CURRENT);
        ErrorObject::owned(self.code(), self.to_string(), data)
    }
}
