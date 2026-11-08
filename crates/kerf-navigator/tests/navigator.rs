// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The navigator and Open Quickly driven over a temporary project, as a host would.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use kerf_navigator::{HitKind, MAX_CHILDREN, Navigator, NodeId, NodeKind, OpenQuickly, Row};
use kerf_semantic::Semantic;

const BOUNDED_RS: &str = "pub struct BoundedQueue;\npub fn bounded_push() {}\n";

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in [
        (".gitignore", "target/\n*.log\n"),
        (".git/HEAD", "ref: refs/heads/main\n"),
        (".github/ci.yml", ""),
        ("README.md", ""),
        ("debug.log", ""),
        ("docs/guide.md", ""),
        ("src/main.rs", "fn main() { bounded_push(); }\n"),
        ("src/bounded.rs", BOUNDED_RS),
        ("src/util/mod.rs", ""),
        ("target/debug/kerf", ""),
    ] {
        let path = dir.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    dir
}

fn view(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .map(|r| {
            let mark = match (r.kind, r.expanded) {
                (NodeKind::File, _) => " ",
                (NodeKind::Folder, false) => "+",
                (NodeKind::Folder, true) => "-",
            };
            let sel = if r.selected { "*" } else { "" };
            format!("{}{mark}{}{sel}", "  ".repeat(r.depth), r.name)
        })
        .collect()
}

fn id(nav: &Navigator, name: &str) -> NodeId {
    nav.rows().into_iter().find(|r| r.name == name).unwrap().id
}

#[test]
fn tree_lists_lazily_honours_ignore_files_and_tracks_selection() {
    let dir = project();
    let mut nav = Navigator::open(dir.path()).unwrap();
    assert_eq!(
        view(&nav.rows()),
        ["+.github", "+docs", "+src", " .gitignore", " README.md"]
    );

    let src = id(&nav, "src");
    assert!(nav.expand(src).unwrap());
    assert!(!nav.expand(src).unwrap());
    let main = id(&nav, "main.rs");
    assert!(nav.select(main));
    assert!(!nav.select(main));
    assert_eq!(
        nav.relative_path(main).unwrap(),
        Path::new("src").join("main.rs")
    );
    assert_eq!(
        view(&nav.rows()),
        [
            "+.github",
            "+docs",
            "-src",
            "  +util",
            "   bounded.rs",
            "   main.rs*",
            " .gitignore",
            " README.md"
        ]
    );

    // Collapsing the folder holding the selection selects the folder.
    assert!(nav.collapse(src));
    assert!(!nav.collapse(src));
    assert_eq!(nav.selected(), Some(src));
    assert!(!nav.expand(id(&nav, "README.md")).unwrap());
    assert!(Navigator::open(dir.path().join("README.md")).is_err());
}

#[test]
fn filter_searches_the_whole_project_and_clears_back() {
    let dir = project();
    let mut nav = Navigator::open(dir.path()).unwrap();
    nav.set_filter(" MOD ");
    assert_eq!(view(&nav.rows()), ["-src", "  -util", "     mod.rs"]);
    nav.set_filter("bound");
    assert_eq!(view(&nav.rows()), ["-src", "   bounded.rs"]);
    nav.set_filter("kerf");
    assert!(nav.rows().is_empty(), "target/ is ignored");
    nav.set_filter("");
    assert_eq!(view(&nav.rows())[2], "+src");
}

#[test]
fn a_huge_folder_is_capped() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..=MAX_CHILDREN {
        std::fs::write(dir.path().join(format!("f{i}")), "").unwrap();
    }
    let rows = Navigator::open(dir.path()).unwrap().rows();
    assert_eq!(rows.len(), MAX_CHILDREN);
}

#[test]
fn open_quickly_ranks_symbols_and_files() {
    let dir = project();
    let mut nav = Navigator::open(dir.path()).unwrap();
    let mut semantic = Semantic::new().unwrap();
    semantic
        .open(dir.path().join("src/bounded.rs"), BOUNDED_RS.into())
        .unwrap();
    let mut quick = OpenQuickly::default();

    let hits = quick.query("Bounded", nav.files(), Some(&semantic));
    let found: Vec<_> = hits.iter().map(|h| (h.kind, h.label.as_str())).collect();
    let file = Path::new("src").join("bounded.rs");
    let file = file.to_str().unwrap();
    assert_eq!(
        found,
        [
            (HitKind::Symbol, "BoundedQueue"),
            (HitKind::Symbol, "bounded_push"),
            (HitKind::File, file),
        ],
        "the call in main.rs is a reference, not a hit"
    );
    assert_eq!(hits[0].ranges, vec![0..7]);
    assert_eq!((hits[0].line, hits[1].line), (0, 1));
    assert_eq!(hits[2].ranges, vec![4..11]);
    assert!(hits[0].score > hits[2].score);

    assert_eq!(quick.selected(), Some(0));
    assert!(quick.move_selection(5));
    assert_eq!(quick.selected(), Some(2));
    assert!(!quick.move_selection(1));
    assert!(quick.move_selection(-9));

    assert!(quick.query("  ", nav.files(), None).is_empty());
    assert_eq!(quick.selected(), None);
    let hits = quick.query("guide", nav.files(), None);
    assert_eq!(hits.len(), 1);
    assert!(quick.query("kerf", nav.files(), None).is_empty());
}
