// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Kerf's WASM plugin host. Plugins are cox plugins as is: a `plugin.toml`
//! that `cox_plugin_api::PluginManifest` describes, and a core WASM module
//! called with JSON through extism (`cox:host/v1`). The engine — a worker
//! thread per plugin, bounded lanes, deadlines, the memory cap, the package
//! layout — is the shared `wasm-plugin-host` crate; this crate is the Kerf
//! wiring around it (`docs/architecture.md` §3).
//!
//! - [`Plugins::discover`] reads every manifest and runs no plugin code.
//! - [`Plugins::fire`] activates the plugins a [`Trigger`] names;
//!   [`Plugins::call`] activates on first use.
//! - A [`Gate`] decides whether a plugin may load; [`default_gate`] keeps
//!   project plugins out. Workspace Trust and the plugin lockfile plug in
//!   here.
//! - A failed load or `cox_init` disables the plugin; so do [`MAX_STRIKES`]
//!   failed calls in a row. The host and every other plugin keep running.
//!
//! ```no_run
//! use kerf_plugin_host::{Config, Lane, Plugins, Trigger};
//!
//! let mut plugins = Plugins::discover(Config {
//!     home: "/home/me/.kerf".into(),
//!     project_root: Some("/work/repo".into()),
//!     session_id: "session-1".into(),
//! });
//! for (id, outcome) in plugins.fire(&Trigger::Language("rust".into())) {
//!     if let Err(e) = outcome {
//!         eprintln!("{id}: {e}");
//!     }
//! }
//! let answer: Option<serde_json::Value> = plugins
//!     .call("rust-tools", Lane::Control, "cox_command", &serde_json::json!({}))
//!     .unwrap_or_default();
//! ```

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod host;
mod manifest;
mod trigger;

pub use host::{
    APP, Config, Gate, HostError, INIT_EXPORT, MAX_NOTICE_BYTES, MAX_NOTICES, MAX_PLUGINS,
    MAX_STRIKES, PluginInfo, Plugins, Status, default_gate,
};
pub use manifest::{MAX_MANIFEST_BYTES, MAX_WASM_BYTES, Manifest};
pub use trigger::{Trigger, triggers};

/// Re-exported so callers name the same contract types this host runs.
pub use cox_plugin_api;
/// Re-exported so callers pick a lane and match errors without depending
/// on the engine crate themselves.
pub use wasm_plugin_host::{Lane, PluginError, Source};
