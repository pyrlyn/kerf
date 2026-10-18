// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The whole window path, headless on Slint's testing backend: open a file,
//! show it, type, undo and redo, and read back what the window shows.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use i_slint_backend_testing::{ElementHandle, ElementQuery};
use kerf_shell::EditorWindow;
use slint::ComponentHandle;
use slint::platform::{Key, WindowEvent};

/// What the element with `accessible-id: id` shows (a `Text`'s label is its text).
fn shown(window: &EditorWindow, id: &'static str) -> String {
    let element: ElementHandle = ElementQuery::from_root(window)
        .match_descendants()
        .match_predicate(move |e| e.accessible_id().as_deref() == Some(id))
        .find_first()
        .unwrap_or_else(|| panic!("no element {id}"));
    element.accessible_label().unwrap_or_default().to_string()
}

fn press(window: &EditorWindow, text: &str) {
    let text = slint::SharedString::from(text);
    let w = window.window();
    w.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    w.dispatch_event(WindowEvent::KeyReleased { text });
}

fn with_control(window: &EditorWindow, text: &str) {
    let control = slint::SharedString::from(Key::Control);
    let w = window.window();
    w.dispatch_event(WindowEvent::KeyPressed {
        text: control.clone(),
    });
    press(window, text);
    w.dispatch_event(WindowEvent::KeyReleased { text: control });
}

#[test]
fn file_opens_and_typing_undo_and_redo_reach_the_window() {
    i_slint_backend_testing::init_no_event_loop();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.rs");
    std::fs::write(&path, "fn main() {}\n").unwrap();

    let window = kerf_shell::open(Some(&path)).unwrap();
    window.show().unwrap();
    assert!(window.window().is_visible());
    assert_eq!(shown(&window, "content"), "fn main() {}\n");
    assert_eq!(shown(&window, "status"), "Ln 1, Col 1");

    for key in ["/", "/", " "] {
        press(&window, key);
    }
    press(&window, &char::from(Key::Return).to_string());
    assert_eq!(shown(&window, "content"), "// \nfn main() {}\n");
    assert_eq!(shown(&window, "status"), "Ln 2, Col 1");

    with_control(&window, "z");
    assert_eq!(shown(&window, "content"), "// fn main() {}\n");
    assert_eq!(shown(&window, "status"), "Ln 1, Col 4");

    with_control(&window, "y");
    assert_eq!(shown(&window, "content"), "// \nfn main() {}\n");

    press(&window, &char::from(Key::Backspace).to_string());
    assert_eq!(shown(&window, "content"), "// fn main() {}\n");
}

#[test]
fn files_over_the_cap_and_non_utf8_files_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("blob.bin");
    std::fs::write(&binary, [0xff, 0xfe, 0x00]).unwrap();
    assert!(matches!(
        kerf_shell::read_text(&binary),
        Err(kerf_shell::OpenError::NotUtf8 { .. })
    ));

    let big = dir.path().join("big.txt");
    let file = std::fs::File::create(&big).unwrap();
    // Sparse on most file systems: the cap is checked without writing 64 MiB.
    file.set_len(kerf_shell::MAX_FILE_BYTES + 1).unwrap();
    assert!(matches!(
        kerf_shell::read_text(&big),
        Err(kerf_shell::OpenError::TooLarge { .. })
    ));
}
