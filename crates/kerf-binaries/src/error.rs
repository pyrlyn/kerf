// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The one error type for installing and resolving pinned binaries.

use std::path::PathBuf;

/// What can go wrong installing or resolving a pinned binary.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A field of the spec is unusable; the message names it.
    #[error("invalid binary spec: {0}")]
    InvalidSpec(String),
    /// The spec pins no asset for the machine Kerf runs on.
    #[error("no pinned asset for this machine ({target})")]
    UnsupportedTarget {
        /// The host as ketch names targets.
        target: String,
    },
    /// Reading or writing a file under the binaries root failed.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file or directory involved.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The lockfile for ketch could not be rendered.
    #[error("could not write the ketch lockfile: {0}")]
    Lockfile(#[from] toml::ser::Error),
    /// The ketch program could not be started.
    #[error("could not run ketch ({}): {source}", program.display())]
    Spawn {
        /// The program Kerf tried to run.
        program: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// ketch ran and refused: a checksum mismatch, a download over the size
    /// cap, a missing release. `stderr` is what ketch said, capped and with
    /// control characters removed.
    #[error("ketch failed ({status}): {stderr}")]
    Ketch {
        /// Exit status as the OS reports it.
        status: String,
        /// The head of ketch's error output.
        stderr: String,
    },
    /// The installed tree is bigger than the cap; it has been uninstalled.
    #[error("{name} unpacked to {size} bytes, over the {limit}-byte cap; removed")]
    TooLarge {
        /// Package name.
        name: String,
        /// Bytes found in the store.
        size: u64,
        /// The cap it went over.
        limit: u64,
    },
    /// No installed binary of that name.
    #[error("{bin} is not installed")]
    NotInstalled {
        /// The binary looked for.
        bin: String,
    },
    /// The installed binary resolves outside the binaries root.
    #[error("{} resolves outside the binaries root", path.display())]
    OutsideRoot {
        /// The path it resolved to.
        path: PathBuf,
    },
}
