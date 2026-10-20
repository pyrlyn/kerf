// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The whole host path through the C functions only: open a file, type,
//! delete, undo, redo, select and replace, render on the CPU, place the caret, read pixels; and
//! the inputs a broken host can send.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Every call here plays the host within the header's contract; a SAFETY
// note per call would only repeat the header.
#![allow(clippy::undocumented_unsafe_blocks)]

use std::ptr::{null, null_mut};

use kerf_ffi::*;

fn open(path: &std::path::Path) -> (KerfStatus, *mut KerfEditor) {
    let path = path.to_str().unwrap();
    let mut ed = null_mut();
    let status = unsafe { kerf_editor_open(path.as_ptr(), path.len(), &mut ed) };
    (status, ed)
}

fn text(ed: *mut KerfEditor) -> String {
    let mut len = 0;
    assert_eq!(
        unsafe { kerf_editor_copy_text(ed, null_mut(), 0, &mut len) },
        if len == 0 {
            KerfStatus::Ok
        } else {
            KerfStatus::TooLarge
        }
    );
    let mut buf = vec![0u8; len];
    assert_eq!(
        unsafe { kerf_editor_copy_text(ed, buf.as_mut_ptr(), buf.len(), &mut len) },
        KerfStatus::Ok
    );
    String::from_utf8(buf).unwrap()
}

fn insert(ed: *mut KerfEditor, s: &str) -> KerfStatus {
    unsafe { kerf_editor_insert(ed, s.as_ptr(), s.len()) }
}

fn command(ed: *mut KerfEditor, c: u32) -> KerfStatus {
    unsafe { kerf_editor_command(ed, c) }
}

fn selection(ed: *mut KerfEditor) -> (usize, usize) {
    let (mut anchor, mut cursor) = (usize::MAX, usize::MAX);
    assert_eq!(
        unsafe { kerf_editor_selection(ed, &mut anchor, &mut cursor) },
        KerfStatus::Ok
    );
    (anchor, cursor)
}

