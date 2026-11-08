// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Recall over hand-labelled fixtures, shared with `kerf-lsp`'s end-to-end
//! test so both measure the same way.

use std::collections::BTreeSet;
use std::path::Path;

use kerf_semantic::{Role, Semantic};

pub type Label = (Role, String, usize);

pub fn labels(text: &str) -> BTreeSet<Label> {
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

pub fn found(semantic: &Semantic, path: &Path) -> BTreeSet<Label> {
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
pub fn check(what: &str, expected: &BTreeSet<Label>, found: &BTreeSet<Label>, min: (f64, f64)) {
    let (defs, missed_defs) = recall(expected, found, Role::Definition);
    let (refs, missed_refs) = recall(expected, found, Role::Reference);
    let extra: Vec<&Label> = found.difference(expected).collect();
    println!("{what}: definition recall {defs:.3}, missed {missed_defs:?}");
    println!("{what}: reference recall {refs:.3}, missed {missed_refs:?}");
    assert!(defs >= min.0, "{what}: definition recall {defs:.3}");
    assert!(refs >= min.1, "{what}: reference recall {refs:.3}");
    assert!(extra.is_empty(), "{what}: unlabelled hits {extra:?}");
}
