// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The model against throwaway repositories: status groups, diffs, stage,
//! unstage, commit and the caps.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;

use git2::{Repository, RepositoryInitOptions};
use kerf_scm::{
    Change, Error, FileEntry, Group, LineKind, MAX_DIFF_LINES, MAX_FILES, MAX_LINE_BYTES, Scm,
};
use tempfile::TempDir;

/// An empty repository on `main` with an identity, so commits work whatever
/// the machine's global config says.
fn repo() -> (TempDir, Scm) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repository::init_opts(
        dir.path(),
        RepositoryInitOptions::new().initial_head("main"),
    )
    .unwrap();
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Kerf Test").unwrap();
    config.set_str("user.email", "test@kerf.invalid").unwrap();
    let scm = Scm::open(dir.path()).unwrap();
    (dir, scm)
}

fn write(dir: &Path, path: &str, text: &str) {
    let path = dir.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn row(path: &str, change: Change) -> FileEntry {
    FileEntry {
        path: path.to_owned(),
        change,
    }
}

/// A repository whose first commit holds `a.txt` and `b.txt`.
fn committed() -> (TempDir, Scm) {
    let (dir, scm) = repo();
    write(dir.path(), "a.txt", "one\ntwo\nthree\n");
    write(dir.path(), "b.txt", "bee\n");
    scm.stage("a.txt").unwrap();
    scm.stage("b.txt").unwrap();
    scm.commit("first").unwrap();
    (dir, scm)
}

#[test]
fn status_splits_staged_changes_and_untracked() {
    let (dir, scm) = committed();
    write(dir.path(), "a.txt", "one\n2\nthree\n");
    fs::remove_file(dir.path().join("b.txt")).unwrap();
    write(dir.path(), "new/c.txt", "see\n");
    write(dir.path(), "d.txt", "dee\n");
    scm.stage("d.txt").unwrap();
    write(dir.path(), "d.txt", "dee, edited\n");

    let status = scm.status().unwrap();
    assert_eq!(status.branch, "main");
    assert!(!status.detached && !status.truncated);
    assert_eq!(status.staged, [row("d.txt", Change::Added)]);
    assert_eq!(
        status.changes,
        [
            row("a.txt", Change::Modified),
            row("b.txt", Change::Deleted),
            row("d.txt", Change::Modified),
            row("new/c.txt", Change::Untracked),
        ]
    );
    let letters: String = status.changes.iter().map(|r| r.change.letter()).collect();
    assert_eq!(letters, "MDM?");
}

#[test]
fn stage_and_unstage_move_rows_between_groups() {
    let (dir, scm) = committed();
    write(dir.path(), "a.txt", "changed\n");
    fs::remove_file(dir.path().join("b.txt")).unwrap();

    scm.stage("a.txt").unwrap();
    scm.stage("b.txt").unwrap();
    let status = scm.status().unwrap();
    assert_eq!(
        status.staged,
        [
            row("a.txt", Change::Modified),
            row("b.txt", Change::Deleted)
        ]
    );
    assert!(status.changes.is_empty());

    scm.unstage("a.txt").unwrap();
    scm.unstage("b.txt").unwrap();
    let status = scm.status().unwrap();
    assert!(status.staged.is_empty());
    assert_eq!(
        status.changes,
        [
            row("a.txt", Change::Modified),
            row("b.txt", Change::Deleted)
        ]
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "changed\n"
    );
}

#[test]
fn unstage_before_the_first_commit_leaves_the_file_untracked() {
    let (dir, scm) = repo();
    write(dir.path(), "a.txt", "a\n");
    scm.stage("a.txt").unwrap();
    assert_eq!(scm.status().unwrap().staged, [row("a.txt", Change::Added)]);
    scm.unstage("a.txt").unwrap();
    let status = scm.status().unwrap();
    assert!(status.staged.is_empty());
    assert_eq!(status.changes, [row("a.txt", Change::Untracked)]);
}

#[test]
fn diffs_carry_hunk_headers_and_line_numbers() {
    let (dir, scm) = committed();
    write(dir.path(), "a.txt", "one\n2\nthree\n");
    let diff = scm.diff(Group::Changes, "a.txt").unwrap();
    assert!(!diff.binary && !diff.truncated);
    assert_eq!(diff.hunks.len(), 1);
    let hunk = &diff.hunks[0];
    assert_eq!(hunk.header, "@@ -1,3 +1,3 @@");
    let lines: Vec<_> = hunk
        .lines
        .iter()
        .map(|l| (l.kind, l.old_line, l.new_line, l.text.as_str()))
        .collect();
    assert_eq!(
        lines,
        [
            (LineKind::Context, Some(1), Some(1), "one"),
            (LineKind::Removed, Some(2), None, "two"),
            (LineKind::Added, None, Some(2), "2"),
            (LineKind::Context, Some(3), Some(3), "three"),
        ]
    );
    assert!(scm.diff(Group::Staged, "a.txt").unwrap().hunks.is_empty());

    scm.stage("a.txt").unwrap();
    assert_eq!(scm.diff(Group::Staged, "a.txt").unwrap().hunks, diff.hunks);
    assert!(scm.diff(Group::Changes, "a.txt").unwrap().hunks.is_empty());
}

#[test]
fn untracked_files_diff_as_added_and_binary_files_have_no_lines() {
    let (dir, scm) = committed();
    write(dir.path(), "new.txt", "x\ny\n");
    let diff = scm.diff(Group::Changes, "new.txt").unwrap();
    let kinds: Vec<_> = diff.hunks[0].lines.iter().map(|l| l.kind).collect();
    assert_eq!(kinds, [LineKind::Added, LineKind::Added]);

    fs::write(dir.path().join("blob.bin"), [0u8, 1, 2, 0, 255]).unwrap();
    let diff = scm.diff(Group::Changes, "blob.bin").unwrap();
    assert!(diff.binary);
    assert!(diff.hunks.is_empty());
}

#[test]
fn diff_paths_are_literal_not_globs() {
    let (dir, scm) = committed();
    write(dir.path(), "a.txt", "changed\n");
    assert!(scm.diff(Group::Changes, "*.txt").unwrap().hunks.is_empty());
}

#[test]
fn commit_takes_the_index_and_refuses_empty_ones() {
    let (dir, scm) = committed();
    assert!(matches!(scm.commit("again"), Err(Error::NothingToCommit)));

    write(dir.path(), "a.txt", "changed\n");
    write(dir.path(), "b.txt", "not staged\n");
    scm.stage("a.txt").unwrap();
    assert!(matches!(scm.commit("  \n\n "), Err(Error::EmptyMessage)));
    let big = "x".repeat(kerf_scm::MAX_MESSAGE_BYTES + 1);
    assert!(matches!(
        scm.commit(&big),
        Err(Error::MessageTooLarge { .. })
    ));

    let id = scm.commit("Change a\n\n\nBody  \n").unwrap();
    let repo = Repository::open(dir.path()).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.id().to_string(), id);
    assert_eq!(head.message().unwrap(), "Change a\n\nBody\n");
    assert_eq!(head.parent_count(), 1);

    let status = scm.status().unwrap();
    assert!(status.staged.is_empty());
    assert_eq!(status.changes, [row("b.txt", Change::Modified)]);
}

