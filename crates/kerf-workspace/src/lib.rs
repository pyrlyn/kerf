// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The logic behind Kerf's Welcome screen, Workspace Trust sheet and
//! restricted-mode banner, without any UI: [`Workspaces`] answers plain
//! snapshots that every native host draws.
//!
//! Trust itself is `kerf-core`'s [`TrustStore`]; this crate only decides
//! what to show and when to save. Opening a folder never trusts it and never
//! starts anything (`docs/architecture.md` §6). An untrusted folder gets a
//! [`Prompt`] listing what would start; the user either trusts it or stays
//! restricted. A revoke answers what must stop.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod prompt;
mod recent;

pub use prompt::{Item, ItemKind, MAX_PROMPT_ITEMS, Prompt, Task};
pub use recent::{MAX_HOST_BYTES, MAX_RECENT, MAX_RECENT_FILE_BYTES, RecentFileError};

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use kerf_core::{SpawnKind, TrustError, TrustStore};
use kerf_plugin::{MAX_NOTICE_BYTES, MAX_NOTICES, MAX_PLUGINS};
use serde::Serialize;
use wasm_plugin_host::{State, discover::discover};

use recent::{Entry, Recent};

/// Most folders open at once; each holds its own prompt.
pub const MAX_OPEN_FOLDERS: usize = 16;

/// Where the workspace model keeps its files and finds user plugins.
#[derive(Debug, Clone)]
pub struct Config {
    /// `kerf-core`'s trust file.
    pub trust_file: PathBuf,
    /// The recent folders file.
    pub recent_file: PathBuf,
    /// Kerf's home; user plugins live in `<plugin_home>/plugins/`.
    pub plugin_home: PathBuf,
}

impl Config {
    /// `trust.toml` and `recent.toml` in `dir`.
    pub fn in_dir(dir: &Path, plugin_home: PathBuf) -> Self {
        Self {
            trust_file: dir.join("trust.toml"),
            recent_file: dir.join("recent.toml"),
            plugin_home,
        }
    }

    /// The directory `kerf-core` keeps the trust file in, so both files sit
    /// together outside every repository. `None` when the OS has no config
    /// directory.
    pub fn default_dir() -> Option<PathBuf> {
        TrustStore::default_path()?.parent().map(Path::to_path_buf)
    }
}

/// The chip next to a recent folder on the Welcome screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Chip {
    /// Inside a trusted root.
    Trusted,
    /// Not trusted, or gone from disk.
    Restricted,
    /// On a remote host; trust is decided there.
    Remote,
}

/// A recent folder as the Welcome screen lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecentFolder {
    /// The folder.
    pub path: PathBuf,
    /// The remote host, for a remote folder.
    pub host: Option<String>,
    /// Its trust chip.
    pub chip: Chip,
}

/// Whether an open folder may start processes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Language servers, debug adapters and tasks start on their triggers.
    Trusted,
    /// Nothing starts.
    Restricted,
}

/// An open folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Folder {
    /// The canonical folder.
    pub root: PathBuf,
    /// Whether it may start processes.
    pub mode: Mode,
    /// True while the Workspace Trust sheet should be up. False once the
    /// user stays restricted, which shows the restricted-mode banner.
    pub asking: bool,
    /// What starts while the folder is trusted.
    pub prompt: Prompt,
}

/// What one open folder must stop after a revoke.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stop {
    /// The open folder that lost trust.
    pub folder: PathBuf,
    /// Its processes and project plugins.
    pub items: Vec<Item>,
}

/// The answer to [`Workspaces::revoke`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Revoked {
    /// The trusted roots removed.
    pub roots: Vec<PathBuf>,
    /// The open folders that lost trust, for the supervisor to stop.
    pub stop: Vec<Stop>,
}

/// Everything the Welcome screen and the open windows show.
#[derive(Debug, Serialize)]
pub struct Snapshot<'a> {
    /// Most recent first.
    pub recent: Vec<RecentFolder>,
    /// Open folders, in the order they were opened.
    pub folders: &'a [Folder],
    /// Problems to tell the user, oldest first.
    pub notices: &'a VecDeque<String>,
}

