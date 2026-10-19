// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The trust file: the only code that reads or writes it, and the type its
//! committed JSON Schema (`crates/kerf-core/schema/trust.schema.json`) is
//! generated from.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{MAX_TRUSTED_ROOTS, TrustStore};
use crate::ReadError;

/// Largest trust file read or written. Room for [`MAX_TRUSTED_ROOTS`] long
/// paths; anything bigger was not written by Kerf and is refused unread.
pub const MAX_TRUST_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Kerf's Workspace Trust file (`trust.toml` in the user's config directory).
/// Written by Kerf when the user trusts or revokes a folder.
#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct TrustFile {
    /// Absolute folders the user trusts; each covers its subfolders.
    #[serde(default)]
    roots: Vec<PathBuf>,
}

/// Why the trust file could not be read or written. Every variant names the
/// file.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TrustFileError {
    /// Reading, creating or replacing the file failed.
    #[error("cannot access trust file {}: {error}", path.display())]
    Io {
        /// The trust file.
        path: PathBuf,
        /// What the OS answered.
        error: io::ErrorKind,
    },
    /// The file is larger than [`MAX_TRUST_FILE_BYTES`].
    #[error("trust file {} is larger than {cap} bytes", path.display())]
    TooLarge {
        /// The trust file.
        path: PathBuf,
        /// The cap that was hit.
        cap: u64,
    },
    /// The file is not valid TOML of the trust file's shape.
    #[error("trust file {} is invalid: {message}", path.display())]
    Parse {
        /// The trust file.
        path: PathBuf,
        /// The parser's description.
        message: String,
    },
    /// A root cannot be written as TOML text (a non-UTF-8 path).
    #[error("cannot encode trust file {}: {message}", path.display())]
    Encode {
        /// The trust file.
        path: PathBuf,
        /// The encoder's description.
        message: String,
    },
    /// A root is relative. It would resolve against whatever folder Kerf runs
    /// in, which may be the repository itself.
    #[error("trust file {} lists relative root {}", path.display(), root.display())]
    RelativeRoot {
        /// The trust file.
        path: PathBuf,
        /// The rejected entry.
        root: PathBuf,
    },
    /// The file lists more than [`MAX_TRUSTED_ROOTS`] roots.
    #[error("trust file {} lists more than {cap} roots", path.display())]
    TooManyRoots {
        /// The trust file.
        path: PathBuf,
        /// The cap that was hit.
        cap: usize,
    },
}

