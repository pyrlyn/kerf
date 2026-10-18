// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Workspace Trust (`docs/architecture.md` §6): an untrusted folder starts no
//! language server, debug adapter or task.
//!
//! The one check is [`TrustStore::authorize`]. It answers a [`SpawnPermit`]
//! that nothing else can build, so code that spawns a process takes the permit
//! as an argument and cannot skip the check. A refusal is a [`TrustError`].
//! Trust is a precondition, not a grant: the plugin's `lsp:` / `dap:` grant is
//! still checked separately (§6 rules).
//!
//! A trusted root covers every folder below it, as in VS Code and Zed. Paths
//! are canonicalized before they are compared, because a repository controls
//! its own symlinks and `..` would otherwise reach a trusted prefix lexically.
//!
//! The store persists to a user-level file in the OS config directory
//! ([`TrustStore::default_path`], decision 7 of the architecture document),
//! never to a file inside a repository: a repository is untrusted input and
//! must not grant itself trust. File I/O lives in the `file` submodule only.

mod file;

pub use file::{MAX_TRUST_FILE_BYTES, TrustFileError};

use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Most folder roots the store holds. Granting is a user action, but the
/// trust file is read back from disk, so its size is capped like any other
/// input.
pub const MAX_TRUSTED_ROOTS: usize = 1024;

/// The kind of process asking to start; named in refusals and trust prompts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpawnKind {
    /// A language server (LSP).
    LanguageServer,
    /// A debug adapter (DAP).
    DebugAdapter,
    /// A build or a task.
    Task,
}

impl fmt::Display for SpawnKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LanguageServer => "language server",
            Self::DebugAdapter => "debug adapter",
            Self::Task => "task",
        })
    }
}

/// Why trust was not granted or a spawn was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TrustError {
    /// The folder is not inside any trusted root.
    #[error("{kind} refused: {} is not in a trusted folder", path.display())]
    Untrusted {
        /// The canonical folder the process would run in.
        path: PathBuf,
        /// What asked to start.
        kind: SpawnKind,
    },
    /// The path could not be resolved to a canonical folder.
    #[error("cannot resolve {}: {error}", path.display())]
    Unresolved {
        /// The path as the caller gave it.
        path: PathBuf,
        /// Why resolving failed (`NotADirectory` for a file).
        error: io::ErrorKind,
    },
    /// The store already holds [`MAX_TRUSTED_ROOTS`] roots.
    #[error("trust store is full ({cap} roots)")]
    TooManyRoots {
        /// The cap that was hit.
        cap: usize,
    },
}

/// Proof that [`TrustStore::authorize`] passed for one spawn.
///
/// Fields are private and there is no other constructor, so a spawn function
/// that takes a `SpawnPermit` by value cannot be reached around the check. It
/// is not `Clone`: one permit, one spawn.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "a permit is only useful when handed to the spawn"]
pub struct SpawnPermit {
    root: PathBuf,
    cwd: PathBuf,
    kind: SpawnKind,
}

impl SpawnPermit {
    /// The trusted root that covers [`cwd`](Self::cwd). A supervisor keys its
    /// processes by it so a revoke can stop them (§5.3).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The canonical folder that was checked. Spawn with this as the working
    /// directory, not the caller's original path, so the process runs where
    /// the check looked.
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// What the permit was issued for.
    pub fn kind(&self) -> SpawnKind {
        self.kind
    }
}

/// The user's trusted folder roots.
#[derive(Debug, Default)]
pub struct TrustStore {
    roots: BTreeSet<PathBuf>,
}

impl TrustStore {
    /// An empty store: every folder is untrusted, which is how a freshly
    /// cloned or opened folder must start (§6).
    pub fn new() -> Self {
        Self::default()
    }

    /// Trusts `root` and everything below it; answers the canonical root.
    ///
    /// Call this only on the user's decision, never on something a repository
    /// or a plugin wrote.
    pub fn grant(&mut self, root: &Path) -> Result<PathBuf, TrustError> {
        let root = canonical_dir(root)?;
        if !self.roots.contains(&root) && self.roots.len() >= MAX_TRUSTED_ROOTS {
            return Err(TrustError::TooManyRoots {
                cap: MAX_TRUSTED_ROOTS,
            });
        }
        self.roots.insert(root.clone());
        Ok(root)
    }

