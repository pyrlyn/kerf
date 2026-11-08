// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

/// Why a source control call failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No repository holds the folder.
    #[error("not inside a git repository")]
    NotARepository,
    /// The repository has no working tree to show.
    #[error("the repository is bare")]
    Bare,
    /// A path is empty, absolute, climbs out with `..`, or is too long.
    #[error("the path is not a file inside the working tree")]
    BadPath,
    /// The message is empty once cleaned up.
    #[error("the commit message is empty")]
    EmptyMessage,
    /// The message is over its cap.
    #[error("the commit message is over {max} bytes")]
    MessageTooLarge {
        /// The cap.
        max: usize,
    },
    /// The index matches HEAD.
    #[error("nothing is staged")]
    NothingToCommit,
    /// `user.name` or `user.email` is not configured.
    #[error("set user.name and user.email to commit")]
    NoIdentity,
    /// A merge, rebase or similar is in progress, or the index has conflicts.
    #[error("the repository is in the middle of a merge, rebase or similar")]
    Busy,
    /// The working tree could not be read.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// libgit2 failed.
    #[error(transparent)]
    Git(#[from] git2::Error),
}
