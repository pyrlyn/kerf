// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The logic behind the plugins settings screen and the plugin permission
//! sheet, written once for every native host (`docs/architecture.md` §3.2,
//! §3.3). No views: a host draws [`Row`]s and sends back [`Decision`]s.
//!
//! - [`Manager::open`] discovers plugins the way `kerf-plugin` does, without
//!   running plugin code, and reads the project's `kerf-plugins.lock`.
//! - [`Manager::row`] is one plugin's [`State`]: enabled, exports off after
//!   [`kerf_plugin::MAX_STRIKES`] failures this session, needing approval, a
//!   project plugin not enabled for this repository, a lockfile mismatch, or
//!   an invalid manifest. Its grant lines are the permission prompt.
//! - [`Manager::decide`] grants every line for the digest the prompt showed;
//!   changed bytes or a wider request ask again.
//! - [`Manager::gate`] hands the same answer to the plugin runtime.

// Unit tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod grant;
mod manager;

pub use grant::{Grant, Grants, MAX_GRANTS, Scope, Verdict, check, display, grant_lines};
pub use kerf_plugin::Source;
pub use kerf_plugin_lock::Entry as LockEntry;
pub use manager::{Decision, Manager, Row, State};

/// The most grant lines one plugin may ask for. A prompt cannot be cut
/// short, since the user would allow lines they never saw, so a bigger
/// request is refused as invalid.
pub const MAX_GRANT_LINES: usize = 256;

/// The most (plugin, export) failure counters a session keeps; past it, new
/// exports are not counted.
pub const MAX_TRACKED_CALLS: usize = 1024;

/// Why a decision or a lockfile change was not made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// No plugin at this index.
    #[error("no plugin at index {0}")]
    NoPlugin(usize),
    /// The plugin is invalid or does not match the lockfile, so no grant can
    /// enable it.
    #[error("plugin {0:?} cannot be granted in its state")]
    NotAskable(String),
    /// The package changed after the prompt was shown.
    #[error("plugin {0:?} changed since the prompt was shown")]
    StaleDigest(String),
    /// The grant store is full.
    #[error("more than {max} grants")]
    TooManyGrants {
        /// The cap.
        max: usize,
    },
    /// The lockfile lives in a project, and none is open.
    #[error("no project is open")]
    NoProject,
    /// The lockfile could not be read or written.
    #[error("{0}")]
    Lockfile(String),
}

#[cfg(test)]
mod tests;
