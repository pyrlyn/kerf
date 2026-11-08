// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The model behind the Source control screen: the branch, the "Staged" and
//! "Changes" groups, one file's diff hunks, stage, unstage and commit. No
//! push, checkout or reset of the working tree: `docs/architecture.md` §8
//! decision 6 keeps writes to stage and commit.
//!
//! Git runs in-process through libgit2, which never runs the repository's
//! hooks or config-driven commands (`core.fsmonitor`, filters' processes), so
//! opening an untrusted repository here starts nothing. Everything read from
//! the repository is capped: file count, diff rows, line length and the size
//! of a file worth diffing.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod diff;
mod error;

use std::path::{Component, Path};

use git2::{ErrorCode, Repository, RepositoryState, StatusOptions};

pub use diff::{FileDiff, Hunk, Line, LineKind};
pub use error::Error;

/// Most files one status lists across both groups; the rest are dropped and
/// [`Status::truncated`] is set.
pub const MAX_FILES: usize = 10_000;
/// Most diff lines one file's diff keeps, hunk headers not counted.
pub const MAX_DIFF_LINES: usize = 20_000;
/// Longest line, path or branch name kept, in bytes; longer ones are cut at
/// a character boundary.
pub const MAX_LINE_BYTES: usize = 4096;
/// Files larger than this are shown as binary instead of diffed: libgit2
/// loads both sides whole to diff them.
pub const MAX_DIFF_FILE_BYTES: u64 = 8 * 1024 * 1024;
/// Longest commit message accepted.
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;

/// Which group a file row is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// HEAD against the index: what the next commit takes.
    Staged,
    /// The index against the working tree, untracked files included.
    Changes,
}

/// What happened to a file, as the screen's status letter shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// New in the index.
    Added,
    /// Content, mode or type changed.
    Modified,
    /// Removed.
    Deleted,
    /// In the working tree, not in the index.
    Untracked,
    /// Unmerged after a merge, rebase or cherry-pick.
    Conflicted,
}

impl Change {
    /// The letter the screen shows: `A`, `M`, `D`, `?` or `U`, as `git status
    /// --short` does.
    pub fn letter(self) -> char {
        match self {
            Self::Added => 'A',
            Self::Modified => 'M',
            Self::Deleted => 'D',
            Self::Untracked => '?',
            Self::Conflicted => 'U',
        }
    }
}

/// One file row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Path from the working tree's root, `/`-separated.
    pub path: String,
    /// What changed.
    pub change: Change,
}

/// A snapshot of the repository for the screen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    /// The branch HEAD points at, even before its first commit; when
    /// detached, the abbreviated commit id.
    pub branch: String,
    /// HEAD is detached.
    pub detached: bool,
    /// The "Staged" group, sorted by path.
    pub staged: Vec<FileEntry>,
    /// The "Changes" group, sorted by path.
    pub changes: Vec<FileEntry>,
    /// Rows were dropped: over [`MAX_FILES`], or a path that is not UTF-8.
    pub truncated: bool,
}

impl Status {
    /// The rows of one group.
    pub fn group(&self, group: Group) -> &[FileEntry] {
        match group {
            Group::Staged => &self.staged,
            Group::Changes => &self.changes,
        }
    }
}

/// An open repository with a working tree.
pub struct Scm {
    repo: Repository,
}

impl std::fmt::Debug for Scm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scm")
            .field("workdir", &self.repo.workdir())
            .finish()
    }
}

