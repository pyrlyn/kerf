// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The Linux host's editor wrapper over the C ABI, without a display: open,
//! type, select, undo and redo through `kerf_editor_*`.

#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used)]

use std::io::Write;
use std::os::unix::ffi::OsStrExt;

use kerf_ffi::KerfStatus;
use kerf_gtk::{Command, Editor, Error};

fn file(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(bytes).unwrap();
    file
}

#[test]
fn open_type_undo_redo() {
    let file = file(b"fn main() {}\n");
    let editor = Editor::open(file.path()).unwrap();
    assert_eq!(editor.text().unwrap(), "fn main() {}\n");
    assert_eq!(editor.selection().unwrap(), (0, 0));
    editor.insert("// ").unwrap();
    assert_eq!(editor.text().unwrap(), "// fn main() {}\n");
    assert!(editor.run(Command::Undo).unwrap());
    assert_eq!(editor.text().unwrap(), "fn main() {}\n");
    assert!(editor.run(Command::Redo).unwrap());
    assert_eq!(editor.text().unwrap(), "// fn main() {}\n");
    assert!(!editor.run(Command::Redo).unwrap());
}

#[test]
fn select_and_replace() {
    let editor = Editor::new().unwrap();
    assert_eq!(editor.text().unwrap(), "");
    editor.insert("héllo").unwrap();
    assert!(editor.run(Command::Left).unwrap());
    assert!(editor.run(Command::SelectLeft).unwrap());
    assert_eq!(editor.selection().unwrap(), (5, 4));
    editor.insert("L").unwrap();
    assert_eq!(editor.text().unwrap(), "hélLo");
    editor.select(0, 3).unwrap();
    assert!(editor.run(Command::Backspace).unwrap());
    assert_eq!(editor.text().unwrap(), "lLo");
    // Inside "é" is no character boundary.
    editor.insert("é").unwrap();
    assert_eq!(editor.select(0, 1), Err(Error(KerfStatus::OutOfRange)));
}

#[test]
fn refuses_what_kerf_refuses() {
    assert_eq!(
        Editor::open(std::path::Path::new("/nonexistent/kerf")).err(),
        Some(Error(KerfStatus::Io))
    );
    let file = file(&[0xff, 0xfe]);
    assert_eq!(
        Editor::open(file.path()).err(),
        Some(Error(KerfStatus::InvalidUtf8))
    );
    // A path that is not UTF-8 reaches Kerf as bytes and is refused there.
    let path = std::ffi::OsStr::from_bytes(b"/tmp/\xff");
    assert_eq!(
        Editor::open(std::path::Path::new(path)).err(),
        Some(Error(KerfStatus::InvalidUtf8))
    );
}
