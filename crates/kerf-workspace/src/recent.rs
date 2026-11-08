// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The recent folders file: the only code that reads or writes it, and the
//! type its committed JSON Schema (`schema/recent.schema.json`) is generated
//! from. It sits next to the trust file in the user's config directory: a
//! file inside a repository could plant entries on the Welcome screen.

use std::io;
use std::path::{Path, PathBuf};

use kerf_core::ReadError;
use serde::{Deserialize, Serialize};

/// Most folders the Welcome screen remembers; the oldest falls off.
pub const MAX_RECENT: usize = 32;
/// Largest recent file read. Room for [`MAX_RECENT`] long paths.
pub const MAX_RECENT_FILE_BYTES: u64 = 256 * 1024;
/// Longest remote host name, the DNS limit.
pub const MAX_HOST_BYTES: usize = 253;

/// Kerf's recent folders (`recent.toml` in the user's config directory).
/// Written by Kerf whenever a folder is opened or forgotten.
#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RecentFile {
    /// Most recent first.
    #[serde(default)]
    folders: Vec<Entry>,
}

/// One remembered folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(crate) struct Entry {
    /// The folder: absolute and canonical for a local one, as the remote
    /// host spells it for a remote one.
    pub(crate) path: PathBuf,
    /// The remote host; absent for a local folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) host: Option<String>,
}

impl Entry {
    /// A local path that is relative would resolve against whatever folder
    /// Kerf runs in; a host with spaces or controls is not one a connect
    /// dialog wrote.
    pub(crate) fn is_valid(&self) -> bool {
        match &self.host {
            None => self.path.is_absolute(),
            Some(host) => {
                !host.is_empty()
                    && host.len() <= MAX_HOST_BYTES
                    && !host.chars().any(|c| c.is_whitespace() || c.is_control())
            }
        }
    }
}

/// Why the recent file could not be read or written. Every variant names
/// the file.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecentFileError {
    /// Reading, creating or replacing the file failed.
    #[error("cannot access recent folders file {}: {error}", path.display())]
    Io {
        /// The recent file.
        path: PathBuf,
        /// What the OS answered.
        error: io::ErrorKind,
    },
    /// The file is larger than [`MAX_RECENT_FILE_BYTES`].
    #[error("recent folders file {} is larger than {cap} bytes", path.display())]
    TooLarge {
        /// The recent file.
        path: PathBuf,
        /// The cap that was hit.
        cap: u64,
    },
    /// The file is not valid TOML of the recent file's shape.
    #[error("recent folders file {} is invalid: {message}", path.display())]
    Parse {
        /// The recent file.
        path: PathBuf,
        /// The parser's description.
        message: String,
    },
    /// A path cannot be written as TOML text (a non-UTF-8 path).
    #[error("cannot encode recent folders file {}: {message}", path.display())]
    Encode {
        /// The recent file.
        path: PathBuf,
        /// The encoder's description.
        message: String,
    },
}

/// The remembered folders, most recent first, at most [`MAX_RECENT`].
#[derive(Debug, Default)]
pub(crate) struct Recent {
    entries: Vec<Entry>,
}

impl Recent {
    /// A missing file is an empty list. Invalid entries are dropped rather
    /// than failing the whole list, and so are entries past the cap.
    pub(crate) fn load(path: &Path) -> Result<Self, RecentFileError> {
        let text = match kerf_core::read_text(path, MAX_RECENT_FILE_BYTES) {
            Ok(text) => text,
            Err(ReadError::Io(e)) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(ReadError::Io(e)) => return Err(io_error(path, e.kind())),
            Err(ReadError::TooLarge { cap }) => {
                return Err(RecentFileError::TooLarge {
                    path: path.to_path_buf(),
                    cap,
                });
            }
            Err(ReadError::NotUtf8) => return Err(io_error(path, io::ErrorKind::InvalidData)),
        };
        let file: RecentFile = toml::from_str(&text).map_err(|e| RecentFileError::Parse {
            path: path.to_path_buf(),
            message: e.message().to_owned(),
        })?;
        let entries = file
            .folders
            .into_iter()
            .filter(Entry::is_valid)
            .take(MAX_RECENT)
            .collect();
        Ok(Self { entries })
    }

    /// Replaces the file atomically, creating its directory.
    pub(crate) fn save(&self, path: &Path) -> Result<(), RecentFileError> {
        let file = RecentFile {
            folders: self.entries.clone(),
        };
        let text = toml::to_string(&file).map_err(|e| RecentFileError::Encode {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        fs_kit::write::Options::new()
            .create_parent(true)
            .write(path, text)
            .map_err(|e| io_error(path, e.kind()))
    }

    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Moves `entry` to the front, dropping the oldest past the cap.
    pub(crate) fn touch(&mut self, entry: Entry) {
        self.entries.retain(|e| *e != entry);
        self.entries.insert(0, entry);
        self.entries.truncate(MAX_RECENT);
    }

    /// Removes the local folder `path`; true when it was listed.
    pub(crate) fn forget(&mut self, path: &Path) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|e| !(e.host.is_none() && e.path == path));
        self.entries.len() != before
    }
}

fn io_error(path: &Path, error: io::ErrorKind) -> RecentFileError {
    RecentFileError::Io {
        path: path.to_path_buf(),
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Under the temp folder so the path is absolute on Windows too.
    fn local(name: &str) -> Entry {
        Entry {
            path: std::env::temp_dir().join(name),
            host: None,
        }
    }

    #[test]
    fn invalid_and_surplus_entries_are_dropped_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("recent.toml");
        let mut text = String::from(
            "[[folders]]\npath = \"relative\"\n[[folders]]\npath = \"/srv\"\nhost = \"a b\"\n",
        );
        for i in 0..=MAX_RECENT {
            let path = local(&format!("f{i}")).path;
            text.push_str(&format!("[[folders]]\npath = {path:?}\n"));
        }
        std::fs::write(&file, text).unwrap();

        let recent = Recent::load(&file).unwrap();
        assert_eq!(recent.entries().len(), MAX_RECENT);
        assert_eq!(recent.entries()[0], local("f0"));
    }

    #[test]
    fn unknown_keys_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("recent.toml");
        std::fs::write(&file, "trusted = true\n").unwrap();
        assert!(matches!(
            Recent::load(&file),
            Err(RecentFileError::Parse { .. })
        ));
    }

    #[test]
    fn touch_moves_to_the_front_and_caps() {
        let mut recent = Recent::default();
        for i in 0..MAX_RECENT + 2 {
            recent.touch(local(&format!("f{i}")));
        }
        recent.touch(local("f5"));
        assert_eq!(recent.entries().len(), MAX_RECENT);
        assert_eq!(recent.entries()[0], local("f5"));
        assert!(recent.forget(&local("f5").path));
        assert!(!recent.forget(&local("f5").path));
    }

    #[test]
    fn the_committed_schema_is_current() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/recent.schema.json");
        config_schema::check_schema::<RecentFile>(
            &path,
            Some("Generated from crates/kerf-workspace. Regenerate with KERF_BLESS_SCHEMA=1 cargo test -p kerf-workspace."),
            "KERF_BLESS_SCHEMA",
        )
        .unwrap();
    }
}
