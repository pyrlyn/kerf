// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Real shells through a real PTY. Every wait has a deadline, so a broken
//! PTY fails a test instead of hanging it.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant};

use kerf_core::{TrustError, TrustStore};
use kerf_terminal::{
    Color, HISTORY_ROWS, Origin, ProcessState, Size, Spec, TabId, TerminalError, Terminals,
};

const SIZE: Size = Size { rows: 24, cols: 80 };

fn trusted() -> (tempfile::TempDir, TrustStore) {
    let dir = tempfile::tempdir().unwrap();
    let mut trust = TrustStore::new();
    trust.grant(dir.path()).unwrap();
    (dir, trust)
}

fn spec(dir: &tempfile::TempDir, argv: &[&str], origin: Origin) -> Spec {
    Spec {
        cwd: dir.path().to_path_buf(),
        argv: argv.iter().map(|s| (*s).to_owned()).collect(),
        size: SIZE,
        origin,
    }
}

fn screen_text(terms: &mut Terminals, id: TabId) -> String {
    let snap = terms.snapshot(id).unwrap();
    (0..snap.size.rows)
        .map(|r| snap.row_text(r))
        .collect::<Vec<_>>()
        .join("\n")
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_shell_command_runs_and_its_coloured_output_reaches_the_screen() {
    let (dir, trust) = trusted();
    let mut terms = Terminals::new();
    let id = terms
        .open(&trust, spec(&dir, &["/bin/sh"], Origin::User))
        .unwrap();

    // Arithmetic, so the echoed command line cannot satisfy the wait.
    terms
        .write(id, b"printf '\\033[31mkerf-%s\\033[0m\\n' $((40+2)); pwd\n")
        .unwrap();
    wait_for("command output", || {
        screen_text(&mut terms, id).contains("kerf-42")
    });

    let snap = terms.snapshot(id).unwrap();
    let row = (0..snap.size.rows)
        .find(|r| snap.row_text(*r) == "kerf-42")
        .expect("output row");
    let first = &snap.cells[usize::from(row) * usize::from(SIZE.cols)];
    assert_eq!(first.text, "k");
    assert_eq!(first.fg, Color::Indexed(1));
    let canonical = std::fs::canonicalize(dir.path()).unwrap();
    assert!(screen_text(&mut terms, id).contains(canonical.to_str().unwrap()));

    let info = terms.info(id).unwrap();
    assert_eq!(info.title, "sh");
    assert_eq!(info.origin, Origin::User);
    assert_eq!(info.state, ProcessState::Running);
    assert!(info.output_bytes > 0);

    terms.write(id, b"exit 3\n").unwrap();
    wait_for("exit", || {
        terms.info(id).unwrap().state == ProcessState::Exited { code: 3 }
    });
    terms.close(id).unwrap();
    assert!(terms.tabs().is_empty());
}

#[test]
fn output_past_the_cap_drops_the_oldest_history() {
    let (dir, trust) = trusted();
    let mut terms = Terminals::new();
    let lines = HISTORY_ROWS * 3;
    let script = format!("i=1; while [ $i -le {lines} ]; do echo line$i; i=$((i+1)); done");
    let id = terms
        .open(
            &trust,
            spec(&dir, &["/bin/sh", "-c", &script], Origin::User),
        )
        .unwrap();

    let last = format!("line{lines}");
    wait_for("the last line", || {
        screen_text(&mut terms, id).contains(&last)
    });
    let info = terms.info(id).unwrap();
    assert_eq!(info.history_rows, HISTORY_ROWS);

    let offset = terms.scroll(id, usize::MAX).unwrap();
    assert_eq!(offset, HISTORY_ROWS);
    let top = terms.snapshot(id).unwrap().row_text(0);
    let n: usize = top.trim_start_matches("line").parse().unwrap();
    assert!(n > 1, "the first line should be gone, top is {top}");
    assert_eq!(terms.scroll(id, 0).unwrap(), 0);
}

#[test]
fn an_untrusted_folder_gets_no_shell() {
    let dir = tempfile::tempdir().unwrap();
    let mut terms = Terminals::new();
    let err = terms
        .open(&TrustStore::new(), spec(&dir, &["/bin/sh"], Origin::User))
        .unwrap_err();
    assert!(matches!(
        err,
        TerminalError::Trust(TrustError::Untrusted { .. })
    ));
    assert!(terms.tabs().is_empty());
}

#[test]
fn a_plugin_tab_is_marked_resizes_and_closes() {
    let (dir, trust) = trusted();
    let mut terms = Terminals::new();
    let user = terms
        .open(&trust, spec(&dir, &["/bin/sh"], Origin::User))
        .unwrap();
    let plugin = terms
        .open(
            &trust,
            spec(&dir, &["/bin/sh"], Origin::Plugin("cargo-runner".into())),
        )
        .unwrap();
    assert_eq!(terms.tabs(), vec![user, plugin]);
    assert_eq!(
        terms.info(plugin).unwrap().origin,
        Origin::Plugin("cargo-runner".into())
    );

    let size = Size {
        rows: 30,
        cols: 100,
    };
    terms.resize(plugin, size).unwrap();
    terms.write(plugin, b"stty size\n").unwrap();
    wait_for("the new size", || {
        screen_text(&mut terms, plugin).contains("30 100")
    });
    assert_eq!(terms.snapshot(plugin).unwrap().cells.len(), 3000);

    terms.close(plugin).unwrap();
    assert!(matches!(
        terms.write(plugin, b"x"),
        Err(TerminalError::NoSuchTab(_))
    ));
    assert_eq!(terms.tabs(), vec![user]);
}