/// Why a workspace action failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkspaceError {
    /// The trust store refused: a missing folder, or too many roots.
    #[error(transparent)]
    Trust(#[from] TrustError),
    /// [`MAX_OPEN_FOLDERS`] folders are already open.
    #[error("at most {cap} folders can be open")]
    TooManyFolders {
        /// The cap that was hit.
        cap: usize,
    },
    /// A remote host name that is empty, too long or has spaces.
    #[error("invalid remote host {0:?}")]
    InvalidHost(String),
}

/// Recent folders, trust and open folders for one Kerf process.
#[derive(Debug)]
pub struct Workspaces {
    config: Config,
    trust: TrustStore,
    recent: Recent,
    folders: Vec<Folder>,
    notices: VecDeque<String>,
}

impl Workspaces {
    /// Reads the trust and recent files. A damaged file is a notice and an
    /// empty list, never an error: untrusted is the safe answer.
    pub fn load(config: Config) -> Self {
        let mut notices = Vec::new();
        let trust = TrustStore::load(&config.trust_file).unwrap_or_else(|e| {
            notices.push(e.to_string());
            TrustStore::new()
        });
        let recent = Recent::load(&config.recent_file).unwrap_or_else(|e| {
            notices.push(e.to_string());
            Recent::default()
        });
        let mut workspaces = Self {
            config,
            trust,
            recent,
            folders: Vec::new(),
            notices: VecDeque::new(),
        };
        for notice in notices {
            workspaces.notice(notice);
        }
        workspaces
    }

    /// Opens `path` and remembers it. A trusted folder opens trusted; any
    /// other opens restricted and asking, with the prompt read from the
    /// user's and the folder's plugin manifests. Opening an open folder
    /// answers it again.
    pub fn open(&mut self, path: &Path) -> Result<&Folder, WorkspaceError> {
        // `authorize` is the one trust check, and it answers the canonical
        // folder either way, so nothing here can canonicalize differently.
        let (root, mode) = match self.trust.authorize(path, SpawnKind::Task) {
            Ok(permit) => (permit.cwd().to_path_buf(), Mode::Trusted),
            Err(TrustError::Untrusted { path, .. }) => (path, Mode::Restricted),
            Err(e) => return Err(e.into()),
        };
        let index = match self.folders.iter().position(|f| f.root == root) {
            Some(index) => index,
            None if self.folders.len() >= MAX_OPEN_FOLDERS => {
                return Err(WorkspaceError::TooManyFolders {
                    cap: MAX_OPEN_FOLDERS,
                });
            }
            None => {
                let prompt = self.read_prompt(&root);
                self.folders.push(Folder {
                    root: root.clone(),
                    mode,
                    asking: mode == Mode::Restricted,
                    prompt,
                });
                self.folders.len() - 1
            }
        };
        self.remember(Entry {
            path: root,
            host: None,
        });
        Ok(&self.folders[index])
    }

    /// The user's "Trust folder": trusts `path` and every open folder inside
    /// it; answers the canonical root.
    pub fn trust(&mut self, path: &Path) -> Result<PathBuf, WorkspaceError> {
        let root = self.trust.grant(path)?;
        for folder in &mut self.folders {
            if self.trust.is_trusted(&folder.root) {
                folder.mode = Mode::Trusted;
                folder.asking = false;
            }
        }
        self.save_trust();
        Ok(root)
    }

    /// The user's "Stay restricted": the sheet closes and the banner shows.
    /// False when `path` is not open.
    pub fn stay_restricted(&mut self, path: &Path) -> bool {
        let root = canonical_or_given(path);
        self.folders
            .iter_mut()
            .find(|f| f.root == root)
            .map(|f| f.asking = false)
            .is_some()
    }

    /// Revokes `path` and every trusted root inside it. Open folders that
    /// lose trust turn restricted without asking again, and the answer
    /// names what each must stop (§5.3).
    pub fn revoke(&mut self, path: &Path) -> Revoked {
        let roots = self.trust.revoke(path);
        let mut stop = Vec::new();
        for folder in &mut self.folders {
            if folder.mode == Mode::Trusted && !self.trust.is_trusted(&folder.root) {
                folder.mode = Mode::Restricted;
                folder.asking = false;
                stop.push(Stop {
                    folder: folder.root.clone(),
                    items: folder.prompt.items.clone(),
                });
            }
        }
        if !roots.is_empty() {
            self.save_trust();
        }
        Revoked { roots, stop }
    }