#[test]
fn host_opens_edits_and_renders_through_the_c_functions() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("main.rs");
    std::fs::write(&file, "fn main() {}\n").unwrap();
    let (status, ed) = open(&file);
    assert_eq!(status, KerfStatus::Ok);

    assert_eq!(command(ed, KERF_COMMAND_LEFT), KerfStatus::Nothing);
    assert_eq!(insert(ed, "// é\n"), KerfStatus::Ok);
    assert_eq!(command(ed, KERF_COMMAND_BACKSPACE), KerfStatus::Ok);
    assert_eq!(text(ed), "// éfn main() {}\n");
    assert_eq!(command(ed, KERF_COMMAND_UNDO), KerfStatus::Ok);
    assert_eq!(text(ed), "// é\nfn main() {}\n");
    assert_eq!(command(ed, KERF_COMMAND_REDO), KerfStatus::Ok);
    assert_eq!(command(ed, KERF_COMMAND_REDO), KerfStatus::Nothing);
    let mut cursor = 0;
    assert_eq!(
        unsafe { kerf_editor_cursor(ed, &mut cursor) },
        KerfStatus::Ok
    );
    assert_eq!(cursor, "// é".len());

    // Shift-Left twice from after "é" selects " é"; typing replaces it.
    assert_eq!(command(ed, KERF_COMMAND_SELECT_LEFT), KerfStatus::Ok);
    assert_eq!(command(ed, KERF_COMMAND_SELECT_LEFT), KerfStatus::Ok);
    assert_eq!(selection(ed), (5, 2));
    assert_eq!(insert(ed, "!"), KerfStatus::Ok);
    assert_eq!(text(ed), "//!fn main() {}\n");
    assert_eq!(selection(ed), (3, 3));
    assert_eq!(command(ed, KERF_COMMAND_UNDO), KerfStatus::Ok);
    assert_eq!(text(ed), "// éfn main() {}\n");
    assert_eq!(unsafe { kerf_editor_select(ed, 0, 2) }, KerfStatus::Ok);
    assert_eq!(command(ed, KERF_COMMAND_SELECT_RIGHT), KerfStatus::Ok);
    assert_eq!(selection(ed), (0, 3));
    assert_eq!(unsafe { kerf_editor_select(ed, 5, 5) }, KerfStatus::Ok);

    let family = "Menlo";
    let theme = KerfTheme {
        font_family: family.as_ptr(),
        font_family_len: family.len(),
        font_size: 13.0,
        line_height: 17.0,
        background: [30, 30, 30, 255],
        text: [250, 250, 250, 255],
        selection: [40, 80, 200, 255],
        caret: [255, 0, 0, 255],
        caret_width: 2.0,
        draw_caret: 1,
    };
    let mut view = null_mut();
    let mut len = 0;
    assert_eq!(
        unsafe { kerf_view_new(160, 40, 2.0, &theme, &mut view) },
        KerfStatus::Ok
    );
    // No frame before the first render.
    assert_eq!(
        unsafe { kerf_view_read_rgba(view, null_mut(), 0, &mut len) },
        KerfStatus::NullPointer
    );
    assert_eq!(len, 0);
    assert_eq!(unsafe { kerf_view_render(view, ed) }, KerfStatus::Ok);
    let mut caret = KerfRect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };
    assert_eq!(unsafe { kerf_view_caret(view, &mut caret) }, KerfStatus::Ok);
    // Line 0 after "// é", in logical points whatever the scale.
    assert_eq!((caret.y, caret.height, caret.width), (0.0, 17.0, 2.0));
    assert!(caret.x > 0.0);

    let mut short = [0u8; 4];
    let status = unsafe { kerf_view_read_rgba(view, short.as_mut_ptr(), short.len(), &mut len) };
    assert_eq!((status, len), (KerfStatus::TooLarge, 160 * 40 * 4));
    let mut rgba = vec![0u8; len];
    assert_eq!(
        unsafe { kerf_view_read_rgba(view, rgba.as_mut_ptr(), rgba.len(), &mut len) },
        KerfStatus::Ok
    );
    let at = |x: usize, y: usize| &rgba[(y * 160 + x) * 4..][..4];
    assert_eq!(at(159, 39), [30, 30, 30, 255]);
    // The caret in physical pixels: scale 2, one pixel into its 4 px width.
    let (cx, cy) = ((caret.x * 2.0 + 1.0) as usize, caret.height as usize);
    assert!(
        at(cx, cy)[0] > 200 && at(cx, cy)[1] < 60,
        "caret pixel {:?}",
        at(cx, cy)
    );

    assert_eq!(
        unsafe { kerf_view_resize(view, 80, 20, 1.0) },
        KerfStatus::Ok
    );
    assert_eq!(unsafe { kerf_view_set_theme(view, null()) }, KerfStatus::Ok);
    assert_eq!(unsafe { kerf_view_render(view, ed) }, KerfStatus::Ok);
    assert_eq!(
        unsafe { kerf_view_read_rgba(view, null_mut(), 0, &mut len) },
        KerfStatus::TooLarge
    );
    assert_eq!(len, 80 * 20 * 4);

    unsafe {
        kerf_view_free(view);
        kerf_editor_free(ed);
    }
}

