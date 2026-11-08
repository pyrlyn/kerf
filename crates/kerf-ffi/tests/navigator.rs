// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The navigator and Open Quickly through the C functions only, and the
//! inputs a broken host can send.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Every call here plays the host within the header's contract; a SAFETY
// note per call would only repeat the header.
#![allow(clippy::undocumented_unsafe_blocks)]

use std::ptr::{null, null_mut};

use kerf_ffi::KerfStatus;
use kerf_ffi::navigator::*;

fn open(root: &str) -> (KerfStatus, *mut KerfNavigator) {
    let mut nav = null_mut();
    let status = unsafe { kerf_navigator_open(root.as_ptr(), root.len(), &mut nav) };
    (status, nav)
}

fn rows(nav: *mut KerfNavigator) -> Vec<KerfNavigatorRow> {
    let mut len = 0;
    unsafe { kerf_navigator_rows(nav, null_mut(), 0, &mut len) };
    let mut buf = vec![KerfNavigatorRow::default(); len];
    let status = unsafe { kerf_navigator_rows(nav, buf.as_mut_ptr(), len, &mut len) };
    assert_eq!(status, KerfStatus::Ok);
    buf
}

type TextFn =
    unsafe extern "C" fn(*mut KerfNavigator, usize, u32, *mut u8, usize, *mut usize) -> KerfStatus;

fn text(f: TextFn, nav: *mut KerfNavigator, index: usize, field: u32) -> String {
    let mut len = 0;
    unsafe { f(nav, index, field, null_mut(), 0, &mut len) };
    let mut buf = vec![0u8; len];
    assert_eq!(
        unsafe { f(nav, index, field, buf.as_mut_ptr(), len, &mut len) },
        KerfStatus::Ok
    );
    String::from_utf8(buf).unwrap()
}

fn names(nav: *mut KerfNavigator) -> Vec<String> {
    (0..rows(nav).len())
        .map(|i| text(kerf_navigator_row_text, nav, i, KERF_NAVIGATOR_TEXT_NAME))
        .collect()
}

fn utf8(
    nav: *mut KerfNavigator,
    f: unsafe extern "C" fn(*mut KerfNavigator, *const u8, usize) -> KerfStatus,
    s: &str,
) -> KerfStatus {
    unsafe { f(nav, s.as_ptr(), s.len()) }
}

fn query(nav: *mut KerfNavigator, q: &str) -> usize {
    let mut count = usize::MAX;
    let status = unsafe { kerf_navigator_quick_query(nav, q.as_ptr(), q.len(), &mut count) };
    assert_eq!(status, KerfStatus::Ok);
    count
}

