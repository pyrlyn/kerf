// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The logic behind the project navigator and Open Quickly, with no UI:
//! hosts draw [`Row`]s and [`Hit`]s and send actions back.
//!
//! The project's folders are a repository's contents, so untrusted: folder
//! listings, loaded nodes, the file list and the hits all have hard caps.

// Tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod quick;
mod tree;

use std::path::PathBuf;

pub use quick::{Hit, HitKind, MAX_HITS, OpenQuickly};
pub use tree::{FileList, MAX_CHILDREN, MAX_FILES, MAX_NODES, Navigator, NodeId, NodeKind, Row};

/// Why the navigator could not do something.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The project root is not a readable folder.
    #[error("{} is not a folder", path.display())]
    NotAFolder {
        /// The root asked for.
        path: PathBuf,
    },
    /// A folder could not be listed.
    #[error("cannot list {}: {source}", path.display())]
    List {
        /// The folder.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
}
