// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Turns a language server's answers about one file into [`Symbol`]s. The
//! answers are untrusted: a symbol is kept only when its range lands, inside
//! the file, on an identifier with the name it claims.

use std::ops::Range;

use gen_lsp_types::{DocumentSymbol, Position, PositionEncodingKind, SymbolKind};

use crate::{Error, MAX_FILE_BYTES, MAX_SYMBOLS_PER_FILE, Role, Source, Symbol};

/// Characters walked to turn columns into bytes, per merge. Without it a
/// server can name the end of one long line a hundred thousand times.
const MAX_SCAN_CHARS: usize = 8 * MAX_FILE_BYTES;

/// What one [`Semantic::merge_lsp`](crate::Semantic::merge_lsp) call did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MergeReport {
    /// Symbols tree-sitter did not have.
    pub added: usize,
    /// Symbols the file already had; tree-sitter's were kept.
    pub known: usize,
    /// Answers dropped: outside the file, not on an identifier, a name that
    /// does not match the text, or a kind the index does not keep.
    pub rejected: usize,
    /// Answers left unread because a cap was hit.
    pub truncated: bool,
}

/// Line starts of one file and the column arithmetic for one encoding.
struct Positions<'a> {
    text: &'a str,
    lines: Vec<usize>,
    utf8: bool,
    utf16: bool,
    scanned: usize,
}

impl<'a> Positions<'a> {
    fn new(text: &'a str, encoding: &PositionEncodingKind) -> Result<Self, Error> {
        let (utf8, utf16) = match encoding {
            PositionEncodingKind::UTF8 => (true, false),
            PositionEncodingKind::UTF16 => (false, true),
            PositionEncodingKind::UTF32 => (false, false),
            PositionEncodingKind::Custom(other) => {
                return Err(Error::UnsupportedEncoding(other.to_string()));
            }
        };
        let starts = text.match_indices('\n').map(|(i, _)| i + 1);
        let lines = std::iter::once(0).chain(starts).collect();
        Ok(Self {
            text,
            lines,
            utf8,
            utf16,
            scanned: 0,
        })
    }

    /// Byte offset of `pos`, or `None` when it is not in the file. A column
    /// past the line end is refused rather than clamped as LSP allows: the
    /// answer is about some other text.
    fn offset(&mut self, pos: Position) -> Option<usize> {
        let line = usize::try_from(pos.line).ok()?;
        let col = usize::try_from(pos.character).ok()?;
        let start = *self.lines.get(line)?;
        let end = self.lines.get(line + 1).map_or(self.text.len(), |e| e - 1);
        let text = self.text.get(start..end)?;
        if self.utf8 {
            return text.is_char_boundary(col).then_some(start + col);
        }
        let mut units = 0;
        for (i, ch) in text.char_indices() {
            if units == col {
                return Some(start + i);
            }
            self.scanned += 1;
            if units > col || self.scanned > MAX_SCAN_CHARS {
                return None;
            }
            units += if self.utf16 { ch.len_utf16() } else { 1 };
        }
        (units == col).then_some(end)
    }

    /// Byte range of an LSP range, if both ends are in the file and in order.
    fn range(&mut self, range: &gen_lsp_types::Range) -> Option<Range<usize>> {
        let start = self.offset(range.start)?;
        let end = self.offset(range.end)?;
        (start <= end).then_some(start..end)
    }

    fn identifier(&self, range: &Range<usize>) -> Option<&'a str> {
        let text = self.text.get(range.clone())?;
        let ok = !text.is_empty() && text.chars().all(|c| c.is_alphanumeric() || c == '_');
        ok.then_some(text)
    }

    fn line(&self, offset: usize) -> usize {
        self.lines.partition_point(|&s| s <= offset) - 1
    }

    fn exhausted(&self) -> bool {
        self.scanned > MAX_SCAN_CHARS
    }
}

/// The index's kind for an LSP symbol kind, or `None` for kinds the index
/// does not keep (fields, locals, literals), so both sources share one scope.
fn kind(kind: SymbolKind) -> Option<&'static str> {
    Some(match kind {
        SymbolKind::Module | SymbolKind::Namespace | SymbolKind::Package => "module",
        SymbolKind::Class | SymbolKind::Struct | SymbolKind::Enum => "class",
        SymbolKind::Interface => "interface",
        SymbolKind::Method | SymbolKind::Constructor => "method",
        SymbolKind::Function => "function",
        SymbolKind::Constant => "constant",
        SymbolKind::EnumMember => "variant",
        _ => return None,
    })
}