#[test]
fn a_new_repository_reports_its_branch_and_takes_a_first_commit() {
    let (dir, scm) = repo();
    let status = scm.status().unwrap();
    assert_eq!(status.branch, "main");
    assert!(matches!(scm.commit("empty"), Err(Error::NothingToCommit)));
    write(dir.path(), "a.txt", "a\n");
    scm.stage("a.txt").unwrap();
    scm.commit("root").unwrap();
    let repo = Repository::open(dir.path()).unwrap();
    assert_eq!(
        repo.head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .parent_count(),
        0
    );
}

#[test]
fn a_detached_head_shows_the_short_commit_id() {
    let (dir, scm) = committed();
    let repo = Repository::open(dir.path()).unwrap();
    let id = repo.head().unwrap().target().unwrap();
    repo.set_head_detached(id).unwrap();
    let status = scm.status().unwrap();
    assert!(status.detached);
    assert_eq!(status.branch, id.to_string()[..7]);
}

#[test]
fn opening_outside_a_repository_and_bad_paths_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(Scm::open(dir.path()), Err(Error::NotARepository)));

    let (dir, scm) = committed();
    write(dir.path(), "sub/x.txt", "x\n");
    let from_sub = Scm::open(&dir.path().join("sub")).unwrap();
    assert_eq!(
        from_sub.workdir().canonicalize().unwrap(),
        dir.path().canonicalize().unwrap()
    );
    for bad in ["../outside", "/etc/passwd", ""] {
        assert!(matches!(scm.stage(bad), Err(Error::BadPath)), "{bad}");
        assert!(matches!(scm.unstage(bad), Err(Error::BadPath)), "{bad}");
        assert!(
            matches!(scm.diff(Group::Changes, bad), Err(Error::BadPath)),
            "{bad}"
        );
    }
    // libgit2 refuses to stage into the repository's own directory.
    assert!(scm.stage(".git/config").is_err());
}

#[test]
fn status_stops_at_the_file_cap() {
    let (dir, scm) = repo();
    for i in 0..=MAX_FILES {
        fs::write(dir.path().join(format!("f{i}")), "").unwrap();
    }
    let status = scm.status().unwrap();
    assert_eq!(status.changes.len(), MAX_FILES);
    assert!(status.truncated);
}

#[test]
fn diffs_stop_at_the_line_cap_and_cut_long_lines() {
    let (dir, scm) = committed();
    let mut text = "y".repeat(MAX_LINE_BYTES * 2);
    text.push('\n');
    text.push_str(&"x\n".repeat(MAX_DIFF_LINES + 10));
    write(dir.path(), "big.txt", &text);
    let diff = scm.diff(Group::Changes, "big.txt").unwrap();
    assert!(diff.truncated);
    let lines: usize = diff.hunks.iter().map(|h| h.lines.len()).sum();
    assert_eq!(lines, MAX_DIFF_LINES);
    assert_eq!(diff.hunks[0].lines[0].text.len(), MAX_LINE_BYTES);
}
