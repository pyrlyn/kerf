// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The Terminal panel through the C functions only: refuse an untrusted
//! folder, run a command, read coloured cells back, hit the output cap,
//! mark a plugin's tab, close. Every wait has a deadline.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Every call here plays the host within the header's contract; a SAFETY
// note per call would only repeat the header.
#![allow(clippy::undocumented_unsafe_blocks)]

use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

use kerf_core::TrustStore;
use kerf_ffi::*;

struct Panel {
    terms: *mut KerfTerminals,
    dir: tempfile::TempDir,
}

impl Panel {
    /// A handle whose trust file trusts `dir/trusted` only.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        for sub in ["trusted", "untrusted"] {
            std::fs::create_dir(dir.path().join(sub)).unwrap();
        }
        let trust_file = dir.path().join("trust.toml");
        let mut trust = TrustStore::new();
        trust.grant(&dir.path().join("trusted")).unwrap();
        trust.save(&trust_file).unwrap();
        let path = trust_file.to_str().unwrap();
        let mut terms = null_mut();
        assert_eq!(
            unsafe { kerf_terminals_new(path.as_ptr(), path.len(), &mut terms) },
            KerfStatus::Ok
        );
        Self { terms, dir }
    }

    fn open(&self, folder: &str, plugin: &str, out: &mut u64) -> KerfStatus {
        let cwd = self.dir.path().join(folder);
        let cwd = cwd.to_str().unwrap();
        let program = "/bin/sh";
        unsafe {
            kerf_terminal_open(
                self.terms,
                cwd.as_ptr(),
                cwd.len(),
                program.as_ptr(),
                program.len(),
                plugin.as_ptr(),
                plugin.len(),
                24,
                80,
                out,
            )
        }
    }

    fn write(&self, id: u64, s: &str) {
        assert_eq!(
            unsafe { kerf_terminal_write(self.terms, id, s.as_ptr(), s.len()) },
            KerfStatus::Ok
        );
    }

    fn text(&self, id: u64, which: u32) -> String {
        // The screen can change between the length query and the copy, so
        // retry with the newer length, as a host would.
        let mut len = 0;
        let mut buf = Vec::new();
        loop {
            let status = unsafe {
                kerf_terminal_copy_text(
                    self.terms,
                    id,
                    which,
                    buf.as_mut_ptr(),
                    buf.len(),
                    &mut len,
                )
            };
            match status {
                KerfStatus::Ok => break,
                KerfStatus::TooLarge => buf.resize(len, 0),
                other => panic!("copy_text: {other:?}"),
            }
        }
        buf.truncate(len);
        String::from_utf8(buf).unwrap()
    }

    fn info(&self, id: u64) -> KerfTerminalInfo {
        let mut info = KerfTerminalInfo::default();
        assert_eq!(
            unsafe { kerf_terminal_info(self.terms, id, &mut info) },
            KerfStatus::Ok
        );
        info
    }

    fn wait_for(&self, id: u64, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !self.text(id, KERF_TERMINAL_TEXT_SCREEN).contains(needle) {
            assert!(Instant::now() < deadline, "timed out waiting for {needle}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        unsafe { kerf_terminals_free(self.terms) };
    }
}

#[test]
fn a_command_runs_and_its_cells_come_back_coloured() {
    let panel = Panel::new();
    let mut id = 0;
    assert_eq!(panel.open("untrusted", "", &mut id), KerfStatus::Untrusted);
    assert_eq!(panel.open("trusted", "", &mut id), KerfStatus::Ok);

    panel.write(id, "printf '\\033[32mok-%s\\033[0m\\n' $((6*7))\n");
    panel.wait_for(id, "ok-42");

    let mut len = 0;
    assert_eq!(
        unsafe { kerf_terminal_cells(panel.terms, id, null_mut(), 0, &mut len) },
        KerfStatus::TooLarge
    );
    assert_eq!(len, 24 * 80);
    let mut cells = vec![KerfTerminalCell::default(); len];
    assert_eq!(
        unsafe { kerf_terminal_cells(panel.terms, id, cells.as_mut_ptr(), len, &mut len) },
        KerfStatus::Ok
    );
    let o = cells
        .iter()
        .zip(cells.iter().skip(1))
        .find(|(a, b)| a.text[0] == b'o' && b.text[0] == b'k' && a.fg.kind != 0)
        .map(|(a, _)| *a)
        .expect("the green cell");
    assert_eq!(o.text_len, 1);
    assert_eq!(o.fg.kind, KERF_TERMINAL_COLOR_INDEXED);
    assert_eq!(o.fg.index, 2);

    let info = panel.info(id);
    assert_eq!((info.rows, info.cols, info.running), (24, 80, 1));
    assert_eq!(info.from_plugin, 0);
    assert_eq!(panel.text(id, KERF_TERMINAL_TEXT_TITLE), "sh");
    assert_eq!(panel.text(id, KERF_TERMINAL_TEXT_ORIGIN), "");

    panel.write(id, "exit 5\n");
    let deadline = Instant::now() + Duration::from_secs(15);
    while panel.info(id).running == 1 {
        assert!(Instant::now() < deadline, "timed out waiting for exit");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(panel.info(id).exit_code, 5);
    assert_eq!(
        unsafe { kerf_terminal_close(panel.terms, id) },
        KerfStatus::Ok
    );
    assert_eq!(
        unsafe { kerf_terminal_close(panel.terms, id) },
        KerfStatus::NotFound
    );
}

#[test]
fn output_hits_the_cap_in_a_plugin_tab() {
    let panel = Panel::new();
    let mut id = 0;
    assert_eq!(
        panel.open("trusted", "cargo-runner", &mut id),
        KerfStatus::Ok
    );
    assert_eq!(panel.info(id).from_plugin, 1);
    assert_eq!(panel.text(id, KERF_TERMINAL_TEXT_ORIGIN), "cargo-runner");

    panel.write(
        id,
        "i=1; while [ $i -le 2000 ]; do echo row$i; i=$((i+1)); done; echo d$((1+1))one\n",
    );
    panel.wait_for(id, "d2one");
    let info = panel.info(id);
    assert!(info.history_rows > 0);
    let mut offset = 0;
    assert_eq!(
        unsafe { kerf_terminal_scroll(panel.terms, id, usize::MAX, &mut offset) },
        KerfStatus::Ok
    );
    assert_eq!(offset as u32, info.history_rows);
    assert!(info.history_rows < 2000, "history is capped");
    let top = panel.text(id, KERF_TERMINAL_TEXT_SCREEN);
    assert!(!top.starts_with("row1\n"), "the oldest rows are gone");

    let mut ids = [0u64; 4];
    let mut n = 0;
    assert_eq!(
        unsafe { kerf_terminal_tabs(panel.terms, ids.as_mut_ptr(), ids.len(), &mut n) },
        KerfStatus::Ok
    );
    assert_eq!(&ids[..n], &[id]);
}

#[test]
fn broken_host_input_is_refused() {
    let panel = Panel::new();
    let mut id = 0;
    assert_eq!(
        unsafe { kerf_terminal_write(panel.terms, 99, null(), 0) },
        KerfStatus::NotFound
    );
    assert_eq!(
        unsafe { kerf_terminal_write(null_mut(), 1, null(), 0) },
        KerfStatus::NullPointer
    );
    assert_eq!(
        unsafe { kerf_terminal_resize(panel.terms, 99, 0, 80) },
        KerfStatus::OutOfRange
    );
    assert_eq!(
        unsafe { kerf_terminal_copy_text(panel.terms, 99, 9, null_mut(), 0, &mut 0) },
        KerfStatus::Unsupported
    );
    assert_eq!(panel.open("missing", "", &mut id), KerfStatus::Io);
    let big = vec![b'x'; 64 * 1024 + 1];
    assert_eq!(
        unsafe { kerf_terminal_write(panel.terms, 1, big.as_ptr(), big.len()) },
        KerfStatus::TooLarge
    );
}