impl Scm {
    /// The repository whose working tree holds `path`, searching upward as
    /// `git` does.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let repo = Repository::discover(path).map_err(|e| match e.code() {
            ErrorCode::NotFound => Error::NotARepository,
            _ => Error::Git(e),
        })?;
        if repo.workdir().is_none() {
            return Err(Error::Bare);
        }
        Ok(Self { repo })
    }

    /// The working tree's root.
    pub fn workdir(&self) -> &Path {
        // `open` refuses bare repositories, so there is always one.
        self.repo.workdir().unwrap_or_else(|| self.repo.path())
    }

    /// Read the branch and both groups.
    pub fn status(&self) -> Result<Status, Error> {
        let (branch, detached) = self.head()?;
        let mut status = Status {
            branch: cut(&branch).0,
            detached,
            ..Status::default()
        };
        // Submodules are left out: staging one is a gitlink update the screen
        // has no row for yet.
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false)
            .exclude_submodules(true)
            .renames_head_to_index(false)
            .renames_index_to_workdir(false);
        let statuses = self.repo.statuses(Some(&mut opts))?;
        let mut rows = 0;
        for entry in statuses.iter() {
            let Ok(path) = entry.path() else {
                // A host takes UTF-8 paths and hands them back to stage; a
                // lossy one would name another file.
                status.truncated = true;
                continue;
            };
            let s = entry.status();
            let staged = if s.is_conflicted() {
                None
            } else if s.is_index_new() {
                Some(Change::Added)
            } else if s.is_index_deleted() {
                Some(Change::Deleted)
            } else if s.is_index_modified() || s.is_index_typechange() || s.is_index_renamed() {
                Some(Change::Modified)
            } else {
                None
            };
            let change = if s.is_conflicted() {
                Some(Change::Conflicted)
            } else if s.is_wt_new() {
                Some(Change::Untracked)
            } else if s.is_wt_deleted() {
                Some(Change::Deleted)
            } else if s.is_wt_modified() || s.is_wt_typechange() || s.is_wt_renamed() {
                Some(Change::Modified)
            } else {
                None
            };
            for (list, change) in [(&mut status.staged, staged), (&mut status.changes, change)] {
                let Some(change) = change else { continue };
                if rows == MAX_FILES {
                    status.truncated = true;
                    continue;
                }
                rows += 1;
                list.push(FileEntry {
                    path: path.to_owned(),
                    change,
                });
            }
        }
        Ok(status)
    }

    /// The branch name and whether HEAD is detached.
    fn head(&self) -> Result<(String, bool), Error> {
        match self.repo.head() {
            Ok(head) if self.repo.head_detached()? => {
                let id = head.target().map(|id| id.to_string()).unwrap_or_default();
                Ok((id.chars().take(7).collect(), true))
            }
            Ok(head) => Ok((
                String::from_utf8_lossy(head.shorthand_bytes()).into_owned(),
                false,
            )),
            Err(e) if e.code() == ErrorCode::UnbornBranch => {
                let head = self.repo.find_reference("HEAD")?;
                let target = head.symbolic_target_bytes().unwrap_or_default();
                let name = target.strip_prefix(b"refs/heads/").unwrap_or(target);
                Ok((String::from_utf8_lossy(name).into_owned(), false))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// One file's diff in `group`: HEAD to index for [`Group::Staged`],
    /// index to working tree for [`Group::Changes`], where an untracked file
    /// shows as all added.
    pub fn diff(&self, group: Group, path: &str) -> Result<FileDiff, Error> {
        diff::file_diff(&self.repo, group, checked(path)?)
    }

    /// Stage `path` as it is in the working tree; a deleted file is staged as
    /// a deletion.
    pub fn stage(&self, path: &str) -> Result<(), Error> {
        let rel = checked(path)?;
        let mut index = self.repo.index()?;
        // Another git may have written the index since it was last read.
        index.read(false)?;
        match self.workdir().join(rel).symlink_metadata() {
            Ok(_) => index.add_path(rel)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => index.remove_path(rel)?,
            Err(e) => return Err(Error::Io(e)),
        }
        index.write()?;
        Ok(())
    }

    /// Put `path` in the index back as HEAD has it; the working tree is not
    /// touched.
    pub fn unstage(&self, path: &str) -> Result<(), Error> {
        let rel = checked(path)?;
        match self.head_commit()? {
            Some(head) => self.repo.reset_default(Some(head.as_object()), [rel])?,
            None => {
                let mut index = self.repo.index()?;
                index.read(false)?;
                index.remove_path(rel)?;
                index.write()?;
            }
        }
        Ok(())
    }

    /// Commit the index on the current branch with `message`, cleaned up as
    /// `git commit` does without comment stripping. Returns the new commit's
    /// id. Hooks do not run and commits are not signed.
    pub fn commit(&self, message: &str) -> Result<String, Error> {
        if message.len() > MAX_MESSAGE_BYTES {
            return Err(Error::MessageTooLarge {
                max: MAX_MESSAGE_BYTES,
            });
        }
        if self.repo.state() != RepositoryState::Clean {
            return Err(Error::Busy);
        }
        let message = git2::message_prettify(message, None)?;
        if message.trim().is_empty() {
            return Err(Error::EmptyMessage);
        }
        let mut index = self.repo.index()?;
        index.read(false)?;
        if index.has_conflicts() {
            return Err(Error::Busy);
        }
        let tree_id = index.write_tree()?;
        let parent = self.head_commit()?;
        let unchanged = match &parent {
            Some(p) => p.tree_id() == tree_id,
            None => index.is_empty(),
        };
        if unchanged {
            return Err(Error::NothingToCommit);
        }
        let tree = self.repo.find_tree(tree_id)?;
        let sig = self.repo.signature().map_err(|e| match e.code() {
            ErrorCode::NotFound => Error::NoIdentity,
            _ => Error::Git(e),
        })?;
        let parents: Vec<_> = parent.iter().collect();
        let id = self
            .repo
            .commit(Some("HEAD"), &sig, &sig, &message, &tree, &parents)?;
        Ok(id.to_string())
    }

    /// The commit HEAD points at; `None` before the first commit.
    fn head_commit(&self) -> Result<Option<git2::Commit<'_>>, Error> {
        match self.repo.head() {
            Ok(head) => Ok(Some(head.peel_to_commit()?)),
            Err(e) if e.code() == ErrorCode::UnbornBranch => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

/// `path` as a relative path inside the working tree. libgit2 checks the
/// rest (`.git`, NTFS and HFS aliases) when it touches the index.
fn checked(path: &str) -> Result<&Path, Error> {
    let p = Path::new(path);
    let inside = !path.is_empty()
        && path.len() <= MAX_LINE_BYTES
        && p.components().all(|c| matches!(c, Component::Normal(_)));
    if inside { Ok(p) } else { Err(Error::BadPath) }
}

/// `text` cut to [`MAX_LINE_BYTES`] at a character boundary, and whether it
/// was cut.
pub(crate) fn cut(text: &str) -> (String, bool) {
    if text.len() <= MAX_LINE_BYTES {
        return (text.to_owned(), false);
    }
    let mut end = MAX_LINE_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

#[cfg(test)]
mod tests {
    use super::{Error, MAX_LINE_BYTES, checked, cut};

    #[test]
    fn paths_must_stay_inside_the_working_tree() {
        assert!(checked("src/main.rs").is_ok());
        for bad in ["", "/etc/passwd", "../x", "a/../../x", "./a"] {
            assert!(matches!(checked(bad), Err(Error::BadPath)), "{bad}");
        }
    }

    #[test]
    fn long_text_is_cut_on_a_character_boundary() {
        let long = "é".repeat(MAX_LINE_BYTES);
        let (text, was_cut) = cut(&long);
        assert!(was_cut);
        assert!(text.len() <= MAX_LINE_BYTES);
        assert_eq!(cut("short"), ("short".to_owned(), false));
    }
}
