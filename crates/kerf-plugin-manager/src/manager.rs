// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! [`Manager`]: the plugins list and the permission prompt over one
//! discovery pass, the lockfile, the grants and this session's failures.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use kerf_plugin::{APP, INIT_EXPORT, MAX_PLUGINS, MAX_STRIKES, PluginInfo, Source};
use kerf_plugin_lock::{Digest, Entry, FILE_NAME, Lockfile};
use wasm_plugin_host::{Options, State as Found, discover::discover};

use crate::grant::{Grant, Grants, Scope, Verdict, check, display, grant_lines};
use crate::{Error, MAX_GRANT_LINES, MAX_TRACKED_CALLS};

/// Where a plugin stands on the plugins screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Granted for these bytes; it loads.
    Enabled,
    /// Granted and loading, but these exports failed [`MAX_STRIKES`] times in
    /// a row and stay off for the session.
    ExportsDisabled(Vec<String>),
    /// No grant for these bytes, or it asks for more than was granted.
    NeedsApproval {
        /// Requested and not granted.
        added: Vec<String>,
        /// Granted and no longer requested.
        removed: Vec<String>,
    },
    /// A project plugin the user has not enabled for this repository.
    ProjectNotEnabled,
    /// The package does not match `kerf-plugins.lock`; kept off until the
    /// user reviews it.
    LockMismatch(String),
    /// The manifest was refused; nothing can enable it.
    Invalid(String),
}

/// One plugin as the plugins screen and the permission prompt show it.
/// Every text but `id` is display-safe (see [`display`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The plugin id, as is: for a refused package it is a directory name
    /// from the repository, so pass it through [`display`] to show it.
    pub id: String,
    /// The manifest's display name; the id when the manifest was refused.
    pub name: String,
    /// The manifest version; empty when refused.
    pub version: String,
    /// User or project plugin.
    pub source: Source,
    /// SHA-256 over the package tree; empty when refused.
    pub digest: String,
    /// The linear memory cap it runs under, in MiB.
    pub memory_mib: u32,
    /// Every grant line it asks for, sorted.
    pub lines: Vec<String>,
    /// The lockfile's pin for this id.
    pub lock: Option<Entry>,
    /// Where it stands.
    pub state: State,
}

/// The user's answer to the permission prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Grant every line for the digest shown.
    Allow,
    /// Grant nothing; the plugin stays off.
    Deny,
}

#[derive(Debug, Clone)]
struct Package {
    id: String,
    source: Source,
    dev: bool,
    loaded: Result<Loaded, String>,
}

#[derive(Debug, Clone)]
struct Loaded {
    name: String,
    version: String,
    digest: String,
    lines: Vec<String>,
    memory_mib: u32,
}

/// What decides whether a package may load, apart from failures: shared by
/// the screen and the [`kerf_plugin::Gate`] the runtime asks.
#[derive(Clone)]
struct Policy {
    project_root: Option<PathBuf>,
    lock: Result<Option<Lockfile>, String>,
    grants: Grants,
}

impl Policy {
    fn scope(&self, source: Source) -> Scope {
        match source {
            Source::User => Scope::User,
            Source::Project => Scope::Project(self.project_root.clone().unwrap_or_default()),
        }
    }

    fn state(&self, package: &Package, version: &str, digest: &str, lines: &[String]) -> State {
        match &self.lock {
            Err(reason) => return State::LockMismatch(reason.clone()),
            Ok(Some(lock)) => match lock.verify(&package.id, version, digest) {
                Ok(()) | Err(kerf_plugin_lock::VerifyError::NotLocked { .. }) => {}
                Err(refused) => return State::LockMismatch(refused.to_string()),
            },
            Ok(None) => {}
        }
        let stored = self.grants.get(&self.scope(package.source), &package.id);
        match check(lines, digest, package.dev, stored) {
            Verdict::Granted => State::Enabled,
            Verdict::NeedsApproval { .. }
                if stored.is_none() && package.source == Source::Project =>
            {
                State::ProjectNotEnabled
            }
            Verdict::NeedsApproval { added, removed } => State::NeedsApproval { added, removed },
        }
    }
}

/// The plugins of one session: discovered without running plugin code,
/// checked against the lockfile and the grants. Rows are sorted by id and
/// keep their index until [`Manager::rescan`].
pub struct Manager {
    home: PathBuf,
    policy: Policy,
    packages: Vec<Package>,
    /// Consecutive failures per (plugin id, export), for this session only.
    failures: BTreeMap<(String, String), u32>,
}

