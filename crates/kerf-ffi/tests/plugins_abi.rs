// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The plugins screen and the permission sheet through the C functions
//! only: every row state, every decision, and the inputs a broken host can
//! send.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Every call here plays the host within the header's contract; a SAFETY
// note per call would only repeat the header.
#![allow(clippy::undocumented_unsafe_blocks)]

use std::fs;
use std::path::Path;
use std::ptr::{null, null_mut};

use kerf_ffi::KerfStatus;
use kerf_ffi::plugins::*;
use tempfile::TempDir;

const RUST_TOOLS: &str = r#"api = 1
id = "rust-tools"
version = "0.3.1"
name = "Rust tools"
wasm = "plugin.wat"

[capabilities.ide]
terminal = true
lsp = true

[[language_servers]]
name = "rust-tools-rust-analyzer"
languages = ["rust"]
command = "rust-analyzer"
"#;

fn manifest(id: &str, extra: &str) -> String {
    format!(
        "api = 1\nid = \"{id}\"\nversion = \"1.0.0\"\nname = \"{id}\"\nwasm = \"plugin.wat\"\n{extra}"
    )
}

fn package(dir: &Path, manifest: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("plugin.toml"), manifest).unwrap();
    fs::write(dir.join("plugin.wat"), "(module)").unwrap();
}

fn user(home: &Path, id: &str, manifest: &str) {
    let plugin = home.join("plugins").join(id);
    package(&plugin.join("versions/0123456789ab"), manifest);
    fs::write(plugin.join("current"), "0123456789ab").unwrap();
}

struct Session {
    home: TempDir,
    project: TempDir,
    p: *mut KerfPlugins,
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { kerf_plugins_free(self.p) };
    }
}

impl Session {
    fn open(setup: impl FnOnce(&Path, &Path)) -> Self {
        let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        setup(home.path(), project.path());
        let (h, j) = (
            home.path().to_str().unwrap(),
            project.path().to_str().unwrap(),
        );
        let mut p = null_mut();
        let status = unsafe { kerf_plugins_open(h.as_ptr(), h.len(), j.as_ptr(), j.len(), &mut p) };
        assert_eq!(status, KerfStatus::Ok);
        Self { home, project, p }
    }

    fn index(&self, id: &str) -> usize {
        let mut count = 0;
        assert_eq!(
            unsafe { kerf_plugins_count(self.p, &mut count) },
            KerfStatus::Ok
        );
        (0..count)
            .find(|&i| self.text(i, KERF_PLUGIN_TEXT_ID, 0) == id)
            .unwrap()
    }

    fn row(&self, index: usize) -> KerfPluginRow {
        let mut row = std::mem::MaybeUninit::uninit();
        assert_eq!(
            unsafe { kerf_plugins_row(self.p, index, row.as_mut_ptr()) },
            KerfStatus::Ok
        );
        unsafe { row.assume_init() }
    }

    fn state(&self, index: usize) -> KerfPluginState {
        self.row(index).state
    }

    fn copy(&self, index: usize, field: u32, item: usize) -> Result<String, KerfStatus> {
        let mut len = 0;
        let status =
            unsafe { kerf_plugins_copy_text(self.p, index, field, item, null_mut(), 0, &mut len) };
        if status != KerfStatus::Ok && status != KerfStatus::TooLarge {
            return Err(status);
        }
        let mut buf = vec![0u8; len];
        let status = unsafe {
            kerf_plugins_copy_text(self.p, index, field, item, buf.as_mut_ptr(), len, &mut len)
        };
        assert_eq!(status, KerfStatus::Ok);
        Ok(String::from_utf8(buf).unwrap())
    }

    fn text(&self, index: usize, field: u32, item: usize) -> String {
        self.copy(index, field, item).unwrap()
    }

    fn texts(&self, index: usize, field: u32, count: usize) -> Vec<String> {
        (0..count)
            .map(|item| self.text(index, field, item))
            .collect()
    }

    fn decide(&self, index: usize, digest: &str, allow: bool) -> KerfStatus {
        unsafe {
            kerf_plugins_decide(
                self.p,
                index,
                digest.as_ptr(),
                digest.len(),
                u8::from(allow),
            )
        }
    }

