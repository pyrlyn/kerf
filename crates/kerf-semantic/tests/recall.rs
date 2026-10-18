// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Definition and reference recall on a hand-labelled Rust fixture.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::Path;

use kerf_semantic::{Role, Semantic};

/// The measured numbers; the upstream query alone reaches 0.750 and 0.519,
/// so any drop means an extra pattern stopped matching.
const MIN_DEFINITION_RECALL: f64 = 1.0;
const MIN_REFERENCE_RECALL: f64 = 1.0;

type Label = (Role, String, usize);

fn labels() -> BTreeSet<Label> {
    include_str!("fixtures/recall.labels")
        .lines()
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

/// Share of `expected` that `found` has; with it, the misses.
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

#[test]
fn recall_on_labelled_fixture() {
    let path = Path::new("recall.rs");
    let mut semantic = Semantic::new().unwrap();
    semantic
        .open(path, include_str!("fixtures/recall.rs").to_string())
        .unwrap();
    let found: BTreeSet<Label> = semantic
        .symbols(path)
        .unwrap()
        .symbols
        .iter()
        .map(|s| (s.role, s.name.clone(), s.line + 1))
        .collect();
    let expected = labels();
    let (defs, missed_defs) = recall(&expected, &found, Role::Definition);
    let (refs, missed_refs) = recall(&expected, &found, Role::Reference);
    let extra: Vec<&Label> = found.difference(&expected).collect();
    println!("definition recall {defs:.3}, missed {missed_defs:?}");
    println!("reference recall {refs:.3}, missed {missed_refs:?}");
    println!("unlabelled hits {extra:?}");
    assert!(defs >= MIN_DEFINITION_RECALL, "definition recall {defs:.3}");
    assert!(refs >= MIN_REFERENCE_RECALL, "reference recall {refs:.3}");
    // A guess that matches what nobody labelled would be noise in "find references".
    assert!(extra.is_empty(), "unlabelled hits {extra:?}");
}