/// Definitions from a `textDocument/documentSymbol` answer and references
/// from `textDocument/references` locations already filtered to this file.
pub(crate) fn symbols(
    text: &str,
    encoding: &PositionEncodingKind,
    definitions: &[DocumentSymbol],
    references: &[gen_lsp_types::Range],
) -> Result<(Vec<Symbol>, MergeReport), Error> {
    let mut pos = Positions::new(text, encoding)?;
    let mut report = MergeReport::default();
    let mut out = Vec::new();
    // A stack, not recursion: the nesting depth is the server's to choose.
    let mut stack: Vec<&DocumentSymbol> = definitions.iter().rev().collect();
    while let Some(def) = stack.pop() {
        if out.len() >= MAX_SYMBOLS_PER_FILE || pos.exhausted() {
            report.truncated = true;
            break;
        }
        if let Some(children) = &def.children {
            stack.extend(children.iter().rev());
        }
        let name_range = pos.range(&def.selection_range);
        let found = name_range.filter(|r| pos.identifier(r) == Some(def.name.as_str()));
        let (Some(name_range), Some(kind)) = (found, kind(def.kind)) else {
            report.rejected += 1;
            continue;
        };
        let range = pos
            .range(&def.range)
            .filter(|r| r.start <= name_range.start && name_range.end <= r.end)
            .unwrap_or_else(|| name_range.clone());
        out.push(symbol(
            &pos,
            def.name.clone(),
            kind,
            Role::Definition,
            name_range,
            range,
        ));
    }
    for reference in references {
        if report.truncated || out.len() >= MAX_SYMBOLS_PER_FILE || pos.exhausted() {
            report.truncated = true;
            break;
        }
        let Some(range) = pos.range(reference) else {
            report.rejected += 1;
            continue;
        };
        let Some(name) = pos.identifier(&range) else {
            report.rejected += 1;
            continue;
        };
        let name = name.to_string();
        out.push(symbol(
            &pos,
            name,
            "reference",
            Role::Reference,
            range.clone(),
            range,
        ));
    }
    Ok((out, report))
}

fn symbol(
    pos: &Positions<'_>,
    name: String,
    kind: &str,
    role: Role,
    name_range: Range<usize>,
    range: Range<usize>,
) -> Symbol {
    Symbol {
        name,
        kind: kind.to_string(),
        role,
        line: pos.line(name_range.start),
        name_range,
        range,
        source: Source::LanguageServer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    #[test]
    fn columns_follow_the_encoding() {
        // "é" is 2 bytes, 1 UTF-16 unit; "𝄞" is 4 bytes, 2 UTF-16 units.
        let text = "ab\né𝄞x\n";
        let mut utf16 = Positions::new(text, &PositionEncodingKind::UTF16).unwrap();
        assert_eq!(utf16.offset(at(1, 3)), Some(9));
        assert_eq!(utf16.offset(at(1, 2)), None, "inside a surrogate pair");
        let mut utf32 = Positions::new(text, &PositionEncodingKind::UTF32).unwrap();
        assert_eq!(utf32.offset(at(1, 2)), Some(9));
        let mut utf8 = Positions::new(text, &PositionEncodingKind::UTF8).unwrap();
        assert_eq!(utf8.offset(at(1, 6)), Some(9));
        assert_eq!(utf8.offset(at(1, 1)), None, "inside a UTF-8 character");
    }

    #[test]
    fn positions_outside_the_file_are_refused() {
        let mut pos = Positions::new("ab\ncd", &PositionEncodingKind::UTF16).unwrap();
        assert_eq!(pos.offset(at(1, 2)), Some(5));
        assert_eq!(pos.offset(at(0, 3)), None, "past the line end");
        assert_eq!(pos.offset(at(2, 0)), None, "past the last line");
        assert_eq!(pos.offset(at(u32::MAX, u32::MAX)), None);
    }

    #[test]
    fn custom_encodings_are_refused() {
        let custom = PositionEncodingKind::new("utf-7");
        assert!(matches!(
            Positions::new("", &custom),
            Err(Error::UnsupportedEncoding(_))
        ));
    }
}