impl TrustStore {
    /// `trust.toml` under `kerf` in the OS config directory (`dirs::config_dir`:
    /// `~/Library/Application Support` on macOS, `%APPDATA%` on Windows,
    /// `$XDG_CONFIG_HOME` or `~/.config` on Linux). `None` when the OS reports
    /// no such directory; the caller then keeps trust in memory.
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("kerf").join("trust.toml"))
    }

    /// Reads the store from `path`. A missing file is an empty store, which is
    /// how a fresh install starts.
    ///
    /// On an error the caller should start from [`TrustStore::new`] and tell
    /// the user: untrusted is the safe answer for a damaged file.
    pub fn load(path: &Path) -> Result<Self, TrustFileError> {
        let Some(text) = read_capped(path)? else {
            return Ok(Self::new());
        };
        let file: TrustFile = toml::from_str(&text).map_err(|e| TrustFileError::Parse {
            path: path.to_path_buf(),
            message: e.message().to_owned(),
        })?;
        if file.roots.len() > MAX_TRUSTED_ROOTS {
            return Err(TrustFileError::TooManyRoots {
                path: path.to_path_buf(),
                cap: MAX_TRUSTED_ROOTS,
            });
        }
        if let Some(root) = file.roots.iter().find(|root| !root.is_absolute()) {
            return Err(TrustFileError::RelativeRoot {
                path: path.to_path_buf(),
                root: root.clone(),
            });
        }
        // Not canonicalized here: a root whose folder is gone must still load
        // so the user can revoke it, and a cwd only ever matches canonically.
        Ok(Self {
            roots: file.roots.into_iter().collect(),
        })
    }

    /// Replaces the file at `path` with the store, creating its directory.
    /// The write is atomic, so a crash leaves the old list or the new one.
    pub fn save(&self, path: &Path) -> Result<(), TrustFileError> {
        let file = TrustFile {
            roots: self.roots.iter().cloned().collect(),
        };
        let text = toml::to_string(&file).map_err(|e| TrustFileError::Encode {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        // `load` must read back whatever `save` wrote.
        if text.len() as u64 > MAX_TRUST_FILE_BYTES {
            return Err(TrustFileError::TooLarge {
                path: path.to_path_buf(),
                cap: MAX_TRUST_FILE_BYTES,
            });
        }
        fs_kit::write::Options::new()
            .create_parent(true)
            .write(path, text)
            .map_err(|e| io_error(path, &e))
    }
}

fn read_capped(path: &Path) -> Result<Option<String>, TrustFileError> {
    match crate::read_text(path, MAX_TRUST_FILE_BYTES) {
        Ok(text) => Ok(Some(text)),
        Err(ReadError::Io(e)) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(ReadError::Io(e)) => Err(io_error(path, &e)),
        Err(ReadError::TooLarge { cap }) => Err(TrustFileError::TooLarge {
            path: path.to_path_buf(),
            cap,
        }),
        Err(ReadError::NotUtf8) => Err(io_error(path, &io::ErrorKind::InvalidData.into())),
    }
}

fn io_error(path: &Path, e: &io::Error) -> TrustFileError {
    TrustFileError::Io {
        path: path.to_path_buf(),
        error: e.kind(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust::{SpawnKind, TrustError};

    const SCHEMA: &str = include_str!("../../schema/trust.schema.json");

    #[test]
    fn trust_survives_a_save_and_load_and_so_does_a_revoke() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        let file = dir.path().join("config/kerf/trust.toml");

        let mut store = TrustStore::new();
        store.grant(&repo).unwrap();
        store.save(&file).unwrap();

        let mut reloaded = TrustStore::load(&file).unwrap();
        let permit = reloaded
            .authorize(&repo, SpawnKind::LanguageServer)
            .unwrap();
        assert_eq!(permit.cwd(), fs_kit::path::canonicalize(&repo).unwrap());

        reloaded.revoke(&repo);
        reloaded.save(&file).unwrap();
        assert!(matches!(
            TrustStore::load(&file)
                .unwrap()
                .authorize(&repo, SpawnKind::LanguageServer),
            Err(TrustError::Untrusted { .. })
        ));
    }

    #[test]
    fn a_missing_file_is_an_empty_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = TrustStore::load(&dir.path().join("trust.toml")).unwrap();
        assert_eq!(store.roots().count(), 0);
    }

    #[test]
    fn a_relative_root_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("trust.toml");
        std::fs::write(&file, "roots = [\".\"]\n").unwrap();

        assert!(matches!(
            TrustStore::load(&file),
            Err(TrustFileError::RelativeRoot { root, .. }) if root == Path::new(".")
        ));
    }

    #[test]
    fn unknown_keys_and_bad_toml_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("trust.toml");
        for text in ["trust_all = true\n", "roots = \n"] {
            std::fs::write(&file, text).unwrap();
            assert!(
                matches!(TrustStore::load(&file), Err(TrustFileError::Parse { .. })),
                "{text:?}"
            );
        }
    }

    #[test]
    fn an_oversized_file_is_refused_unread() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("trust.toml");
        let size = usize::try_from(MAX_TRUST_FILE_BYTES).unwrap() + 1;
        std::fs::write(&file, " ".repeat(size)).unwrap();

        assert_eq!(
            TrustStore::load(&file).unwrap_err(),
            TrustFileError::TooLarge {
                path: file,
                cap: MAX_TRUST_FILE_BYTES
            }
        );
    }

    #[test]
    fn a_file_over_the_root_cap_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("trust.toml");
        let roots: Vec<String> = (0..=MAX_TRUSTED_ROOTS)
            .map(|i| format!("{:?}", dir.path().join(i.to_string())))
            .collect();
        std::fs::write(&file, format!("roots = [{}]\n", roots.join(", "))).unwrap();

        assert!(matches!(
            TrustStore::load(&file),
            Err(TrustFileError::TooManyRoots {
                cap: MAX_TRUSTED_ROOTS,
                ..
            })
        ));
    }

    #[test]
    fn the_default_path_is_kerf_trust_toml() {
        if let Some(path) = TrustStore::default_path() {
            assert!(path.ends_with(Path::new("kerf").join("trust.toml")));
        }
    }

    #[test]
    fn the_committed_schema_is_current() {
        let fresh = format!(
            "{}\n",
            serde_json::to_string_pretty(&schemars::schema_for!(TrustFile)).unwrap()
        );
        if std::env::var_os("KERF_BLESS").is_some() {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/schema/trust.schema.json");
            std::fs::write(path, &fresh).unwrap();
            return;
        }
        // A Windows checkout with `core.autocrlf` turns the committed `\n`
        // into `\r\n`; that is not staleness.
        assert_eq!(
            SCHEMA.replace("\r\n", "\n"),
            fresh,
            "schema/trust.schema.json is stale; rerun with KERF_BLESS=1"
        );
    }
}