    /// Removes `root` and every trusted root inside it; answers what was
    /// removed so the caller can stop the processes started under those roots
    /// (§5.3).
    ///
    /// A folder still covered by a trusted ancestor stays trusted; revoke the
    /// ancestor for that. A root that no longer exists on disk is matched as
    /// given, so the user can still remove it.
    pub fn revoke(&mut self, root: &Path) -> Vec<PathBuf> {
        let root = fs_kit::path::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let removed: Vec<PathBuf> = self
            .roots
            .iter()
            .filter(|r| fs_kit::path::path_starts_with(r, &root))
            .cloned()
            .collect();
        for r in &removed {
            self.roots.remove(r);
        }
        removed
    }

    /// Whether `path` lies inside a trusted root. An unresolvable path is
    /// untrusted.
    pub fn is_trusted(&self, path: &Path) -> bool {
        canonical_dir(path).is_ok_and(|p| self.covering_root(&p).is_some())
    }

    /// The check every language-server, debug-adapter and task spawn passes.
    ///
    /// `cwd` is the folder the process would run in (its workspace root or
    /// working directory). Answers a [`SpawnPermit`] when a trusted root
    /// covers it, [`TrustError::Untrusted`] when none does.
    pub fn authorize(&self, cwd: &Path, kind: SpawnKind) -> Result<SpawnPermit, TrustError> {
        let cwd = canonical_dir(cwd)?;
        match self.covering_root(&cwd) {
            Some(root) => Ok(SpawnPermit {
                root: root.to_path_buf(),
                cwd,
                kind,
            }),
            None => Err(TrustError::Untrusted { path: cwd, kind }),
        }
    }

    /// The trusted roots, sorted, for a settings page or a future writer.
    pub fn roots(&self) -> impl Iterator<Item = &Path> {
        self.roots.iter().map(PathBuf::as_path)
    }

    fn covering_root(&self, canonical: &Path) -> Option<&Path> {
        // Whole components, so `/a/bc` is not inside `/a/b`; ASCII case is
        // folded on Windows, where `C:\Repo` and `c:\repo` are one folder.
        self.roots()
            .find(|root| fs_kit::path::path_starts_with(canonical, root))
    }
}

