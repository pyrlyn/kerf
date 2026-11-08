// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The Source control screen through the C functions only: open a
//! repository, read the groups, load a diff, stage, unstage and commit; and
//! the refusals and inputs a broken host can send.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Every call here plays the host within the header's contract; a SAFETY
// note per call would only repeat the header.
#![allow(clippy::undocumented_unsafe_blocks)]

use std::path::Path;
use std::ptr::{null, null_mut};

use kerf_ffi::KerfStatus;
use kerf_ffi::scm::*;

fn open(path: &Path) -> (KerfStatus, *mut KerfScm, KerfScmError) {
    let path = path.to_str().unwrap();
    let (mut scm, mut error) = (null_mut(), KerfScmError::None);
    let status = unsafe { kerf_scm_open(path.as_ptr(), path.len(), &mut scm, &mut error) };
    (status, scm, error)
}

fn refresh(scm: *mut KerfScm) -> KerfScmSummary {
    let mut summary = KerfScmSummary::default();
    assert_eq!(
        unsafe { kerf_scm_refresh(scm, &mut summary) },
        KerfStatus::Ok
    );
    summary
}

/// Read a string the way a host does: ask the length, then copy.
fn string(mut copy: impl FnMut(*mut u8, usize, *mut usize) -> KerfStatus) -> String {
    let mut len = 0;
    let first = copy(null_mut(), 0, &mut len);
    assert!(matches!(first, KerfStatus::Ok | KerfStatus::TooLarge));
    let mut buf = vec![0u8; len];
    assert_eq!(copy(buf.as_mut_ptr(), buf.len(), &mut len), KerfStatus::Ok);
    String::from_utf8(buf).unwrap()
}

fn files(scm: *mut KerfScm, group: u32, count: usize) -> Vec<(KerfScmChange, String)> {
    (0..count)
        .map(|i| {
            let mut change = KerfScmChange::Added;
            let path =
                string(|b, c, l| unsafe { kerf_scm_file(scm, group, i, &mut change, b, c, l) });
            (change, path)
        })
        .collect()
}

fn act(
    f: unsafe extern "C" fn(*mut KerfScm, *const u8, usize) -> KerfStatus,
    scm: *mut KerfScm,
    s: &str,
) -> KerfStatus {
    unsafe { f(scm, s.as_ptr(), s.len()) }
}

fn error(scm: *mut KerfScm) -> KerfScmError {
    let mut e = KerfScmError::Git;
    assert_eq!(unsafe { kerf_scm_error(scm, &mut e) }, KerfStatus::Ok);
    e
}

/// An empty repository on `main` with an identity, so commits work whatever
/// the machine's global config says.
fn repo(dir: &Path) {
    let mut opts = git2::RepositoryInitOptions::new();
    let repo = git2::Repository::init_opts(dir, opts.initial_head("main")).unwrap();
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Kerf Test").unwrap();
    config.set_str("user.email", "test@kerf.invalid").unwrap();
}

