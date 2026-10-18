// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Definition and reference recall on hand-labelled fixtures, for tree-sitter
//! alone and with a scripted language server's answers merged in.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::Path;

use gen_lsp_types::{DocumentSymbol, Location, PositionEncodingKind};
use kerf_semantic::{Role, Semantic, Source};
use serde_json::Value;

/// tree-sitter alone on `recall.rs`: 22 of 23 definitions and 30 of 31
/// references. The misses are the item `getter!` defines and `square` named
/// as a value; the upstream query alone reached 0.750 and 0.519 on the T3
/// part of the fixture.
const RUST_TREE_SITTER: (f64, f64) = (0.956, 0.967);
/// tree-sitter plus the scripted answers in `recall.lsp.json`.
const RUST_MERGED: (f64, f64) = (1.0, 1.0);
/// The upstream Python tags query alone on `recall.py`.
const PYTHON_TREE_SITTER: (f64, f64) = (1.0, 1.0);

type Label = (Role, String, usize);

fn labels(text: &str) -> BTreeSet<Label> {
    text.lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut parts = l.split_whitespace();
            let role = match parts.next() {
                Some("def") => Role::Definition,
                Some("ref") => Role::Reference,
                other => panic!("bad label role {other:?} in {l:?}"),
            };
            let name = parts.next().expect("label name").to_string();
            let line = parts.next().expect("label line").parse().expect("line");
            (role, name, line)
        })
        .collect()
}

fn found(semantic: &Semantic, path: &Path) -> BTreeSet<Label> {
    let symbols = &semantic.symbols(path).unwrap().symbols;
    symbols
        .iter()
        .map(|s| (s.role, s.name.clone(), s.line + 1))
        .collect()
}

/// Share of `expected` with `role` that `found` has; with it, the misses.
fn recall(expected: &BTreeSet<Label>, found: &BTreeSet<Label>, role: Role) -> (f64, Vec<Label>) {
    let want: Vec<&Label> = expected.iter().filter(|l| l.0 == role).collect();
    let missed: Vec<Label> = want
        .iter()
        .filter(|l| !found.contains(**l))
        .map(|l| (*l).clone())
        .collect();
    #[allow(clippy::cast_precision_loss)]
    let share = (want.len() - missed.len()) as f64 / want.len() as f64;
    (share, missed)
}

/// Prints and checks recall against `min`, and that nothing unlabelled was found:
/// a guess nobody labelled would be noise in "find references".
fn check(what: &str, expected: &BTreeSet<Label>, found: &BTreeSet<Label>, min: (f64, f64)) {
    let (defs, missed_defs) = recall(expected, found, Role::Definition);
    let (refs, missed_refs) = recall(expected, found, Role::Reference);
    let extra: Vec<&Label> = found.difference(expected).collect();
    println!("{what}: definition recall {defs:.3}, missed {missed_defs:?}");
    println!("{what}: reference recall {refs:.3}, missed {missed_refs:?}");
    assert!(defs >= min.0, "{what}: definition recall {defs:.3}");
    assert!(refs >= min.1, "{what}: reference recall {refs:.3}");
    assert!(extra.is_empty(), "{what}: unlabelled hits {extra:?}");
}

/// The scripted server's answers: every document symbol, and the ranges in
/// `uri` of every scripted `textDocument/references` answer.
fn scripted(uri: &str) -> (Vec<DocumentSymbol>, Vec<gen_lsp_types::Range>) {
    let script: Value = serde_json::from_str(include_str!("fixtures/recall.lsp.json")).unwrap();
    let symbols = serde_json::from_value(script["textDocument/documentSymbol"].clone()).unwrap();
    let mut ranges = Vec::new();
    for answer in script["textDocument/references"].as_array().unwrap() {
        let locations: Vec<Location> = serde_json::from_value(answer["locations"].clone()).unwrap();
        ranges.extend(
            locations
                .into_iter()
                .filter(|l| l.uri.0 == uri)
                .map(|l| l.range),
        );
    }
    (symbols, ranges)
}

#[test]
fn rust_recall_improves_with_a_language_server() {
    let path = Path::new("recall.rs");
    let expected = labels(include_str!("fixtures/recall.labels"));
    let mut semantic = Semantic::new().unwrap();
    semantic
        .open(path, include_str!("fixtures/recall.rs").to_string())
        .unwrap();
    let base = found(&semantic, path);
    check("rust, tree-sitter", &expected, &base, RUST_TREE_SITTER);

    let (symbols, references) = scripted("file:///recall.rs");
    let version = semantic.version(path).unwrap();
    let encoding = PositionEncodingKind::UTF16;
    let report = semantic
        .merge_lsp(path, version, &encoding, &symbols, &references)
        .unwrap();
    println!("merge {report:?}");
    check(
        "rust, merged",
        &expected,
        &found(&semantic, path),
        RUST_MERGED,
    );
    assert_eq!(report.added, 2, "only what tree-sitter missed is added");

    // tree-sitter stays the base: everything it found keeps its tag.
    for s in &semantic.symbols(path).unwrap().symbols {
        let label = (s.role, s.name.clone(), s.line + 1);
        let want = if base.contains(&label) {
            Source::TreeSitter
        } else {
            Source::LanguageServer
        };
        assert_eq!(s.source, want, "{label:?}");
    }
}

#[test]
fn python_recall_on_labelled_fixture() {
    let path = Path::new("recall.py");
    let mut semantic = Semantic::new().unwrap();
    semantic
        .open(path, include_str!("fixtures/recall.py").to_string())
        .unwrap();
    let expected = labels(include_str!("fixtures/recall_py.labels"));
    let found = found(&semantic, path);
    check("python, tree-sitter", &expected, &found, PYTHON_TREE_SITTER);
}