fn canonical_dir(path: &Path) -> Result<PathBuf, TrustError> {
    let unresolved = |error| TrustError::Unresolved {
        path: path.to_path_buf(),
        error,
    };
    // Without the Windows `\\?\` prefix, so roots stay readable in the trust
    // file and match paths spelled either way.
    let canonical = fs_kit::path::canonicalize(path).map_err(|e| unresolved(e.kind()))?;
    if !canonical.is_dir() {
        return Err(unresolved(io::ErrorKind::NotADirectory));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tree() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        for sub in ["repo/src", "repo-evil", "other"] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        dir
    }

    #[test]
    fn spawn_is_refused_until_trusted_and_again_after_revoke() {
        let dir = tree();
        let repo = dir.path().join("repo");
        let mut store = TrustStore::new();

        let err = store
            .authorize(&repo, SpawnKind::LanguageServer)
            .unwrap_err();
        assert!(matches!(
            err,
            TrustError::Untrusted {
                kind: SpawnKind::LanguageServer,
                ..
            }
        ));

        let root = store.grant(&repo).unwrap();
        let permit = store.authorize(&repo, SpawnKind::DebugAdapter).unwrap();
        assert_eq!(permit.root(), root);
        assert_eq!(permit.cwd(), root);
        assert_eq!(permit.kind(), SpawnKind::DebugAdapter);

        assert_eq!(store.revoke(&repo), vec![root]);
        assert!(matches!(
            store.authorize(&repo, SpawnKind::Task),
            Err(TrustError::Untrusted {
                kind: SpawnKind::Task,
                ..
            })
        ));
    }

    #[test]
    fn a_trusted_root_covers_subfolders_but_not_siblings_sharing_a_prefix() {
        let dir = tree();
        let mut store = TrustStore::new();
        let root = store.grant(&dir.path().join("repo")).unwrap();

        let permit = store
            .authorize(&dir.path().join("repo/src"), SpawnKind::Task)
            .unwrap();
        assert_eq!(permit.root(), root);
        assert!(!store.is_trusted(&dir.path().join("repo-evil")));
        assert!(!store.is_trusted(&dir.path().join("other")));
    }

    #[test]
    fn dot_dot_cannot_escape_a_trusted_root() {
        let dir = tree();
        let mut store = TrustStore::new();
        store.grant(&dir.path().join("repo")).unwrap();

        let escape = dir.path().join("repo/../other");
        assert!(matches!(
            store.authorize(&escape, SpawnKind::LanguageServer),
            Err(TrustError::Untrusted { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_inside_a_trusted_root_is_judged_by_its_target() {
        let dir = tree();
        let mut store = TrustStore::new();
        store.grant(&dir.path().join("repo")).unwrap();
        let link = dir.path().join("repo/link");
        std::os::unix::fs::symlink(dir.path().join("other"), &link).unwrap();

        assert!(!store.is_trusted(&link));
    }

    #[test]
    fn revoking_a_parent_removes_roots_inside_it() {
        let dir = tree();
        let mut store = TrustStore::new();
        let inner = store.grant(&dir.path().join("repo/src")).unwrap();
        let outer = store.grant(dir.path()).unwrap();

        let mut removed = store.revoke(dir.path());
        removed.sort();
        assert_eq!(removed, vec![outer, inner]);
        assert_eq!(store.roots().count(), 0);
    }

    #[test]
    fn revoking_a_subfolder_leaves_the_trusted_ancestor() {
        let dir = tree();
        let mut store = TrustStore::new();
        store.grant(&dir.path().join("repo")).unwrap();

        assert!(store.revoke(&dir.path().join("repo/src")).is_empty());
        assert!(store.is_trusted(&dir.path().join("repo/src")));
    }

    #[test]
    fn a_root_deleted_from_disk_can_still_be_revoked() {
        let dir = tree();
        let gone = dir.path().join("other");
        let mut store = TrustStore::new();
        let root = store.grant(&gone).unwrap();
        std::fs::remove_dir(&gone).unwrap();

        assert_eq!(store.revoke(&root), vec![root]);
    }

    #[test]
    fn missing_paths_and_files_are_unresolved() {
        let dir = tree();
        let file = dir.path().join("repo/main.rs");
        std::fs::write(&file, "fn main() {}").unwrap();
        let mut store = TrustStore::new();
        store.grant(dir.path()).unwrap();

        assert!(matches!(
            store.grant(&dir.path().join("missing")),
            Err(TrustError::Unresolved {
                error: io::ErrorKind::NotFound,
                ..
            })
        ));
        assert!(matches!(
            store.authorize(&file, SpawnKind::Task),
            Err(TrustError::Unresolved {
                error: io::ErrorKind::NotADirectory,
                ..
            })
        ));
    }

    #[test]
    fn the_store_refuses_roots_past_its_cap() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = TrustStore::new();
        for i in 0..MAX_TRUSTED_ROOTS {
            let sub = dir.path().join(i.to_string());
            std::fs::create_dir(&sub).unwrap();
            store.grant(&sub).unwrap();
        }
        let extra = dir.path().join("extra");
        std::fs::create_dir(&extra).unwrap();

        assert_eq!(
            store.grant(&extra),
            Err(TrustError::TooManyRoots {
                cap: MAX_TRUSTED_ROOTS
            })
        );
        // Re-granting a root already held is not growth.
        store.grant(&dir.path().join("0")).unwrap();
    }

    #[test]
    fn refusal_names_the_kind_and_folder() {
        let dir = tree();
        let err = TrustStore::new()
            .authorize(&dir.path().join("repo"), SpawnKind::DebugAdapter)
            .unwrap_err();
        let text = err.to_string();
        assert!(text.starts_with("debug adapter refused: "), "{text}");
        assert!(text.ends_with("is not in a trusted folder"), "{text}");
    }
}
