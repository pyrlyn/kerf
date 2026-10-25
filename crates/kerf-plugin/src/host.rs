// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! [`Plugins`]: every discovered plugin and its lifecycle — dormant after
//! discovery, active once a trigger or a first call loads it, disabled when
//! it fails. A failure is a notice and an absence, never an error that stops
//! the IDE (`docs/architecture.md` §3.3).

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cox_plugin_api::{API_MAJOR, InitIn, InitOut, PluginManifest, SessionInfo};
use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;
use wasm_plugin_host::extism::Function;
use wasm_plugin_host::{
    HostEnv, Lane, Limits, Options, PluginError, PluginHost, Source, State, discover::discover,
};

use crate::manifest::{MAX_WASM_BYTES, Manifest, read_capped};
use crate::trigger::{Trigger, triggers};

/// The `.<app>` directory project plugins live under: `<project>/.kerf/plugins/`.
pub const APP: &str = "kerf";
/// The export every module must have; the coding agent's ABI runs it once at activation.
pub const INIT_EXPORT: &str = "cox_init";
/// Consecutive failed calls after which a plugin is disabled for the session.
pub const MAX_STRIKES: u32 = 3;
/// The most plugins one discovery pass keeps; the rest are dropped with a
/// notice, since a repository can hold any number of plugin directories.
pub const MAX_PLUGINS: usize = 256;
/// The most notices kept; older ones are dropped and counted.
pub const MAX_NOTICES: usize = 64;
/// The longest notice kept, in bytes. Notices quote plugin-supplied text.
pub const MAX_NOTICE_BYTES: usize = 1024;

const OPTIONS: Options = Options::new(INIT_EXPORT, "kerf-plugin");

/// Where plugins come from and the session they are initialised for.
#[derive(Debug, Clone)]
pub struct Config {
    /// Kerf's home; user plugins live in `<home>/plugins/`.
    pub home: PathBuf,
    /// The open project; its plugins live in `<project>/.kerf/plugins/`.
    pub project_root: Option<PathBuf>,
    /// The session id passed to `agent_init`.
    pub session_id: String,
}

/// Where a plugin is in its lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Manifest read; no code loaded yet.
    Dormant,
    /// Loaded and `agent_init` answered.
    Active,
    /// Invalid, refused at load, or failed too often; stays off for the session.
    Disabled(String),
}

/// What a [`Gate`] sees about a plugin before its code is loaded.
#[derive(Debug, Clone, Copy)]
pub struct PluginInfo<'a> {
    /// The plugin id.
    pub id: &'a str,
    /// User or project plugin.
    pub source: Source,
    /// The package directory.
    pub dir: &'a Path,
    /// SHA-256 over the package tree.
    pub digest: &'a str,
    /// True for a `link`ed development package, whose digest changes on
    /// every rebuild.
    pub dev: bool,
    /// The validated manifest.
    pub manifest: &'a PluginManifest,
}

