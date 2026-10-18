// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Incremental parse and per-file re-index, driven by real `kerf-core` edits.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use kerf_core::{EditEvent, TextBuffer};
use kerf_semantic::{Error, MAX_FILE_BYTES, MAX_FILES, MAX_SYMBOLS_PER_FILE, Semantic};

const A: &str = "a.rs";
const B: &str = "b.rs";

fn names(found: &[(&Path, &kerf_semantic::Symbol)]) -> Vec<(String, usize)> {
    found
        .iter()
        .map(|(p, s)| (p.display().to_string(), s.line))
        .collect()
}

/// Opens `text` as `path` in both `semantic` and a buffer that edits it.
fn open(semantic: &mut Semantic, path: &str, text: &str) -> TextBuffer {
    semantic.open(path, text.to_string()).unwrap();
    TextBuffer::with_text(text)
}

fn apply(semantic: &mut Semantic, path: &str, buf: &mut TextBuffer, event: &EditEvent) {
    semantic
        .edit(Path::new(path), event, buf.to_text())
        .unwrap();
}

fn find(buf: &TextBuffer, needle: &str) -> usize {
    buf.to_text().find(needle).expect(needle)
}

#[test]
fn insert_edit_and_delete_keep_the_index_right() {
    let mut s = Semantic::new().unwrap();
    let mut buf = open(&mut s, A, "fn alpha() {}\n");
    assert_eq!(names(&s.definitions("alpha")), [(A.into(), 0)]);

    // Insert a caller.
    let end = buf.len_bytes();
    let ev = buf
        .apply(end..end, "fn beta() {\n    alpha();\n}\n")
        .unwrap();
    apply(&mut s, A, &mut buf, &ev);
    assert_eq!(names(&s.definitions("beta")), [(A.into(), 1)]);
    assert_eq!(names(&s.references("alpha")), [(A.into(), 2)]);

    // Rename the definition; the old name's definition goes, the call stays.
    let at = find(&buf, "alpha");
    let ev = buf.apply(at..at + 5, "gamma").unwrap();
    apply(&mut s, A, &mut buf, &ev);
    assert!(s.definitions("alpha").is_empty());
    assert_eq!(names(&s.definitions("gamma")), [(A.into(), 0)]);
    assert_eq!(names(&s.references("alpha")), [(A.into(), 2)]);

    // Delete the caller.
    let at = find(&buf, "fn beta");
    let ev = buf.apply(at..buf.len_bytes(), "").unwrap();
    apply(&mut s, A, &mut buf, &ev);
    assert!(s.definitions("beta").is_empty());
    assert!(s.references("alpha").is_empty());
}

#[test]
fn incremental_parse_matches_a_fresh_parse() {
    let mut s = Semantic::new().unwrap();
    let mut buf = open(
        &mut s,
        A,
        "struct P { x: i32 }\nfn f(p: P) -> i32 { p.x }\n",
    );
    let at = find(&buf, "p.x");
    let ev = buf.apply(at..at + 3, "g(p.x) + 1").unwrap();
    apply(&mut s, A, &mut buf, &ev);
    let ev = buf.apply(0..0, "/// Doc.\n").unwrap();
    apply(&mut s, A, &mut buf, &ev);
    let end = buf.len_bytes();
    let ev = buf
        .apply(end..end, "fn g(v: i32) -> i32 {\n    v\n}\n")
        .unwrap();
    apply(&mut s, A, &mut buf, &ev);
    let at = find(&buf, "struct");
    let ev = buf.apply(at..at + 6, "enum").unwrap();
    apply(&mut s, A, &mut buf, &ev);
    for _ in 0..2 {
        let ev = buf.undo().unwrap();
        apply(&mut s, A, &mut buf, &ev);
    }
    let ev = buf.redo().unwrap();
    apply(&mut s, A, &mut buf, &ev);

    let mut fresh = Semantic::new().unwrap();
    fresh.open(A, buf.to_text()).unwrap();
    let path = Path::new(A);
    assert_eq!(s.sexp(path), fresh.sexp(path));
    let symbols = |sem: &Semantic| sem.symbols(path).unwrap().symbols.clone();
    assert_eq!(symbols(&s), symbols(&fresh));
}

