// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The wire contract: JSON-RPC 2.0, one JSON object per line, as ACP does
//! over stdio (research.md §6). Unknown JSON fields are ignored both ways, so
//! a field added in a minor version never breaks the other side.

use serde::{Deserialize, Serialize};

/// Breaking changes raise the major; the host refuses any other major.
pub const PROTOCOL_MAJOR: u32 = 1;
/// Additions (a method, an optional field) raise the minor and keep the
/// connection.
pub const PROTOCOL_MINOR: u32 = 0;

/// Method names.
pub mod method {
    /// Must be the first request; carries [`InitializeParams`](super::InitializeParams).
    pub const INITIALIZE: &str = "initialize";
    /// [`PathParams`](super::PathParams) → [`LenResult`](super::LenResult).
    pub const OPEN: &str = "buffer/open";
    /// [`PathParams`](super::PathParams) → [`ReadResult`](super::ReadResult).
    pub const READ: &str = "buffer/read";
    /// [`EditParams`](super::EditParams) → [`LenResult`](super::LenResult).
    pub const EDIT: &str = "buffer/edit";
    /// [`PathParams`](super::PathParams) → [`ChangedResult`](super::ChangedResult).
    pub const UNDO: &str = "buffer/undo";
    /// [`PathParams`](super::PathParams) → [`ChangedResult`](super::ChangedResult).
    pub const REDO: &str = "buffer/redo";
    /// [`PathParams`](super::PathParams) → [`ChangedResult`](super::ChangedResult).
    pub const CLOSE: &str = "buffer/close";
    /// [`LookupParams`](super::LookupParams) → [`LookupResult`](super::LookupResult).
    pub const LOOKUP: &str = "index/lookup";
}

/// Error codes beyond JSON-RPC's own (`-32700`…`-32600`), so a UI can match
/// on the number instead of the message.
pub mod code {
    use jsonrpsee_types::ErrorCode;

    /// The line is not JSON.
    pub const PARSE: i32 = ErrorCode::ParseError.code();
    /// JSON, but not a JSON-RPC 2.0 request.
    pub const INVALID_REQUEST: i32 = ErrorCode::InvalidRequest.code();
    /// No such method.
    pub const METHOD_NOT_FOUND: i32 = ErrorCode::MethodNotFound.code();
    /// Params do not fit the method.
    pub const INVALID_PARAMS: i32 = ErrorCode::InvalidParams.code();
    /// A line longer than the message cap.
    pub const MESSAGE_TOO_LARGE: i32 = ErrorCode::OversizedRequest.code();
    /// More requests in flight than the host queues.
    pub const BUSY: i32 = ErrorCode::ServerIsBusy.code();
    /// The client's protocol major is not this host's.
    pub const UNSUPPORTED_VERSION: i32 = -32010;
    /// A request other than `initialize` came first, or `initialize` came twice.
    pub const HANDSHAKE: i32 = -32011;
    /// The answer would be longer than the message cap.
    pub const REPLY_TOO_LARGE: i32 = -32012;
    /// A buffer request failed: bad path, not open, a cap, a bad range, I/O.
    pub const BUFFER: i32 = -32020;
}

/// A protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    /// Breaking changes.
    pub major: u32,
    /// Additions.
    pub minor: u32,
}

impl Version {
    /// The version this crate speaks.
    pub const CURRENT: Self = Self {
        major: PROTOCOL_MAJOR,
        minor: PROTOCOL_MINOR,
    };
}

/// `initialize` params, sent by the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    /// The newest version the UI speaks.
    pub protocol: Version,
}

/// `initialize` result; an unknown major gets [`code::UNSUPPORTED_VERSION`]
/// with this as its error data instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    /// The version the host speaks.
    pub protocol: Version,
}

/// A buffer, by its path relative to the host's workspace root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathParams {
    /// Relative, `/`-separated, without `.` or `..`.
    pub path: String,
}

/// Replace the bytes `start..end` with `text`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditParams {
    /// Relative path of an open buffer.
    pub path: String,
    /// Byte offset, on a UTF-8 character boundary.
    pub start: usize,
    /// Byte offset, on a UTF-8 character boundary.
    pub end: usize,
    /// Inserted text.
    pub text: String,
}

/// The buffer's length after the request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LenResult {
    /// Bytes.
    pub len_bytes: usize,
}

/// A buffer's whole text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResult {
    /// UTF-8 text.
    pub text: String,
}

/// Whether the request changed anything (an empty undo stack does not).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedResult {
    /// `false` when there was nothing to do.
    pub changed: bool,
}

/// Definition or reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolRole {
    /// Where a name is defined.
    Definition,
    /// Where a name is used.
    Reference,
}

/// Find a name in the semantic index of the open buffers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupParams {
    /// Exact identifier.
    pub name: String,
    /// Definitions or references.
    pub role: SymbolRole,
}

/// One symbol in a lookup answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolInfo {
    /// Relative path of the buffer.
    pub path: String,
    /// The identifier.
    pub name: String,
    /// The tags-query kind: `function`, `call`, ….
    pub kind: String,
    /// 0-based line of the identifier.
    pub line: usize,
    /// Byte offset where the identifier starts.
    pub start: usize,
    /// Byte offset where the identifier ends.
    pub end: usize,
}

/// A lookup answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupResult {
    /// By path, then source order.
    pub symbols: Vec<SymbolInfo>,
    /// More symbols matched than [`Limits::max_symbols`].
    pub truncated: bool,
}

/// Caps on everything the other side can make the host hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Longest line in either direction, newline excluded.
    pub max_message_bytes: usize,
    /// Requests in flight the host queues; more get [`code::BUSY`].
    pub max_queued: usize,
    /// Buffers open at once.
    pub max_buffers: usize,
    /// Largest buffer, on open and after an edit.
    pub max_buffer_bytes: usize,
    /// Most symbols in one lookup answer.
    pub max_symbols: usize,
}

impl Default for Limits {
    /// The single-message cap matches the one approved for language-server
    /// traffic (`docs/architecture.md` §5.2); the buffer cap is the largest
    /// file the semantic layer parses.
    fn default() -> Self {
        Self {
            max_message_bytes: 4 * 1024 * 1024,
            max_queued: 64,
            max_buffers: 256,
            max_buffer_bytes: kerf_semantic::MAX_FILE_BYTES,
            max_symbols: 10_000,
        }
    }
}
