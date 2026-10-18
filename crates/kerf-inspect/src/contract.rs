// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The inspection export's JSON payloads, in the style of cox's ABI v1
//! (`cox-plugin-api` `abi.rs`): the input carries the ABI major as `api`, and
//! no type denies unknown fields, so a minor addition (a new optional field)
//! keeps `api = 1` because an older peer ignores what it does not know
//! (`docs/architecture.md` §7).
//!
//! Offsets are UTF-8 byte offsets into the file text sent in the input, the
//! same coordinates as `kerf-core`'s buffer and `kerf-semantic`'s index.

use serde::{Deserialize, Serialize};

/// The ABI major of this contract.
pub const API: u32 = 1;

/// The plugin export the host calls with [`InspectIn`]; it returns
/// [`InspectOut`].
pub const EXPORT: &str = "kerf_inspect";

/// `kerf_inspect` input: one file and its slice of the index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectIn {
    /// The ABI major the host speaks.
    pub api: u32,
    /// The file under inspection.
    pub file: FileIn,
    /// The file's symbols from the core's index, in source order.
    pub symbols: Vec<SymbolIn>,
    /// True when the index dropped symbols past its per-file cap, so an
    /// inspection must not conclude that a name is unused.
    pub truncated: bool,
}

/// The file an inspection runs over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileIn {
    /// Workspace path, for messages only; the plugin cannot open it.
    pub path: String,
    /// The whole text, as the buffer holds it.
    pub text: String,
}

/// One definition or reference from the index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolIn {
    /// The identifier.
    pub name: String,
    /// The tags-query kind: `function`, `method`, `call`, ….
    pub kind: String,
    /// Definition or reference.
    pub role: SymbolRole,
    /// Bytes of the identifier.
    pub name_span: Span,
    /// Bytes of the whole tagged node.
    pub span: Span,
    /// 0-based line of the identifier.
    pub line: usize,
}

/// Whether a symbol defines a name or uses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolRole {
    /// Where the name is defined.
    Definition,
    /// Where the name is used.
    Reference,
}

/// A byte range `start..end` in the input's file text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// First byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
}

/// `kerf_inspect` output. Every field may be omitted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InspectOut {
    /// Problems found, in any order.
    pub diagnostics: Vec<Diagnostic>,
}

/// One problem an inspection reports, with the fixes it offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// The inspection's own id for this kind of problem, for suppression and
    /// settings (`deprecated-call`).
    pub code: String,
    /// How loud the editor shows it.
    pub severity: Severity,
    /// What is wrong, as shown to the user.
    pub message: String,
    /// Where it is.
    pub span: Span,
    /// Quick fixes, in the order the plugin ranks them.
    #[serde(default)]
    pub fixes: Vec<Fix>,
}

/// IntelliJ's highlight levels, which inspections are modelled on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Code that does not compile or run.
    Error,
    /// Code that works but is likely wrong.
    Warning,
    /// A style or cleanup hint.
    WeakWarning,
    /// Information only, no highlight.
    Info,
}

/// One quick fix: edits applied together as a single undo step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fix {
    /// The menu text (`Replace with new_api`).
    pub title: String,
    /// Replacements in input coordinates; they must not overlap.
    pub edits: Vec<Edit>,
}

/// Replace the bytes in `span` with `text`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edit {
    /// Bytes to drop; empty for a pure insert.
    pub span: Span,
    /// Text to put there; empty for a pure delete.
    #[serde(default)]
    pub text: String,
}