    fn allow(&self, index: usize) -> KerfStatus {
        self.decide(index, &self.text(index, KERF_PLUGIN_TEXT_DIGEST, 0), true)
    }
}

#[test]
fn the_permission_sheet_grants_lines_for_the_digest_shown() {
    let s = Session::open(|home, _| user(home, "rust-tools", RUST_TOOLS));
    let row = s.row(0);
    assert_eq!(row.state, KerfPluginState::NeedsApproval);
    assert_eq!((row.project, row.locked, row.memory_mib), (0, 0, 16));
    let lines = [
        "ide.lsp",
        "ide.terminal",
        "lsp:rust-tools-rust-analyzer rust-analyzer",
    ];
    assert_eq!(s.texts(0, KERF_PLUGIN_TEXT_LINE, row.lines), lines);
    assert_eq!(s.texts(0, KERF_PLUGIN_TEXT_ADDED, row.added), lines);
    assert_eq!(s.text(0, KERF_PLUGIN_TEXT_NAME, 0), "Rust tools");
    assert_eq!(s.text(0, KERF_PLUGIN_TEXT_VERSION, 0), "0.3.1");
    let digest = s.text(0, KERF_PLUGIN_TEXT_DIGEST, 0);
    assert_eq!(digest.len(), 64);

    // Don't allow: nothing changes.
    assert_eq!(s.decide(0, &digest, false), KerfStatus::Ok);
    assert_eq!(s.state(0), KerfPluginState::NeedsApproval);
    // Bytes the user did not see.
    assert_eq!(s.decide(0, &"0".repeat(64), true), KerfStatus::Unsupported);
    assert_eq!(s.state(0), KerfPluginState::NeedsApproval);
    // Allow.
    assert_eq!(s.decide(0, &digest, true), KerfStatus::Ok);
    assert_eq!(s.state(0), KerfPluginState::Enabled);
    // Turned off in settings: asks again.
    assert_eq!(unsafe { kerf_plugins_revoke(s.p, 0) }, KerfStatus::Ok);
    assert_eq!(s.state(0), KerfPluginState::NeedsApproval);
}

#[test]
fn a_wider_update_asks_again_for_the_new_line() {
    let s = Session::open(|home, _| user(home, "demo", &manifest("demo", "")));
    assert_eq!(s.allow(0), KerfStatus::Ok);
    user(
        s.home.path(),
        "demo",
        &manifest("demo", "[capabilities.ide]\nindex = true\n"),
    );
    assert_eq!(unsafe { kerf_plugins_rescan(s.p) }, KerfStatus::Ok);
    let row = s.row(0);
    assert_eq!(row.state, KerfPluginState::NeedsApproval);
    assert_eq!(s.texts(0, KERF_PLUGIN_TEXT_ADDED, row.added), ["ide.index"]);
}