impl Manager {
    /// Discovers the plugins under `home` and `project_root`, reads the
    /// project's lockfile, and checks both against `grants`.
    pub fn open(home: PathBuf, project_root: Option<PathBuf>, grants: Grants) -> Self {
        let policy = Policy {
            project_root,
            lock: Ok(None),
            grants,
        };
        let mut manager = Self {
            home,
            policy,
            packages: Vec::new(),
            failures: BTreeMap::new(),
        };
        manager.rescan();
        manager
    }

    /// Reads the plugin roots and the lockfile again: after an install, an
    /// update or a lockfile change. Grants and failures stay.
    pub fn rescan(&mut self) {
        let root = self.policy.project_root.as_deref();
        let found = discover::<kerf_plugin::Manifest>(&self.home, APP, root);
        let options = Options::new(INIT_EXPORT, "kerf-plugin");
        self.packages = found
            .plugins
            .into_iter()
            .take(MAX_PLUGINS)
            .map(|plugin| {
                let loaded = match plugin.state {
                    Found::Loaded { manifest, digest } => {
                        let manifest = manifest.0;
                        let lines = grant_lines(&manifest);
                        if lines.len() > MAX_GRANT_LINES {
                            Err(format!(
                                "asks for {} capabilities, more than {MAX_GRANT_LINES}",
                                lines.len()
                            ))
                        } else {
                            Ok(Loaded {
                                name: manifest.name,
                                version: manifest.version,
                                digest,
                                lines,
                                memory_mib: manifest
                                    .limits
                                    .memory_mib
                                    .unwrap_or(options.memory_mib)
                                    .min(options.max_memory_mib),
                            })
                        }
                    }
                    Found::Skipped { reason } => Err(reason),
                };
                Package {
                    id: plugin.id,
                    source: plugin.source,
                    dev: plugin.dev,
                    loaded,
                }
            })
            .collect();
        self.policy.lock = match root.map(|root| root.join(FILE_NAME)) {
            Some(path) if path.exists() => Lockfile::read(&path)
                .map(Some)
                .map_err(|e| format!("kerf-plugins.lock is unreadable: {e}")),
            _ => Ok(None),
        };
    }

    /// How many plugins were found.
    pub fn len(&self) -> usize {
        self.packages.len()
    }

    /// True when no plugin was found.
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    /// The grants, including those decided in this session.
    pub fn grants(&self) -> &Grants {
        &self.policy.grants
    }

    /// The plugin at `index`.
    pub fn row(&self, index: usize) -> Option<Row> {
        let package = self.packages.get(index)?;
        let lock = match &self.policy.lock {
            Ok(Some(lock)) => lock.get(&package.id).cloned(),
            _ => None,
        };
        let options = Options::new(INIT_EXPORT, "kerf-plugin");
        let mut row = Row {
            id: package.id.clone(),
            name: display(&package.id),
            version: String::new(),
            source: package.source,
            digest: String::new(),
            memory_mib: options.memory_mib,
            lines: Vec::new(),
            lock,
            state: State::Invalid(String::new()),
        };
        let loaded = match &package.loaded {
            Ok(loaded) => loaded,
            Err(reason) => {
                row.state = State::Invalid(display(reason));
                return Some(row);
            }
        };
        row.name = display(&loaded.name);
        row.version = display(&loaded.version);
        row.digest = loaded.digest.clone();
        row.memory_mib = loaded.memory_mib;
        row.lines = loaded.lines.iter().map(|line| display(line)).collect();
        let shown = |lines: Vec<String>| lines.iter().map(|line| display(line)).collect();
        row.state = match self
            .policy
            .state(package, &loaded.version, &loaded.digest, &loaded.lines)
        {
            State::Enabled => {
                let off: Vec<String> = self
                    .failures
                    .iter()
                    .filter(|((id, _), strikes)| *id == package.id && **strikes >= MAX_STRIKES)
                    .map(|((_, export), _)| display(export))
                    .collect();
                if off.is_empty() {
                    State::Enabled
                } else {
                    State::ExportsDisabled(off)
                }
            }
            State::NeedsApproval { added, removed } => State::NeedsApproval {
                added: shown(added),
                removed: shown(removed),
            },
            State::LockMismatch(reason) => State::LockMismatch(display(&reason)),
            other => other,
        };
        Some(row)
    }