#[test]
fn host_reads_groups_diffs_stages_and_commits_through_the_c_functions() {
    let dir = tempfile::tempdir().unwrap();
    repo(dir.path());
    std::fs::write(dir.path().join("a.txt"), "one\ntwo\n").unwrap();
    let (status, scm, _) = open(dir.path());
    assert_eq!(status, KerfStatus::Ok);

    let summary = refresh(scm);
    assert_eq!(
        (summary.staged, summary.changes, summary.detached),
        (0, 1, 0)
    );
    assert_eq!(
        string(|b, c, l| unsafe { kerf_scm_branch(scm, b, c, l) }),
        "main"
    );
    assert_eq!(
        files(scm, KERF_SCM_GROUP_CHANGES, 1),
        [(KerfScmChange::Untracked, "a.txt".to_owned())]
    );

    assert_eq!(act(kerf_scm_stage, scm, "a.txt"), KerfStatus::Ok);
    assert_eq!(refresh(scm).staged, 1);
    assert_eq!(act(kerf_scm_commit, scm, "first"), KerfStatus::Ok);
    assert_eq!(act(kerf_scm_commit, scm, "again"), KerfStatus::Nothing);
    assert_eq!(error(scm), KerfScmError::NothingToCommit);
    assert!(!string(|b, c, l| unsafe { kerf_scm_error_message(scm, b, c, l) }).is_empty());

    std::fs::write(dir.path().join("a.txt"), "one\n2\n").unwrap();
    let summary = refresh(scm);
    assert_eq!(error(scm), KerfScmError::None);
    assert_eq!(
        files(scm, KERF_SCM_GROUP_CHANGES, summary.changes),
        [(KerfScmChange::Modified, "a.txt".to_owned())]
    );

    let mut info = KerfScmDiffInfo::default();
    let path = "a.txt";
    assert_eq!(
        unsafe {
            kerf_scm_load_diff(
                scm,
                KERF_SCM_GROUP_CHANGES,
                path.as_ptr(),
                path.len(),
                &mut info,
            )
        },
        KerfStatus::Ok
    );
    assert_eq!((info.rows, info.binary, info.truncated), (4, 0, 0));
    let rows: Vec<_> = (0..info.rows)
        .map(|i| {
            let mut row = KerfScmRow {
                kind: KerfScmRowKind::Hunk,
                old_line: 9,
                new_line: 9,
            };
            let text = string(|b, c, l| unsafe { kerf_scm_diff_row(scm, i, &mut row, b, c, l) });
            (row.kind, row.old_line, row.new_line, text)
        })
        .collect();
    assert_eq!(
        rows,
        [
            (KerfScmRowKind::Hunk, 0, 0, "@@ -1,2 +1,2 @@".to_owned()),
            (KerfScmRowKind::Context, 1, 1, "one".to_owned()),
            (KerfScmRowKind::Removed, 2, 0, "two".to_owned()),
            (KerfScmRowKind::Added, 0, 2, "2".to_owned()),
        ]
    );

    assert_eq!(act(kerf_scm_stage, scm, "a.txt"), KerfStatus::Ok);
    assert_eq!(refresh(scm).staged, 1);
    assert_eq!(act(kerf_scm_unstage, scm, "a.txt"), KerfStatus::Ok);
    let summary = refresh(scm);
    assert_eq!((summary.staged, summary.changes), (0, 1));

    unsafe { kerf_scm_free(scm) };
}

#[test]
fn refusals_and_broken_inputs_come_back_as_statuses() {
    let outside = tempfile::tempdir().unwrap();
    let (status, scm, error_code) = open(outside.path());
    assert_eq!(status, KerfStatus::Io);
    assert!(scm.is_null());
    assert_eq!(error_code, KerfScmError::NotARepository);

    let dir = tempfile::tempdir().unwrap();
    repo(dir.path());
    let (_, scm, _) = open(dir.path());

    assert_eq!(act(kerf_scm_stage, scm, "../x"), KerfStatus::OutOfRange);
    assert_eq!(error(scm), KerfScmError::BadPath);
    assert_eq!(act(kerf_scm_commit, scm, "  "), KerfStatus::Nothing);
    assert_eq!(error(scm), KerfScmError::EmptyMessage);
    let big = "x".repeat(kerf_scm::MAX_MESSAGE_BYTES + 1);
    assert_eq!(act(kerf_scm_commit, scm, &big), KerfStatus::TooLarge);

    let bad = [0xffu8];
    assert_eq!(
        unsafe { kerf_scm_stage(scm, bad.as_ptr(), 1) },
        KerfStatus::InvalidUtf8
    );
    assert_eq!(error(scm), KerfScmError::None);
    assert_eq!(
        unsafe { kerf_scm_stage(scm, null(), 3) },
        KerfStatus::NullPointer
    );
    assert_eq!(
        unsafe { kerf_scm_stage(null_mut(), b"a".as_ptr(), 1) },
        KerfStatus::NullPointer
    );
    assert_eq!(
        unsafe { kerf_scm_refresh(scm, null_mut()) },
        KerfStatus::NullPointer
    );

    let mut change = KerfScmChange::Added;
    let mut len = 0;
    assert_eq!(
        unsafe { kerf_scm_file(scm, 7, 0, &mut change, null_mut(), 0, &mut len) },
        KerfStatus::Unsupported
    );
    assert_eq!(
        unsafe {
            kerf_scm_file(
                scm,
                KERF_SCM_GROUP_STAGED,
                0,
                &mut change,
                null_mut(),
                0,
                &mut len,
            )
        },
        KerfStatus::OutOfRange
    );
    let mut row = KerfScmRow {
        kind: KerfScmRowKind::Hunk,
        old_line: 0,
        new_line: 0,
    };
    assert_eq!(
        unsafe { kerf_scm_diff_row(scm, 0, &mut row, null_mut(), 0, &mut len) },
        KerfStatus::OutOfRange
    );

    unsafe { kerf_scm_free(scm) };
    unsafe { kerf_scm_free(null_mut()) };
}
