// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

use std::fs;
use std::path::Path;

use kerf_plugin::{Config, HostError, Plugins};
use tempfile::TempDir;

use super::*;

const RUST_TOOLS: &str = r#"api = 1
id = "rust-tools"
version = "0.3.1"
name = "Rust tools"
wasm = "plugin.wat"

[capabilities.ide]
buffer = true
index = true
git = true
terminal = true
lsp = true
dap = true

[[language_servers]]
name = "rust-tools-rust-analyzer"
languages = ["rust"]
command = "rust-analyzer"

[[debug_adapters]]
name = "codelldb"
languages = ["rust"]
command = "bin/codelldb"
args = ["--port", "0"]
"#;

const LINES: [&str; 8] = [
    "dap:codelldb bin/codelldb --port 0",
    "ide.buffer",
    "ide.dap",
    "ide.git",
    "ide.index",
    "ide.lsp",
    "ide.terminal",
    "lsp:rust-tools-rust-analyzer rust-analyzer",
];

fn small(id: &str, extra: &str) -> String {
    format!(
        "api = 1\nid = \"{id}\"\nversion = \"1.0.0\"\nname = \"{id}\"\nwasm = \"plugin.wat\"\n{extra}"
    )
}

fn package(dir: &Path, manifest: &str, wasm: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("plugin.toml"), manifest).unwrap();
    fs::write(dir.join("plugin.wat"), wasm).unwrap();
}

struct Roots {
    home: TempDir,
    project: TempDir,
}

impl Roots {
    fn new() -> Self {
        Self {
            home: tempfile::tempdir().unwrap(),
            project: tempfile::tempdir().unwrap(),
        }
    }

    fn user(&self, id: &str, manifest: &str, wasm: &str) {
        let plugin = self.home.path().join("plugins").join(id);
        package(&plugin.join("versions/0123456789ab"), manifest, wasm);
        fs::write(plugin.join("current"), "0123456789ab").unwrap();
    }

    fn project(&self, id: &str, manifest: &str) {
        package(
            &self.project.path().join(".kerf/plugins").join(id),
            manifest,
            "(module)",
        );
    }

    fn open(&self, grants: Grants) -> Manager {
        let home = self.home.path().to_path_buf();
        Manager::open(home, Some(self.project.path().to_path_buf()), grants)
    }
}

fn index(manager: &Manager, id: &str) -> usize {
    (0..manager.len())
        .find(|&i| manager.row(i).unwrap().id == id)
        .unwrap()
}

fn allow(manager: &mut Manager, i: usize) {
    let digest = manager.row(i).unwrap().digest;
    manager.decide(i, &digest, Decision::Allow).unwrap();
}

#[test]
fn a_new_user_plugin_asks_for_every_line_verbatim() {
    let roots = Roots::new();
    roots.user("rust-tools", RUST_TOOLS, "(module)");
    let manager = roots.open(Grants::default());
    let row = manager.row(0).unwrap();
    assert_eq!(row.lines, LINES);
    assert_eq!(row.digest.len(), 64);
    assert_eq!(row.memory_mib, 16);
    assert_eq!(row.lock, None);
    let State::NeedsApproval { added, removed } = row.state else {
        panic!("{:?}", row.state)
    };
    assert_eq!(added, LINES);
    assert!(removed.is_empty());
}

#[test]
fn allow_enables_and_deny_grants_nothing() {
    let roots = Roots::new();
    roots.user("rust-tools", RUST_TOOLS, "(module)");
    let mut manager = roots.open(Grants::default());
    let digest = manager.row(0).unwrap().digest;
    manager.decide(0, &digest, Decision::Deny).unwrap();
    assert!(manager.grants().get(&Scope::User, "rust-tools").is_none());
    assert!(matches!(
        manager.row(0).unwrap().state,
        State::NeedsApproval { .. }
    ));

    manager.decide(0, &digest, Decision::Allow).unwrap();
    assert_eq!(manager.row(0).unwrap().state, State::Enabled);
    let grant = manager.grants().get(&Scope::User, "rust-tools").unwrap();
    assert_eq!(grant.digest, digest);
    assert_eq!(grant.lines.len(), LINES.len());

    manager.revoke(0).unwrap();
    assert!(matches!(
        manager.row(0).unwrap().state,
        State::NeedsApproval { .. }
    ));
}