#[test]
fn every_state_of_the_plugins_screen() {
    let s = Session::open(|home, project| {
        user(home, "rust-tools", RUST_TOOLS);
        user(home, "markdown-preview", &manifest("markdown-preview", ""));
        user(home, "git-blame", &manifest("git-blame", ""));
        user(home, "broken", &manifest("broken", "wasi = true\n"));
        package(
            &project.join(".kerf/plugins/lint-rules"),
            &manifest("lint-rules", ""),
        );
        let lock = "format = 1\n\n[plugins.git-blame]\nversion = \"1.0.0\"\n";
        let lock = format!("{lock}digest = \"{}\"\n", "0".repeat(64));
        fs::write(project.join("kerf-plugins.lock"), lock).unwrap();
    });
    let [rust, md, blame, broken, lint] = [
        "rust-tools",
        "markdown-preview",
        "git-blame",
        "broken",
        "lint-rules",
    ]
    .map(|id| s.index(id));
    for i in [rust, md] {
        assert_eq!(s.allow(i), KerfStatus::Ok);
    }
    assert_eq!(s.state(rust), KerfPluginState::Enabled);

    for _ in 0..3 {
        let export = "cox_render";
        let status = unsafe { kerf_plugins_record_call(s.p, md, export.as_ptr(), export.len(), 0) };
        assert_eq!(status, KerfStatus::Ok);
    }
    let row = s.row(md);
    assert_eq!(row.state, KerfPluginState::ExportsDisabled);
    assert_eq!(
        s.texts(md, KERF_PLUGIN_TEXT_DISABLED_EXPORT, row.disabled_exports),
        ["cox_render"]
    );

    let row = s.row(lint);
    assert_eq!(
        (row.state, row.project),
        (KerfPluginState::ProjectNotEnabled, 1)
    );
    assert_eq!(s.allow(lint), KerfStatus::Ok);
    assert_eq!(s.state(lint), KerfPluginState::Enabled);

    let row = s.row(blame);
    assert_eq!((row.state, row.locked), (KerfPluginState::LockMismatch, 1));
    assert!(s.text(blame, KERF_PLUGIN_TEXT_REASON, 0).contains("digest"));
    assert_eq!(
        s.text(blame, KERF_PLUGIN_TEXT_LOCK_DIGEST, 0),
        "0".repeat(64)
    );
    assert_eq!(s.text(blame, KERF_PLUGIN_TEXT_LOCK_VERSION, 0), "1.0.0");
    assert_eq!(s.allow(blame), KerfStatus::Unsupported);
    assert_eq!(
        unsafe { kerf_plugins_accept_lock(s.p, blame) },
        KerfStatus::Ok
    );
    assert_eq!(s.state(blame), KerfPluginState::NeedsApproval);
    let pinned = fs::read_to_string(s.project.path().join("kerf-plugins.lock")).unwrap();
    assert!(pinned.contains(&s.text(blame, KERF_PLUGIN_TEXT_DIGEST, 0)));

    assert_eq!(s.state(broken), KerfPluginState::Invalid);
    assert!(s.text(broken, KERF_PLUGIN_TEXT_REASON, 0).contains("WASI"));
    assert_eq!(s.allow(broken), KerfStatus::Unsupported);
}

#[test]
fn a_broken_host_gets_a_status_not_a_crash() {
    let s = Session::open(|home, _| user(home, "demo", &manifest("demo", "")));
    let mut out = null_mut();
    assert_eq!(
        unsafe { kerf_plugins_open(null(), 4, null(), 0, &mut out) },
        KerfStatus::NullPointer
    );
    let bad = [0xff_u8, 0xfe];
    assert_eq!(
        unsafe { kerf_plugins_open(bad.as_ptr(), bad.len(), null(), 0, &mut out) },
        KerfStatus::InvalidUtf8
    );
    assert_eq!(
        unsafe { kerf_plugins_open(b"/x".as_ptr(), 2, null(), 0, null_mut()) },
        KerfStatus::NullPointer
    );
    assert_eq!(
        unsafe { kerf_plugins_rescan(null_mut()) },
        KerfStatus::NullPointer
    );
    let mut row = std::mem::MaybeUninit::uninit();
    assert_eq!(
        unsafe { kerf_plugins_row(s.p, 9, row.as_mut_ptr()) },
        KerfStatus::OutOfRange
    );
    assert_eq!(s.copy(0, 99, 0), Err(KerfStatus::Unsupported));
    assert_eq!(
        s.copy(0, KERF_PLUGIN_TEXT_LINE, 5),
        Err(KerfStatus::OutOfRange)
    );
    assert_eq!(
        s.copy(9, KERF_PLUGIN_TEXT_ID, 0),
        Err(KerfStatus::OutOfRange)
    );
    let (mut buf, mut len) = ([0u8; 2], 0);
    let status = unsafe {
        kerf_plugins_copy_text(
            s.p,
            0,
            KERF_PLUGIN_TEXT_ID,
            0,
            buf.as_mut_ptr(),
            2,
            &mut len,
        )
    };
    assert_eq!((status, len), (KerfStatus::TooLarge, 4));
    assert_eq!(s.decide(7, "x", true), KerfStatus::OutOfRange);
    assert_eq!(
        unsafe { kerf_plugins_accept_lock(s.p, 7) },
        KerfStatus::OutOfRange
    );
    unsafe { kerf_plugins_free(null_mut()) };
}