    /// Answers the permission prompt for the plugin at `index`. `digest` is
    /// the one the prompt showed: if the package changed since, nothing is
    /// granted, so the user never allows bytes they did not see.
    pub fn decide(&mut self, index: usize, digest: &str, decision: Decision) -> Result<(), Error> {
        let package = self.packages.get(index).ok_or(Error::NoPlugin(index))?;
        let not_askable = || Error::NotAskable(package.id.clone());
        let loaded = package.loaded.as_ref().map_err(|_| not_askable())?;
        if matches!(
            self.policy
                .state(package, &loaded.version, &loaded.digest, &loaded.lines),
            State::LockMismatch(_)
        ) {
            return Err(not_askable());
        }
        if loaded.digest != digest {
            return Err(Error::StaleDigest(package.id.clone()));
        }
        if decision == Decision::Deny {
            return Ok(());
        }
        let grant = Grant {
            digest: loaded.digest.clone(),
            lines: loaded.lines.iter().cloned().collect::<BTreeSet<_>>(),
            dev: package.dev,
        };
        let scope = self.policy.scope(package.source);
        let id = package.id.clone();
        self.policy.grants.insert(scope, &id, grant)
    }

    /// Turns the plugin at `index` off: its grant is cleared, so it asks
    /// again before it next loads.
    pub fn revoke(&mut self, index: usize) -> Result<(), Error> {
        let package = self.packages.get(index).ok_or(Error::NoPlugin(index))?;
        let scope = self.policy.scope(package.source);
        self.policy.grants.remove(&scope, &package.id);
        Ok(())
    }

    /// Pins the package at `index` in `kerf-plugins.lock`, as it is now: the
    /// user reviewed a mismatch and accepts these bytes. The grant is still
    /// checked afterwards, so changed bytes ask for permission too.
    pub fn accept_lock(&mut self, index: usize) -> Result<(), Error> {
        let package = self.packages.get(index).ok_or(Error::NoPlugin(index))?;
        let loaded =
            (package.loaded.as_ref()).map_err(|_| Error::NotAskable(package.id.clone()))?;
        let root = self.policy.project_root.as_ref().ok_or(Error::NoProject)?;
        let mut lock = match &self.policy.lock {
            Ok(lock) => lock.clone().unwrap_or_default(),
            Err(reason) => return Err(Error::Lockfile(reason.clone())),
        };
        let digest = Digest::parse(&loaded.digest)
            .map_err(|e| Error::Lockfile(format!("{}: {e}", package.id)))?;
        let entry = Entry {
            version: loaded.version.clone(),
            digest,
        };
        lock.pin(package.id.clone(), entry);
        lock.write(&root.join(FILE_NAME))
            .map_err(|e| Error::Lockfile(e.to_string()))?;
        self.policy.lock = Ok(Some(lock));
        Ok(())
    }

    /// Records the outcome of a call to `export` of plugin `id`. The
    /// [`MAX_STRIKES`]th failure in a row turns that export off for the
    /// session; answers whether it is off now.
    pub fn record_call(&mut self, id: &str, export: &str, ok: bool) -> bool {
        let key = (id.to_owned(), export.to_owned());
        let tracked = self.failures.contains_key(&key);
        if !tracked && (ok || self.failures.len() >= MAX_TRACKED_CALLS) {
            return false;
        }
        let strikes = self.failures.entry(key).or_insert(0);
        if *strikes >= MAX_STRIKES {
            return true;
        }
        *strikes = if ok { 0 } else { *strikes + 1 };
        *strikes >= MAX_STRIKES
    }

    /// The gate the plugin runtime asks before it loads a plugin: only a
    /// package that matches the lockfile and its grant loads. A snapshot of
    /// the grants and lockfile as they are now; take a new one after a
    /// decision.
    pub fn gate(&self) -> kerf_plugin::Gate {
        let policy = self.policy.clone();
        Box::new(move |info: &PluginInfo<'_>| {
            let package = Package {
                id: info.id.to_owned(),
                source: info.source,
                dev: info.dev,
                loaded: Err(String::new()),
            };
            let lines = grant_lines(info.manifest);
            match policy.state(&package, &info.manifest.version, info.digest, &lines) {
                State::Enabled => Ok(()),
                State::ProjectNotEnabled => {
                    Err("project plugin not enabled for this repository".into())
                }
                State::LockMismatch(reason) => Err(reason),
                _ => Err("not granted for this package digest".into()),
            }
        })
    }
}
