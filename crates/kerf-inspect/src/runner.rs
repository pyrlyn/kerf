// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Calls an inspection, checks its reply against the file it was given, and
//! applies a chosen fix to the buffer.

use std::ops::Range;
use std::path::Path;

use kerf_core::{EditEvent, TextBuffer};
use kerf_semantic::{Role, Semantic, Symbol};

use crate::contract::{
    API, Diagnostic, Edit, FileIn, InspectIn, InspectOut, Span, SymbolIn, SymbolRole,
};
use crate::error::{CallError, Error};

/// Largest reply read, checked before it is parsed so a plugin cannot make
/// the core allocate a parse tree of its choosing.
pub const MAX_REPLY_BYTES: usize = 1024 * 1024;
/// Most diagnostics per reply.
pub const MAX_DIAGNOSTICS: usize = 1_000;
/// Most fixes per diagnostic.
pub const MAX_FIXES: usize = 8;
/// Most edits per fix.
pub const MAX_EDITS: usize = 1_000;
/// Most inserted bytes per fix, across its edits.
pub const MAX_FIX_TEXT_BYTES: usize = 256 * 1024;
/// Longest diagnostic code.
pub const MAX_CODE_BYTES: usize = 64;
/// Longest diagnostic message.
pub const MAX_MESSAGE_BYTES: usize = 1024;
/// Longest fix title.
pub const MAX_TITLE_BYTES: usize = 256;

/// One inspection behind its `kerf_inspect` export: JSON [`InspectIn`] in,
/// JSON [`InspectOut`] out. Raw bytes rather than typed values so the reply
/// is parsed here, under this crate's caps, whoever produced it — the plugin
/// host (T6) or an in-process test double.
pub trait Inspection {
    /// Runs the inspection once and returns its raw reply.
    ///
    /// # Errors
    ///
    /// Whatever stopped the call before a reply (trap, deadline, busy lane).
    fn call(&mut self, input: &[u8]) -> Result<Vec<u8>, CallError>;
}

/// Checked diagnostics for one file, tied to the text they were computed on.
#[derive(Debug, Clone)]
pub struct Report {
    /// The text sent to the inspection, so a fix can tell whether the buffer
    /// still matches it.
    text: String,
    diagnostics: Vec<Diagnostic>,
}

/// Runs `inspection` over the file at `path`: its text from `buffer`, its
/// symbols from `semantic`, which must be up to date with `buffer`.
///
/// # Errors
///
/// [`Error::NotOpen`] if `semantic` does not know the file,
/// [`Error::Call`] if the call failed, or any reply check; a refused reply
/// gives no diagnostics.
pub fn run(
    inspection: &mut dyn Inspection,
    semantic: &Semantic,
    path: &Path,
    buffer: &TextBuffer,
) -> Result<Report, Error> {
    let file = semantic.symbols(path).ok_or_else(|| Error::NotOpen {
        path: path.to_path_buf(),
    })?;
    let input = InspectIn {
        api: API,
        file: FileIn {
            path: path.to_string_lossy().into_owned(),
            text: buffer.to_text(),
        },
        symbols: file.symbols.iter().map(symbol_in).collect(),
        truncated: file.truncated,
    };
    let bytes = serde_json::to_vec(&input).map_err(Error::Encode)?;
    let reply = inspection.call(&bytes).map_err(Error::Call)?;
    let text = input.file.text;
    let diagnostics = check_reply(&reply, &text)?;
    Ok(Report { text, diagnostics })
}

impl Report {
    /// The diagnostics, checked, with each fix's edits sorted by position.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Applies fix `fix` of diagnostic `diagnostic` to `buffer` as one
    /// replace, so one undo takes the whole fix back. Returns the edit event
    /// to hand to the semantic layer.
    ///
    /// # Errors
    ///
    /// [`Error::NoSuchFix`], or [`Error::Stale`] when the buffer's length or
    /// the text under the fix differs from what the inspection saw.
    pub fn apply_fix(
        &self,
        diagnostic: usize,
        fix: usize,
        buffer: &mut TextBuffer,
    ) -> Result<EditEvent, Error> {
        let edits = self
            .diagnostics
            .get(diagnostic)
            .and_then(|d| d.fixes.get(fix))
            .map(|f| f.edits.as_slice())
            .ok_or(Error::NoSuchFix { diagnostic, fix })?;
        let (Some(first), Some(last)) = (edits.first(), edits.last()) else {
            return Err(Error::EmptyFix);
        };
        // Sorted and disjoint, so the last edit ends furthest right.
        let span = first.span.start..last.span.end;
        let seen = self.text.get(span.clone()).ok_or(Error::Stale)?;
        // Same length and same bytes under the span means every offset the
        // fix names still points where the inspection meant; edits elsewhere
        // that kept the length do not matter.
        if buffer.len_bytes() != self.text.len() || buffer.slice(span.clone())? != seen {
            return Err(Error::Stale);
        }
        let replacement = merge(&self.text, &span, edits).ok_or(Error::Stale)?;
        Ok(buffer.apply(span, &replacement)?)
    }
}

