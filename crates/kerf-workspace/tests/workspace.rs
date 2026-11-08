// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The Welcome screen, Workspace Trust sheet and restricted banner end to
//! end over temporary config, plugin and project folders.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};

use kerf_workspace::{Chip, Config, ItemKind, MAX_OPEN_FOLDERS, Mode, WorkspaceError, Workspaces};
use tempfile::TempDir;

struct Env {
    dir: TempDir,
}

impl Env {
    /// A config folder, a user plugin with a language server, and a project
    /// with a project plugin that declares a debug adapter.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("home/plugins/rust-tools");
        write_plugin(
            &user.join("versions/0123456789ab"),
            "rust-tools",
            "[[language_servers]]\nname = \"ra\"\nlanguages = [\"rust\"]\ncommand = \"rust-analyzer\"\nargs = [\"--log-file\", \"/tmp/ra log\"]\n",
        );
        fs::write(user.join("current"), "0123456789ab").unwrap();
        write_plugin(
            &dir.path().join("project/.kerf/plugins/local-dbg"),
            "local-dbg",
            "[[debug_adapters]]\nname = \"dbg\"\nlanguages = [\"rust\"]\ncommand = \"bin/dbg\"\nargs = [\"--port\", \"0\"]\n",
        );
        fs::create_dir_all(dir.path().join("project/src")).unwrap();
        Self { dir }
    }

    fn path(&self, sub: &str) -> PathBuf {
        self.dir.path().join(sub)
    }

    fn load(&self) -> Workspaces {
        Workspaces::load(Config::in_dir(&self.path("config"), self.path("home")))
    }
}

fn write_plugin(dir: &Path, id: &str, tables: &str) {
    fs::create_dir_all(dir).unwrap();
    let head = format!("api = 1\nid = \"{id}\"\nversion = \"0.1.0\"\nname = \"{id}\"\n");
    fs::write(dir.join("plugin.toml"), format!("{head}{tables}")).unwrap();
}

#[test]
fn an_untrusted_folder_opens_restricted_with_the_prompt_from_manifests() {
    let env = Env::new();
    let mut ws = env.load();

    let folder = ws.open(&env.path("project")).unwrap();
    assert_eq!(folder.mode, Mode::Restricted);
    assert!(folder.asking);
    let items: Vec<_> = folder
        .prompt
        .items
        .iter()
        .map(|i| {
            (
                i.kind,
                i.plugin.as_deref(),
                i.command.as_deref(),
                i.args.clone(),
            )
        })
        .collect();
    assert_eq!(
        items,
        [
            (ItemKind::ProjectPlugin, Some("local-dbg"), None, vec![]),
            (
                ItemKind::DebugAdapter,
                Some("local-dbg"),
                Some("bin/dbg"),
                vec!["--port".to_owned(), "0".to_owned()]
            ),
            (
                ItemKind::LanguageServer,
                Some("rust-tools"),
                Some("rust-analyzer"),
                vec!["--log-file".to_owned(), "/tmp/ra log".to_owned()]
            ),
        ]
    );

    let root = folder.root.clone();
    assert!(ws.stay_restricted(&root));
    let folder = &ws.folders()[0];
    assert_eq!((folder.mode, folder.asking), (Mode::Restricted, false));
    assert_eq!(ws.recent()[0].chip, Chip::Restricted);
}

#[test]
fn trusting_turns_the_open_folder_trusted_and_covers_subfolders() {
    let env = Env::new();
    let mut ws = env.load();
    ws.open(&env.path("project")).unwrap();

    let root = ws.trust(&env.path("project")).unwrap();
    assert_eq!(ws.folders()[0].mode, Mode::Trusted);
    assert!(!ws.folders()[0].asking);

    let sub = ws.open(&env.path("project/src")).unwrap().clone();
    assert_eq!(sub.mode, Mode::Trusted);
    assert!(sub.root.starts_with(&root));
    assert_eq!(ws.recent()[0].path, sub.root);
}

#[test]
fn a_remembered_folder_opens_trusted_in_the_next_session() {
    let env = Env::new();
    let mut ws = env.load();
    ws.open(&env.path("project")).unwrap();
    ws.trust(&env.path("project")).unwrap();
    ws.remember_remote("build-box", Path::new("/srv/app"))
        .unwrap();
    drop(ws);

    let mut ws = env.load();
    let chips: Vec<Chip> = ws.recent().iter().map(|r| r.chip).collect();
    assert_eq!(chips, [Chip::Remote, Chip::Trusted]);
    let folder = ws.open(&env.path("project")).unwrap();
    assert_eq!((folder.mode, folder.asking), (Mode::Trusted, false));
    assert_eq!(ws.notices().count(), 0);
}

#[test]
fn revoke_reports_what_to_stop_and_persists() {
    let env = Env::new();
    let mut ws = env.load();
    ws.trust(&env.path("project")).unwrap();
    ws.open(&env.path("project")).unwrap();

    let revoked = ws.revoke(&env.path("project"));
    assert_eq!(revoked.roots.len(), 1);
    assert_eq!(revoked.stop.len(), 1);
    assert_eq!(revoked.stop[0].items, ws.folders()[0].prompt.items);
    assert_eq!(revoked.stop[0].items.len(), 3);
    let folder = &ws.folders()[0];
    assert_eq!((folder.mode, folder.asking), (Mode::Restricted, false));

    assert!(ws.revoke(&env.path("project")).stop.is_empty());
    let mut ws = env.load();
    assert_eq!(
        ws.open(&env.path("project")).unwrap().mode,
        Mode::Restricted
    );
}

#[test]
fn bad_input_is_refused_and_damaged_files_are_notices() {
    let env = Env::new();
    fs::create_dir_all(env.path("config")).unwrap();
    fs::write(env.path("config/trust.toml"), "trust_all = true\n").unwrap();
    let mut ws = env.load();
    assert_eq!(ws.notices().count(), 1);

    assert!(matches!(
        ws.open(&env.path("missing")),
        Err(WorkspaceError::Trust(_))
    ));
    assert!(matches!(
        ws.remember_remote("a host", Path::new("/srv")),
        Err(WorkspaceError::InvalidHost(_))
    ));
    for i in 0..MAX_OPEN_FOLDERS {
        let sub = env.path(&format!("many/{i}"));
        fs::create_dir_all(&sub).unwrap();
        ws.open(&sub).unwrap();
    }
    assert!(matches!(
        ws.open(&env.path("project")),
        Err(WorkspaceError::TooManyFolders { .. })
    ));
    assert!(ws.close(&env.path("many/0")));
    ws.open(&env.path("project")).unwrap();
    assert!(ws.forget(&env.path("project")));
    assert!(!ws.forget(&env.path("project")));
}