#[test]
fn host_drives_the_tree_and_open_quickly() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::create_dir_all(dir.path().join("target")).unwrap();
    std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
    std::fs::write(dir.path().join("src/queue.rs"), "struct BoundedQueue;\n").unwrap();
    std::fs::write(dir.path().join("src/notes.txt"), "").unwrap();
    let (status, nav) = open(dir.path().to_str().unwrap());
    assert_eq!(status, KerfStatus::Ok);
    assert_eq!(names(nav), ["src", ".gitignore"]);

    let cmd = |row, c| unsafe { kerf_navigator_row_command(nav, row, c) };
    assert_eq!(cmd(0, KERF_NAVIGATOR_EXPAND), KerfStatus::Ok);
    assert_eq!(cmd(0, KERF_NAVIGATOR_EXPAND), KerfStatus::Nothing);
    assert_eq!(names(nav), ["src", "notes.txt", "queue.rs", ".gitignore"]);
    assert_eq!(cmd(2, KERF_NAVIGATOR_SELECT), KerfStatus::Ok);
    let r = rows(nav);
    assert_eq!(
        (r[0].folder, r[0].expanded, r[2].depth, r[2].selected),
        (1, 1, 1, 1)
    );
    let path = text(kerf_navigator_row_text, nav, 2, KERF_NAVIGATOR_TEXT_PATH);
    assert_eq!(
        std::path::Path::new(&path),
        std::path::Path::new("src").join("queue.rs")
    );
    assert_eq!(cmd(9, KERF_NAVIGATOR_SELECT), KerfStatus::OutOfRange);
    assert_eq!(cmd(0, 99), KerfStatus::Unsupported);

    assert_eq!(
        utf8(nav, kerf_navigator_set_filter, "NOTES"),
        KerfStatus::Ok
    );
    assert_eq!(names(nav), ["src", "notes.txt"]);
    assert_eq!(utf8(nav, kerf_navigator_set_filter, ""), KerfStatus::Ok);
    assert_eq!(cmd(0, KERF_NAVIGATOR_COLLAPSE), KerfStatus::Ok);
    assert_eq!(names(nav), ["src", ".gitignore"]);
    assert_eq!(
        rows(nav)[0].selected,
        1,
        "selection moved to the collapsed folder"
    );

    // Symbols come from files the host indexed.
    assert_eq!(query(nav, "bounded"), 0);
    assert_eq!(
        utf8(nav, kerf_navigator_index_file, "src/queue.rs"),
        KerfStatus::Ok
    );
    assert_eq!(
        utf8(nav, kerf_navigator_index_file, "src/notes.txt"),
        KerfStatus::Unsupported
    );
    assert_eq!(
        utf8(nav, kerf_navigator_index_file, "../x.rs"),
        KerfStatus::OutOfRange
    );
    assert_eq!(
        utf8(nav, kerf_navigator_index_file, "src/gone.rs"),
        KerfStatus::Io
    );
    assert_eq!(query(nav, "Bounded"), 1);
    // The path match right after `/` beats the one inside the symbol name.
    assert_eq!(query(nav, "que"), 2);
    let mut hits = [KerfQuickHit::default(); 2];
    let mut len = 0;
    let status = unsafe { kerf_navigator_hits(nav, hits.as_mut_ptr(), 2, &mut len) };
    assert_eq!((status, len), (KerfStatus::Ok, 2));
    assert_eq!(
        (hits[0].symbol, hits[0].selected, hits[1].symbol),
        (0, 1, 1)
    );
    assert_eq!(
        text(kerf_navigator_hit_text, nav, 1, KERF_NAVIGATOR_TEXT_NAME),
        "BoundedQueue"
    );
    assert_eq!(
        text(kerf_navigator_hit_text, nav, 1, KERF_NAVIGATOR_TEXT_DETAIL),
        "class"
    );
    let mut ranges = [KerfByteRange::default(); 4];
    let status = unsafe { kerf_navigator_hit_ranges(nav, 1, ranges.as_mut_ptr(), 4, &mut len) };
    assert_eq!((status, len), (KerfStatus::Ok, 1));
    assert_eq!((ranges[0].start, ranges[0].end), (7, 10));
    assert_eq!(unsafe { kerf_navigator_quick_move(nav, 1) }, KerfStatus::Ok);
    assert_eq!(
        unsafe { kerf_navigator_quick_move(nav, 1) },
        KerfStatus::Nothing
    );
    let status = unsafe { kerf_navigator_hits(nav, hits.as_mut_ptr(), 1, &mut len) };
    assert_eq!((status, len), (KerfStatus::TooLarge, 2));
    let mut one = 0;
    assert_eq!(
        unsafe { kerf_navigator_hit_ranges(nav, 5, null_mut(), 0, &mut one) },
        KerfStatus::OutOfRange
    );

    unsafe { kerf_navigator_free(nav) };
}

#[test]
fn broken_host_input_is_refused() {
    let file = tempfile::NamedTempFile::new().unwrap();
    assert_eq!(open(file.path().to_str().unwrap()).0, KerfStatus::Io);
    let mut out = null_mut();
    assert_eq!(
        unsafe { kerf_navigator_open(null(), 3, &mut out) },
        KerfStatus::NullPointer
    );
    let mut len = 0;
    assert_eq!(
        unsafe { kerf_navigator_rows(null_mut(), null_mut(), 0, &mut len) },
        KerfStatus::NullPointer
    );
    assert_eq!(
        unsafe { kerf_navigator_quick_move(null_mut(), 1) },
        KerfStatus::NullPointer
    );
    unsafe { kerf_navigator_free(null_mut()) };
}