#[test]
fn broken_host_input_gets_a_status_not_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(open(&dir.path().join("missing")).0, KerfStatus::Io);
    let binary = dir.path().join("bin");
    std::fs::write(&binary, [0xff, 0xfe]).unwrap();
    assert_eq!(open(&binary).0, KerfStatus::InvalidUtf8);
    let bad_path = [0xffu8];
    let mut ed = null_mut();
    assert_eq!(
        unsafe { kerf_editor_open(bad_path.as_ptr(), 1, &mut ed) },
        KerfStatus::InvalidUtf8
    );
    assert_eq!(
        unsafe { kerf_editor_open(null(), 3, &mut ed) },
        KerfStatus::NullPointer
    );
    assert_eq!(
        unsafe { kerf_editor_new(null_mut()) },
        KerfStatus::NullPointer
    );

    assert_eq!(unsafe { kerf_editor_new(&mut ed) }, KerfStatus::Ok);
    assert_eq!(insert(null_mut(), "a"), KerfStatus::NullPointer);
    assert_eq!(
        unsafe { kerf_editor_insert(ed, bad_path.as_ptr(), 1) },
        KerfStatus::InvalidUtf8
    );
    assert_eq!(
        unsafe { kerf_editor_insert(ed, bad_path.as_ptr(), KERF_MAX_INSERT_BYTES + 1) },
        KerfStatus::TooLarge
    );
    assert_eq!(command(ed, 99), KerfStatus::Unsupported);
    assert_eq!(insert(ed, "abé"), KerfStatus::Ok);
    // Past the end, inside "é", and null out-pointers change nothing.
    assert_eq!(
        unsafe { kerf_editor_select(ed, 0, 9) },
        KerfStatus::OutOfRange
    );
    assert_eq!(
        unsafe { kerf_editor_select(ed, 3, 0) },
        KerfStatus::OutOfRange
    );
    assert_eq!(selection(ed), (4, 4));
    let mut at = 0;
    assert_eq!(
        unsafe { kerf_editor_selection(ed, &mut at, null_mut()) },
        KerfStatus::NullPointer
    );
    assert_eq!(
        unsafe { kerf_editor_selection(ed, null_mut(), &mut at) },
        KerfStatus::NullPointer
    );
    assert_eq!(at, 0);
    assert_eq!(command(ed, KERF_COMMAND_BACKSPACE), KerfStatus::Ok);
    assert_eq!(insert(ed, "c"), KerfStatus::Ok);
    let mut len = 0;
    let mut two = [0u8; 2];
    assert_eq!(
        unsafe { kerf_editor_copy_text(ed, two.as_mut_ptr(), 2, &mut len) },
        KerfStatus::TooLarge
    );
    assert_eq!((len, two), (3, [0, 0]));
    assert_eq!(
        unsafe { kerf_editor_copy_text(ed, null_mut(), 8, &mut len) },
        KerfStatus::NullPointer
    );

    let theme = KerfTheme {
        font_family: bad_path.as_ptr(),
        font_family_len: 1,
        font_size: 13.0,
        line_height: 17.0,
        background: [0; 4],
        text: [0; 4],
        selection: [0; 4],
        caret: [0; 4],
        caret_width: 1.0,
        draw_caret: 7,
    };
    let mut view = null_mut();
    assert_eq!(
        unsafe { kerf_view_new(10, 10, 1.0, &theme, &mut view) },
        KerfStatus::InvalidUtf8
    );
    assert_eq!(
        unsafe { kerf_view_render(null_mut(), ed) },
        KerfStatus::NullPointer
    );
    assert_eq!(
        unsafe { kerf_view_caret(null_mut(), null_mut()) },
        KerfStatus::NullPointer
    );
    unsafe {
        kerf_view_free(null_mut());
        kerf_editor_free(null_mut());
        kerf_editor_free(ed);
    }
}

#[test]
fn a_view_used_from_another_thread_gets_a_status() {
    let mut ed = null_mut();
    let mut view = null_mut();
    assert_eq!(unsafe { kerf_editor_new(&mut ed) }, KerfStatus::Ok);
    assert_eq!(
        unsafe { kerf_view_new(20, 20, 1.0, null(), &mut view) },
        KerfStatus::Ok
    );
    // A host that breaks the header's one-thread rule; the handles cross as
    // integers because raw pointers are not `Send`.
    let (v, e) = (view as usize, ed as usize);
    let status = std::thread::spawn(move || unsafe {
        kerf_view_render(v as *mut KerfView, e as *mut KerfEditor)
    })
    .join()
    .unwrap();
    assert_eq!(status, KerfStatus::WrongThread);
    // Freed from the wrong thread, the view is leaked, not dropped there.
    std::thread::spawn(move || unsafe { kerf_view_free(v as *mut KerfView) })
        .join()
        .unwrap();
    assert_eq!(unsafe { kerf_view_render(view, ed) }, KerfStatus::Ok);
    unsafe {
        kerf_view_free(view);
        kerf_editor_free(ed);
    }
}