#[test]
fn changed_bytes_ask_again_with_nothing_new() {
    let roots = Roots::new();
    roots.user("rust-tools", RUST_TOOLS, "(module)");
    let mut manager = roots.open(Grants::default());
    allow(&mut manager, 0);
    roots.user("rust-tools", RUST_TOOLS, "(module) ;; rebuilt");
    manager.rescan();
    let state = manager.row(0).unwrap().state;
    assert_eq!(
        state,
        State::NeedsApproval {
            added: vec![],
            removed: vec![]
        }
    );
}

#[test]
fn a_wider_request_asks_for_the_new_line_only() {
    let roots = Roots::new();
    roots.user(
        "demo",
        &small("demo", "[capabilities.ide]\nbuffer = true\n"),
        "(module)",
    );
    let mut manager = roots.open(Grants::default());
    allow(&mut manager, 0);
    let wider = small("demo", "[capabilities.ide]\nbuffer = true\ngit = true\n");
    roots.user("demo", &wider, "(module)");
    manager.rescan();
    let State::NeedsApproval { added, .. } = manager.row(0).unwrap().state else {
        panic!()
    };
    assert_eq!(added, ["ide.git"]);
}

#[test]
fn fewer_lines_or_a_lower_model_tier_never_ask_again() {
    let granted = Grant {
        digest: "d".into(),
        lines: ["ide.git", "model:code"].map(String::from).into(),
        dev: false,
    };
    let fewer = ["model:cheap".to_owned()];
    assert_eq!(check(&fewer, "d", false, Some(&granted)), Verdict::Granted);
    assert!(matches!(
        check(&fewer, "other", false, Some(&granted)),
        Verdict::NeedsApproval { .. }
    ));
    // A linked development package rebuilds into a new digest every time.
    let dev = Grant {
        dev: true,
        ..granted
    };
    assert_eq!(check(&fewer, "other", true, Some(&dev)), Verdict::Granted);
}

#[test]
fn a_prompt_answered_for_old_bytes_grants_nothing() {
    let roots = Roots::new();
    roots.user("demo", &small("demo", ""), "(module)");
    let mut manager = roots.open(Grants::default());
    let shown = manager.row(0).unwrap().digest;
    roots.user("demo", &small("demo", ""), "(module) ;; swapped");
    manager.rescan();
    assert_eq!(
        manager.decide(0, &shown, Decision::Allow),
        Err(Error::StaleDigest("demo".into()))
    );
    assert!(manager.grants().get(&Scope::User, "demo").is_none());
}

#[test]
fn a_project_plugin_waits_for_a_per_repository_enable() {
    let roots = Roots::new();
    roots.project(
        "lint-rules",
        &small("lint-rules", "[capabilities.ide]\nbuffer = true\n"),
    );
    let mut manager = roots.open(Grants::default());
    let row = manager.row(0).unwrap();
    assert_eq!(row.source, Source::Project);
    assert_eq!(row.state, State::ProjectNotEnabled);
    assert_eq!(row.lines, ["ide.buffer"]);
    allow(&mut manager, 0);
    assert_eq!(manager.row(0).unwrap().state, State::Enabled);

    // The same plugin in another clone is another repository's content.
    let other = Roots::new();
    other.project(
        "lint-rules",
        &small("lint-rules", "[capabilities.ide]\nbuffer = true\n"),
    );
    let elsewhere = other.open(manager.grants().clone());
    assert_eq!(elsewhere.row(0).unwrap().state, State::ProjectNotEnabled);
}

#[test]
fn a_lockfile_mismatch_stays_off_until_reviewed() {
    let roots = Roots::new();
    roots.user("git-blame", &small("git-blame", ""), "(module)");
    let mut manager = roots.open(Grants::default());
    allow(&mut manager, 0);
    let mut lock = kerf_plugin_lock::Lockfile::new();
    let pinned = kerf_plugin_lock::Digest::parse(&"0".repeat(64)).unwrap();
    let entry = LockEntry {
        version: "1.0.0".into(),
        digest: pinned,
    };
    lock.pin("git-blame", entry.clone());
    lock.write(&roots.project.path().join(kerf_plugin_lock::FILE_NAME))
        .unwrap();
    manager.rescan();
    let row = manager.row(0).unwrap();
    assert_eq!(row.lock, Some(entry));
    assert!(
        matches!(&row.state, State::LockMismatch(r) if r.contains("digest")),
        "{:?}",
        row.state
    );
    assert!(!gate_lets_load(&roots, &manager));
    assert_eq!(
        manager.decide(0, &row.digest, Decision::Allow),
        Err(Error::NotAskable("git-blame".into()))
    );

    manager.accept_lock(0).unwrap();
    assert_eq!(manager.row(0).unwrap().state, State::Enabled);
    manager.rescan();
    assert_eq!(
        manager.row(0).unwrap().lock.unwrap().digest.as_str(),
        row.digest
    );
}