/// The text the span becomes once every edit is applied: the untouched gaps
/// between edits come from `text`.
fn merge(text: &str, span: &Range<usize>, edits: &[Edit]) -> Option<String> {
    let mut out = String::new();
    let mut at = span.start;
    for edit in edits {
        out.push_str(text.get(at..edit.span.start)?);
        out.push_str(&edit.text);
        at = edit.span.end;
    }
    Some(out)
}

fn symbol_in(s: &Symbol) -> SymbolIn {
    SymbolIn {
        name: s.name.clone(),
        kind: s.kind.clone(),
        role: match s.role {
            Role::Definition => SymbolRole::Definition,
            Role::Reference => SymbolRole::Reference,
        },
        name_span: span_of(&s.name_range),
        span: span_of(&s.range),
        line: s.line,
    }
}

fn span_of(r: &Range<usize>) -> Span {
    Span {
        start: r.start,
        end: r.end,
    }
}

/// Parses a reply and checks every cap, string and span against `text`.
fn check_reply(reply: &[u8], text: &str) -> Result<Vec<Diagnostic>, Error> {
    if reply.len() > MAX_REPLY_BYTES {
        return Err(Error::ReplyTooLarge {
            len: reply.len(),
            max: MAX_REPLY_BYTES,
        });
    }
    let out: InspectOut = serde_json::from_slice(reply).map_err(Error::Malformed)?;
    let mut diagnostics = out.diagnostics;
    check_count("diagnostics", diagnostics.len(), MAX_DIAGNOSTICS)?;
    for d in &mut diagnostics {
        check_text("diagnostic code", &d.code, MAX_CODE_BYTES)?;
        check_text("diagnostic message", &d.message, MAX_MESSAGE_BYTES)?;
        check_span(d.span, text)?;
        check_count("fixes", d.fixes.len(), MAX_FIXES)?;
        for fix in &mut d.fixes {
            check_text("fix title", &fix.title, MAX_TITLE_BYTES)?;
            check_edits(&mut fix.edits, text)?;
        }
    }
    Ok(diagnostics)
}

/// Sorts a fix's edits and checks they are in bounds, disjoint and capped.
fn check_edits(edits: &mut [Edit], text: &str) -> Result<(), Error> {
    if edits.is_empty() {
        return Err(Error::EmptyFix);
    }
    check_count("edits", edits.len(), MAX_EDITS)?;
    let inserted: usize = edits.iter().map(|e| e.text.len()).sum();
    if inserted > MAX_FIX_TEXT_BYTES {
        return Err(Error::TooLong {
            what: "fix text",
            len: inserted,
            max: MAX_FIX_TEXT_BYTES,
        });
    }
    for edit in edits.iter() {
        check_span(edit.span, text)?;
    }
    // Stable, so two inserts at one offset keep the plugin's order.
    edits.sort_by_key(|e| (e.span.start, e.span.end));
    for pair in edits.windows(2) {
        if let [a, b] = pair
            && a.span.end > b.span.start
        {
            return Err(Error::Overlap {
                first: a.span.start,
                first_end: a.span.end,
                second: b.span.start,
                second_end: b.span.end,
            });
        }
    }
    Ok(())
}

fn check_count(what: &'static str, len: usize, max: usize) -> Result<(), Error> {
    if len > max {
        return Err(Error::TooMany { what, len, max });
    }
    Ok(())
}

/// Strings the editor shows the user: capped, non-empty, and free of control
/// characters that could fake or garble the UI (line breaks and tabs pass).
fn check_text(what: &'static str, s: &str, max: usize) -> Result<(), Error> {
    if s.len() > max {
        return Err(Error::TooLong {
            what,
            len: s.len(),
            max,
        });
    }
    if s.trim().is_empty() || s.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Err(Error::BadText { what });
    }
    Ok(())
}

fn check_span(span: Span, text: &str) -> Result<(), Error> {
    if span.start > span.end || span.end > text.len() {
        return Err(Error::OutOfBounds {
            start: span.start,
            end: span.end,
            len: text.len(),
        });
    }
    for at in [span.start, span.end] {
        if !text.is_char_boundary(at) {
            return Err(Error::NotCharBoundary(at));
        }
    }
    Ok(())
}
