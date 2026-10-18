// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Language-server answers merged into the index: untrusted input, stale
//! answers and the edit that retires them.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use gen_lsp_types::{DocumentSymbol, Position, PositionEncodingKind, Range, SymbolKind};
use kerf_core::TextBuffer;
use kerf_semantic::{Error, MAX_SYMBOLS_PER_FILE, Semantic, Source};

const TEXT: &str = "fn a() {}\nfn b() { let f = a; }\n";

fn range(line: u32, start: u32, end: u32) -> Range {
    Range {
        start: Position::new(line, start),
        end: Position::new(line, end),
    }
}

#[allow(deprecated)]
fn function(name: &str, selection: Range) -> DocumentSymbol {
    DocumentSymbol::new(
        name.into(),
        None,
        SymbolKind::Function,
        None,
        None,
        selection,
        selection,
        None,
    )
}

fn opened() -> (Semantic, &'static Path) {
    let path = Path::new("a.rs");
    let mut semantic = Semantic::new().unwrap();
    semantic.open(path, TEXT.to_string()).unwrap();
    (semantic, path)
}

fn merge(
    s: &mut Semantic,
    path: &Path,
    defs: &[DocumentSymbol],
    refs: &[Range],
) -> Result<kerf_semantic::MergeReport, Error> {
    let version = s.version(path).unwrap();
    s.merge_lsp(path, version, &PositionEncodingKind::UTF16, defs, refs)
}

#[test]
fn answers_that_do_not_fit_the_text_are_rejected() {
    let (mut s, path) = opened();
    let defs = [
        function("zzz", range(0, 3, 4)),  // name is not the text
        function("a", range(9, 0, 1)),    // no such line
        function("a", range(0, 3, 40)),   // past the line end
        function("fn a", range(0, 0, 4)), // not an identifier
    ];
    let refs = [
        range(1, 20, 18),
        range(0, 5, 6),
        range(1, u32::MAX, u32::MAX),
    ];
    let report = merge(&mut s, path, &defs, &refs).unwrap();
    assert_eq!((report.added, report.rejected), (0, 7), "{report:?}");
    assert!(s.references("a").is_empty());
}

#[test]
fn a_value_reference_is_added_and_an_edit_retires_it() {
    let (mut s, path) = opened();
    // `a` in `let f = a;` is a value, which tree-sitter does not tag.
    let report = merge(
        &mut s,
        path,
        &[function("a", range(0, 3, 4))],
        &[range(1, 17, 18)],
    )
    .unwrap();
    assert_eq!((report.added, report.known), (1, 1), "{report:?}");
    let refs = s.references("a");
    assert_eq!(refs.len(), 1);
    assert_eq!(
        (refs[0].1.line, refs[0].1.source),
        (1, Source::LanguageServer)
    );

    let old = s.version(path).unwrap();
    let mut buf = TextBuffer::with_text(TEXT);
    let event = buf.apply(0..0, "\n").unwrap();
    s.edit(path, &event, buf.to_text()).unwrap();
    assert!(
        s.references("a").is_empty(),
        "positions of the old text are gone"
    );
    assert!(matches!(
        merge_at(&mut s, path, old),
        Err(Error::Stale { answered, .. }) if answered == old
    ));

    // Reopening never reuses a version, so the old answer stays stale.
    s.open(path, TEXT.to_string()).unwrap();
    assert!(matches!(
        merge_at(&mut s, path, old),
        Err(Error::Stale { .. })
    ));
    s.close(path);
    assert!(matches!(
        merge_at(&mut s, path, old),
        Err(Error::NotOpen { .. })
    ));
}

fn merge_at(
    s: &mut Semantic,
    path: &Path,
    version: u64,
) -> Result<kerf_semantic::MergeReport, Error> {
    s.merge_lsp(
        path,
        version,
        &PositionEncodingKind::UTF16,
        &[],
        &[range(1, 17, 18)],
    )
}

#[test]
fn answers_past_the_cap_are_left_unread() {
    let (mut s, path) = opened();
    let refs = vec![range(1, 17, 18); MAX_SYMBOLS_PER_FILE + 1];
    let report = merge(&mut s, path, &[], &refs).unwrap();
    assert!(report.truncated);
    assert_eq!(report.added, 1, "duplicates of one range add one symbol");
}