#[test]
fn an_unreadable_lockfile_keeps_every_plugin_off() {
    let roots = Roots::new();
    roots.user("demo", &small("demo", ""), "(module)");
    fs::write(
        roots.project.path().join(kerf_plugin_lock::FILE_NAME),
        "format = 9\n",
    )
    .unwrap();
    let manager = roots.open(Grants::default());
    assert!(matches!(
        manager.row(0).unwrap().state,
        State::LockMismatch(_)
    ));
}

#[test]
fn an_invalid_manifest_cannot_be_granted() {
    let roots = Roots::new();
    roots.user("bad", &small("bad", "wasi = true\n"), "(module)");
    let many: Vec<String> = (0..=MAX_GRANT_LINES)
        .map(|i| format!("\"h{i}.example\""))
        .collect();
    let net = format!("[capabilities]\nnet = [{}]\n", many.join(", "));
    roots.user("greedy", &small("greedy", &net), "(module)");
    let mut manager = roots.open(Grants::default());
    for id in ["bad", "greedy"] {
        let i = index(&manager, id);
        let row = manager.row(i).unwrap();
        let State::Invalid(reason) = &row.state else {
            panic!("{id}: {:?}", row.state)
        };
        let why = if id == "bad" { "WASI" } else { "more than 256" };
        assert!(reason.contains(why), "{reason}");
        assert_eq!(
            manager.decide(i, &row.digest, Decision::Allow),
            Err(Error::NotAskable(id.into()))
        );
    }
}

#[test]
fn three_failures_in_a_row_turn_one_export_off_for_the_session() {
    let roots = Roots::new();
    roots.user(
        "markdown-preview",
        &small("markdown-preview", ""),
        "(module)",
    );
    let mut manager = roots.open(Grants::default());
    allow(&mut manager, 0);
    let id = "markdown-preview";
    assert!(!manager.record_call(id, "export", false));
    assert!(!manager.record_call(id, "export", false));
    assert!(!manager.record_call(id, "export", true));
    assert!(!manager.record_call(id, "export", false));
    assert!(!manager.record_call(id, "export", false));
    assert!(manager.record_call(id, "export", false));
    assert!(manager.record_call(id, "export", true), "stays off");
    assert!(!manager.record_call(id, "render", true));
    let state = manager.row(0).unwrap().state;
    assert_eq!(state, State::ExportsDisabled(vec!["export".into()]));
}

#[test]
fn the_gate_loads_only_granted_packages() {
    let roots = Roots::new();
    roots.user("demo", &small("demo", ""), "(module)");
    let mut manager = roots.open(Grants::default());
    assert!(!gate_lets_load(&roots, &manager));
    allow(&mut manager, 0);
    assert!(gate_lets_load(&roots, &manager));
}

/// Whether `kerf-plugin`'s runtime, under the manager's gate, gets past the
/// gate for the first plugin. Past it, the empty test module then fails to
/// load, which is a different error than a refusal.
fn gate_lets_load(roots: &Roots, manager: &Manager) -> bool {
    let mut plugins = Plugins::discover(Config {
        home: roots.home.path().to_path_buf(),
        project_root: Some(roots.project.path().to_path_buf()),
        session_id: "s".into(),
    })
    .with_gate(manager.gate());
    let id = plugins.ids().next().unwrap().to_owned();
    !matches!(plugins.activate(&id), Err(HostError::Refused { .. }))
}

#[test]
fn the_prompt_escapes_characters_that_hide_text() {
    assert_eq!(
        display("lsp:x run\u{202e}fdp.exe"),
        "lsp:x run\\u{202e}fdp.exe"
    );
    assert_eq!(display("a\nb\u{1b}[2J"), "a\\u{a}b\\u{1b}[2J");
    assert_eq!(display("ide.terminal"), "ide.terminal");
}