/// Decides whether a plugin may load. Asked at every activation attempt, so
/// a refusal leaves the plugin dormant and a later attempt can succeed once
/// the answer changes (Workspace Trust, a lockfile, a grant).
pub type Gate = Box<dyn Fn(&PluginInfo<'_>) -> Result<(), String> + Send + Sync>;

/// The gate [`Plugins::discover`] starts with: user plugins load, project
/// plugins do not. A project plugin is repository content, and cloning a
/// repository must never run its code (`docs/architecture.md` §3.2).
pub fn default_gate(info: &PluginInfo<'_>) -> Result<(), String> {
    match info.source {
        Source::User => Ok(()),
        Source::Project => {
            Err("project plugins load only after the user enables them for this repository".into())
        }
    }
}

/// Why an activation or a call did not happen.
#[derive(Debug, Error)]
pub enum HostError {
    /// No plugin with this id was discovered.
    #[error("no plugin {0:?}")]
    Unknown(String),
    /// The plugin is disabled for the session.
    #[error("plugin {id:?} is disabled: {reason}")]
    Disabled {
        /// The plugin id.
        id: String,
        /// Why it was disabled.
        reason: String,
    },
    /// The gate refused to load the plugin; it stays dormant.
    #[error("plugin {id:?} was not loaded: {reason}")]
    Refused {
        /// The plugin id.
        id: String,
        /// The gate's answer.
        reason: String,
    },
    /// A data-only package: nothing to load or call.
    #[error("plugin {0:?} has no wasm module")]
    NoModule(String),
    /// The plugin call failed.
    #[error("plugin {id:?}: {source}")]
    Plugin {
        /// The plugin id.
        id: String,
        /// What `wasm-plugin-host` reported.
        source: PluginError,
    },
}

struct Package {
    manifest: PluginManifest,
    digest: String,
    triggers: Vec<Trigger>,
}

enum Slot {
    Dormant,
    Active {
        host: PluginHost,
        // Boxed: `InitOut` dwarfs the other variants, and most plugins
        // stay dormant.
        init: Box<InitOut>,
        strikes: u32,
    },
    Disabled(String),
}

struct Entry {
    source: Source,
    dir: PathBuf,
    dev: bool,
    /// `None` when discovery skipped the package; the slot is then disabled.
    package: Option<Package>,
    slot: Slot,
}

/// Grants nothing yet: the `the coding agent:host/v1` and `kerf:host/v1` functions land
/// with the tasks that need them. A module importing one fails to load and
/// is disabled like any other load failure.
struct Env {
    id: String,
}

impl HostEnv for Env {
    fn id(&self) -> &str {
        &self.id
    }

    fn functions(self: &Arc<Self>) -> Vec<Function> {
        Vec::new()
    }
}

/// Every plugin found for one session. Calls block on the plugin's worker
/// up to its call budget; the host owns that worker, so dropping `Plugins`
/// cancels running calls and stops every plugin.
pub struct Plugins {
    config: Config,
    gate: Gate,
    entries: BTreeMap<String, Entry>,
    notices: VecDeque<String>,
    dropped_notices: u64,
}

impl Plugins {
    /// Reads every manifest under the user and project plugin roots. No
    /// plugin code runs: each valid plugin starts [`Status::Dormant`], each
    /// invalid one [`Status::Disabled`] with the reason.
    pub fn discover(config: Config) -> Self {
        let found = discover::<Manifest>(&config.home, APP, config.project_root.as_deref());
        let mut plugins = Self {
            config,
            gate: Box::new(default_gate),
            entries: BTreeMap::new(),
            notices: VecDeque::new(),
            dropped_notices: 0,
        };
        for notice in found.notices {
            plugins.notice(notice);
        }
        let total = found.plugins.len();
        for plugin in found.plugins.into_iter().take(MAX_PLUGINS) {
            let (package, slot) = match plugin.state {
                State::Loaded { manifest, digest } => {
                    let triggers = triggers(&manifest.0);
                    let package = Package {
                        manifest: manifest.0,
                        digest,
                        triggers,
                    };
                    (Some(package), Slot::Dormant)
                }
                State::Skipped { reason } => {
                    plugins.notice(format!("plugin {:?} skipped: {reason}", plugin.id));
                    (None, Slot::Disabled(reason))
                }
            };
            let entry = Entry {
                source: plugin.source,
                dir: plugin.dir,
                dev: plugin.dev,
                package,
                slot,
            };
            plugins.entries.insert(plugin.id, entry);
        }
        if total > MAX_PLUGINS {
            plugins.notice(format!(
                "{} plugins past the limit of {MAX_PLUGINS} were not loaded",
                total - MAX_PLUGINS
            ));
        }
        plugins
    }

    /// Replaces the gate every activation asks first ([`default_gate`] until
    /// then).
    pub fn with_gate(mut self, gate: Gate) -> Self {
        self.gate = gate;
        self
    }

    /// Every discovered plugin id, sorted.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// The plugin's lifecycle state.
    pub fn status(&self, id: &str) -> Option<Status> {
        self.entries.get(id).map(|entry| match &entry.slot {
            Slot::Dormant => Status::Dormant,
            Slot::Active { .. } => Status::Active,
            Slot::Disabled(reason) => Status::Disabled(reason.clone()),
        })
    }

    /// The validated manifest, when discovery accepted the package.
    pub fn manifest(&self, id: &str) -> Option<&PluginManifest> {
        Some(&self.entries.get(id)?.package.as_ref()?.manifest)
    }

    /// The triggers the manifest declares (empty for an unknown id).
    pub fn triggers(&self, id: &str) -> &[Trigger] {
        self.entries
            .get(id)
            .and_then(|entry| entry.package.as_ref())
            .map_or(&[], |package| &package.triggers)
    }

    /// What `agent_init` contributed, once the plugin is active.
    pub fn init_out(&self, id: &str) -> Option<&InitOut> {
        match &self.entries.get(id)?.slot {
            Slot::Active { init, .. } => Some(init),
            _ => None,
        }
    }

    /// Activates every dormant plugin that declares `trigger`, and answers
    /// each attempt by id. A failed load disables that plugin; a gate
    /// refusal leaves it dormant. Neither stops the others.
    pub fn fire(&mut self, trigger: &Trigger) -> Vec<(String, Result<(), HostError>)> {
        let due: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, entry)| matches!(entry.slot, Slot::Dormant))
            .filter(|(_, entry)| {
                entry.package.as_ref().is_some_and(|package| {
                    package.manifest.wasm.is_some() && package.triggers.contains(trigger)
                })
            })
            .map(|(id, _)| id.clone())
            .collect();
        due.into_iter()
            .map(|id| {
                let outcome = self.activate(&id);
                (id, outcome)
            })
            .collect()
    }

    /// Loads the plugin and runs `agent_init`, unless it is already active.
    pub fn activate(&mut self, id: &str) -> Result<(), HostError> {
        let entry = self
            .entries
            .get(id)
            .ok_or_else(|| HostError::Unknown(id.into()))?;
        let package = match (&entry.slot, &entry.package) {
            (Slot::Active { .. }, _) => return Ok(()),
            (Slot::Disabled(reason), _) => {
                return Err(HostError::Disabled {
                    id: id.into(),
                    reason: reason.clone(),
                });
            }
            (Slot::Dormant, Some(package)) => package,
            // Discovery never leaves a package-less entry dormant.
            (Slot::Dormant, None) => return Err(HostError::NoModule(id.into())),
        };
        let Some(wasm) = &package.manifest.wasm else {
            return Err(HostError::NoModule(id.into()));
        };
        let info = PluginInfo {
            id,
            source: entry.source,
            dir: &entry.dir,
            digest: &package.digest,
            dev: entry.dev,
            manifest: &package.manifest,
        };
        (self.gate)(&info).map_err(|reason| HostError::Refused {
            id: id.into(),
            reason,
        })?;
        match self.load(id, &entry.dir.join(wasm), &package.manifest) {
            Ok((host, init)) => {
                if let Some(entry) = self.entries.get_mut(id) {
                    entry.slot = Slot::Active {
                        host,
                        init,
                        strikes: 0,
                    };
                }
                Ok(())
            }
            Err(reason) => {
                self.disable(id, &reason);
                Err(HostError::Disabled {
                    id: id.into(),
                    reason,
                })
            }
        }
    }

    fn load(
        &self,
        id: &str,
        wasm_path: &Path,
        manifest: &PluginManifest,
    ) -> Result<(PluginHost, Box<InitOut>), String> {
        let wasm = read_capped(wasm_path, MAX_WASM_BYTES)
            .map_err(|e| format!("cannot read {}: {e}", wasm_path.display()))?;
        let limits = Limits {
            memory_mib: manifest.limits.memory_mib,
            call_ms: manifest.limits.call_ms,
        };
        let env = Arc::new(Env { id: id.into() });
        let host = PluginHost::load(&wasm, &limits, &OPTIONS, env).map_err(|e| e.to_string())?;
        let cwd = self
            .config
            .project_root
            .as_deref()
            .unwrap_or(&self.config.home);
        let input = InitIn {
            api: API_MAJOR,
            plugin_id: id.into(),
            config: serde_json::Value::Object(serde_json::Map::new()),
            session: SessionInfo {
                id: self.config.session_id.clone(),
                cwd: cwd.display().to_string(),
            },
            // No grant store yet: the gate decides whether a plugin loads at
            // all, and a loaded plugin is told what its manifest asked for.
            granted: serde_json::to_value(&manifest.capabilities).unwrap_or_default(),
            // Kerf is not one of the coding agent's surfaces; absence is the documented
            // "older host" answer, so the guest falls back to its defaults.
            surface: None,
        };
        let init = host
            .call::<_, InitOut>(Lane::Control, INIT_EXPORT, &input, host.call_cap())
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        Ok((host, Box::new(init)))
    }

    /// Calls `export` with JSON in and out, activating the plugin first if
    /// it is dormant (first use). An export the module does not have answers
    /// `Ok(None)`. A failed call counts against the plugin, and the
    /// [`MAX_STRIKES`]th failure in a row disables it; a full lane does not
    /// count, since that is back-pressure rather than a fault.
    pub fn call<I: Serialize, O: DeserializeOwned>(
        &mut self,
        id: &str,
        lane: Lane,
        export: &str,
        input: &I,
    ) -> Result<Option<O>, HostError> {
        self.activate(id)?;
        let Some(Slot::Active { host, strikes, .. }) =
            self.entries.get_mut(id).map(|entry| &mut entry.slot)
        else {
            return Err(HostError::Unknown(id.into()));
        };
        match host.call(lane, export, input, host.call_cap()) {
            Ok(out) => {
                *strikes = 0;
                Ok(out)
            }
            Err(PluginError::Busy) => Err(HostError::Plugin {
                id: id.into(),
                source: PluginError::Busy,
            }),
            Err(source) => {
                *strikes += 1;
                let strikes = *strikes;
                self.notice(format!("plugin {id:?}: {source}"));
                if strikes >= MAX_STRIKES {
                    self.disable(
                        id,
                        &format!("{strikes} failed calls in a row, last: {source}"),
                    );
                }
                Err(HostError::Plugin {
                    id: id.into(),
                    source,
                })
            }
        }
    }

    /// Turns the plugin off for the session with `reason` and stops its
    /// worker. An unknown id is ignored.
    pub fn disable(&mut self, id: &str, reason: &str) {
        let Some(entry) = self.entries.get_mut(id) else {
            return;
        };
        // Dropping the old slot drops its `PluginHost`, which cancels a
        // running call and joins the worker.
        entry.slot = Slot::Disabled(reason.into());
        self.notice(format!("plugin {id:?} disabled: {reason}"));
    }

    /// Warnings for the user, oldest first.
    pub fn notices(&self) -> impl Iterator<Item = &str> {
        self.notices.iter().map(String::as_str)
    }

    /// How many notices were dropped to keep [`MAX_NOTICES`].
    pub fn dropped_notices(&self) -> u64 {
        self.dropped_notices
    }

    fn notice(&mut self, mut text: String) {
        if text.len() > MAX_NOTICE_BYTES {
            text.truncate(text.floor_char_boundary(MAX_NOTICE_BYTES));
        }
        if self.notices.len() >= MAX_NOTICES {
            self.notices.pop_front();
            self.dropped_notices += 1;
        }
        self.notices.push_back(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> Plugins {
        let home = tempfile::tempdir().unwrap();
        Plugins::discover(Config {
            home: home.path().to_path_buf(),
            project_root: None,
            session_id: "s".into(),
        })
    }

    #[test]
    fn notices_are_capped_in_count_and_length() {
        let mut plugins = empty();
        for i in 0..MAX_NOTICES + 5 {
            plugins.notice(format!("{i}"));
        }
        assert_eq!(plugins.notices().count(), MAX_NOTICES);
        assert_eq!(plugins.dropped_notices(), 5);
        assert_eq!(plugins.notices().next(), Some("5"));

        plugins.notice("é".repeat(MAX_NOTICE_BYTES));
        let last = plugins.notices().last().unwrap();
        assert!(last.len() <= MAX_NOTICE_BYTES, "{}", last.len());
    }

    #[test]
    fn unknown_id_is_an_error_not_a_panic() {
        let mut plugins = empty();
        assert!(matches!(
            plugins.activate("nope"),
            Err(HostError::Unknown(_))
        ));
        plugins.disable("nope", "ignored");
        assert_eq!(plugins.status("nope"), None);
    }
}