#[test]
fn changed_ranges_stay_inside_the_edited_function() {
    let text: String = (0..200)
        .map(|i| format!("fn f{i}() -> i32 {{\n    {i}\n}}\n"))
        .collect();
    let mut s = Semantic::new().unwrap();
    let mut buf = open(&mut s, A, &text);
    let body = find(&buf, "fn f100(");
    let at = body + text[body..].find("    100").unwrap();
    let ev = buf.apply(at..at, "    let x = 1;\n").unwrap();
    let changed = s.edit(Path::new(A), &ev, buf.to_text()).unwrap();
    let f101 = find(&buf, "fn f101(");
    assert!(!changed.is_empty());
    for range in changed {
        assert!(
            range.start >= body && range.end <= f101,
            "{range:?} leaves f100"
        );
    }
}

#[test]
fn an_edit_reindexes_only_its_own_file() {
    let mut s = Semantic::new().unwrap();
    let mut a = open(&mut s, A, "fn one() {}\n");
    open(&mut s, B, "fn two() { one(); }\n");
    let (a_rev, b_before) = (
        s.symbols(Path::new(A)).unwrap().revision,
        s.symbols(Path::new(B)).unwrap().clone(),
    );
    let b_tree = s.sexp(Path::new(B));

    let ev = a.apply(0..0, "fn zero() {}\n").unwrap();
    apply(&mut s, A, &mut a, &ev);

    assert!(s.symbols(Path::new(A)).unwrap().revision > a_rev);
    assert_eq!(s.symbols(Path::new(B)).unwrap(), &b_before);
    assert_eq!(s.sexp(Path::new(B)), b_tree);
    assert_eq!(names(&s.references("one")), [(B.into(), 0)]);
}

#[test]
fn a_bad_edit_changes_nothing() {
    let mut s = Semantic::new().unwrap();
    let mut buf = open(&mut s, A, "fn one() {}\n");
    let before = s.symbols(Path::new(A)).unwrap().clone();
    let ev = buf.apply(0..0, "fn zero() {}\n").unwrap();
    // The text does not follow from the event: stale by one byte.
    let err = s.edit(Path::new(A), &ev, format!("{}x", buf.to_text()));
    assert!(matches!(err, Err(Error::BadEdit { .. })), "{err:?}");
    assert_eq!(s.symbols(Path::new(A)).unwrap(), &before);
    let err = s.edit(Path::new(B), &ev, buf.to_text());
    assert!(matches!(err, Err(Error::NotOpen { .. })), "{err:?}");
}

#[test]
fn caps_and_unsupported_files_are_refused() {
    let mut s = Semantic::new().unwrap();
    let err = s.open("notes.md", String::new());
    assert!(
        matches!(err, Err(Error::UnsupportedLanguage { .. })),
        "{err:?}"
    );
    let err = s.open(A, " ".repeat(MAX_FILE_BYTES + 1));
    assert!(matches!(err, Err(Error::FileTooLarge { .. })), "{err:?}");
    assert!(s.symbols(Path::new(A)).is_none());

    for i in 0..MAX_FILES {
        s.open(format!("{i}.rs"), String::new()).unwrap();
    }
    let err = s.open("one-more.rs", String::new());
    assert!(matches!(err, Err(Error::TooManyFiles { .. })), "{err:?}");
    // Re-opening a file that is already open does not need a new slot.
    s.open("0.rs", "fn zero() {}".into()).unwrap();
}

#[test]
fn symbols_past_the_cap_are_dropped_and_flagged() {
    let calls = "a();".repeat(MAX_SYMBOLS_PER_FILE + 1);
    let mut s = Semantic::new().unwrap();
    s.open(A, format!("fn f() {{ {calls} }}")).unwrap();
    let file = s.symbols(Path::new(A)).unwrap();
    assert!(file.truncated);
    assert_eq!(file.symbols.len(), MAX_SYMBOLS_PER_FILE);
}

#[test]
fn closing_a_file_drops_it_from_lookups() {
    let mut s = Semantic::new().unwrap();
    open(&mut s, A, "fn one() {}\n");
    open(&mut s, B, "fn one() {}\n");
    assert!(s.close(Path::new(A)));
    assert!(!s.close(Path::new(A)));
    assert_eq!(names(&s.definitions("one")), [(B.into(), 0)]);
}