    /// Closes an open folder; false when it was not open.
    pub fn close(&mut self, path: &Path) -> bool {
        let root = canonical_or_given(path);
        let before = self.folders.len();
        self.folders.retain(|f| f.root != root);
        self.folders.len() != before
    }

    /// Removes a local folder from the recent list; false when it was not
    /// listed. A folder gone from disk can still be forgotten.
    pub fn forget(&mut self, path: &Path) -> bool {
        let forgotten = self.recent.forget(&canonical_or_given(path));
        if forgotten {
            self.save_recent();
        }
        forgotten
    }

    /// Remembers a folder on a remote host, after the remote connection
    /// opened it.
    pub fn remember_remote(&mut self, host: &str, path: &Path) -> Result<(), WorkspaceError> {
        let entry = Entry {
            path: path.to_path_buf(),
            host: Some(host.to_owned()),
        };
        if !entry.is_valid() {
            return Err(WorkspaceError::InvalidHost(host.to_owned()));
        }
        self.remember(entry);
        Ok(())
    }

    /// The recent folders with their chips, most recent first.
    pub fn recent(&self) -> Vec<RecentFolder> {
        self.recent
            .entries()
            .iter()
            .map(|e| RecentFolder {
                path: e.path.clone(),
                host: e.host.clone(),
                chip: match e.host {
                    Some(_) => Chip::Remote,
                    None if self.trust.is_trusted(&e.path) => Chip::Trusted,
                    None => Chip::Restricted,
                },
            })
            .collect()
    }

    /// The open folders, in the order they were opened.
    pub fn folders(&self) -> &[Folder] {
        &self.folders
    }

    /// Problems to tell the user (a damaged file, a skipped plugin), oldest
    /// first, at most [`MAX_NOTICES`].
    pub fn notices(&self) -> impl Iterator<Item = &str> {
        self.notices.iter().map(String::as_str)
    }

    /// Everything the screens show, in one value a host can encode.
    pub fn snapshot(&self) -> Snapshot<'_> {
        Snapshot {
            recent: self.recent(),
            folders: &self.folders,
            notices: &self.notices,
        }
    }

    fn read_prompt(&mut self, root: &Path) -> Prompt {
        let found = discover::<kerf_plugin::Manifest>(
            &self.config.plugin_home,
            kerf_plugin::APP,
            Some(root),
        );
        let plugins = &found.plugins[..found.plugins.len().min(MAX_PLUGINS)];
        let prompt = Prompt::build(
            plugins.iter().filter_map(|p| match &p.state {
                State::Loaded { manifest, .. } => Some((p.source, &manifest.0)),
                State::Skipped { .. } => None,
            }),
            // No task source exists yet; the task runner feeds this.
            &[],
        );
        for p in plugins {
            if let State::Skipped { reason } = &p.state {
                self.notice(format!("plugin {:?} skipped: {reason}", p.id));
            }
        }
        for notice in found.notices {
            self.notice(notice);
        }
        prompt
    }

    fn remember(&mut self, entry: Entry) {
        self.recent.touch(entry);
        self.save_recent();
    }

    // A failed save keeps the session's state and tells the user: the next
    // launch reads the old file, which they can then fix.
    fn save_trust(&mut self) {
        if let Err(e) = self.trust.save(&self.config.trust_file) {
            self.notice(e.to_string());
        }
    }

    fn save_recent(&mut self) {
        if let Err(e) = self.recent.save(&self.config.recent_file) {
            self.notice(e.to_string());
        }
    }

    // Notices quote plugin and file text, so both their number and length
    // are capped.
    fn notice(&mut self, mut text: String) {
        text.truncate(text.floor_char_boundary(MAX_NOTICE_BYTES));
        if self.notices.len() >= MAX_NOTICES {
            self.notices.pop_front();
        }
        self.notices.push_back(text);
    }
}

// A folder gone from disk is matched as given, so it can still be closed or
// forgotten.
fn canonical_or_given(path: &Path) -> PathBuf {
    fs_kit::path::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
