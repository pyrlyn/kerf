// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Grants: what the user allowed a plugin to do, bound to the package digest
//! it was decided against (`docs/architecture.md` §3.2).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use kerf_plugin::cox_plugin_api::{ModelTier, PluginManifest};

use crate::Error;

/// The most grants a [`Grants`] store holds. Granting is a user action, but
/// project plugins are keyed per repository, so the store grows with every
/// clone the user enables plugins in.
pub const MAX_GRANTS: usize = 1024;

/// Where a grant applies. A project plugin's grant is keyed on its
/// repository root, so the same id in another clone asks again.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    /// A user plugin, everywhere.
    User,
    /// A project plugin, in this repository only.
    Project(PathBuf),
}

/// One decided grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// The package digest the user saw.
    pub digest: String,
    /// The grant lines the user allowed, sorted.
    pub lines: BTreeSet<String>,
    /// Decided for a `link`ed development package, whose digest moves on
    /// every rebuild, so only its lines are checked.
    pub dev: bool,
}

/// Every grant the user decided, by scope and plugin id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Grants {
    grants: BTreeMap<(Scope, String), Grant>,
}

impl Grants {
    /// The grant for `id` in `scope`, if any.
    pub fn get(&self, scope: &Scope, id: &str) -> Option<&Grant> {
        self.grants.get(&(scope.clone(), id.to_owned()))
    }

    /// Records `grant`, replacing an earlier one for the same plugin.
    pub fn insert(&mut self, scope: Scope, id: &str, grant: Grant) -> Result<(), Error> {
        let key = (scope, id.to_owned());
        if !self.grants.contains_key(&key) && self.grants.len() >= MAX_GRANTS {
            return Err(Error::TooManyGrants { max: MAX_GRANTS });
        }
        self.grants.insert(key, grant);
        Ok(())
    }

    /// Removes the grant and returns it.
    pub fn remove(&mut self, scope: &Scope, id: &str) -> Option<Grant> {
        self.grants.remove(&(scope.clone(), id.to_owned()))
    }
}

/// Whether a package may run under the grant on file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The digest matches and every requested line is granted.
    Granted,
    /// No grant, changed bytes, or more lines than were granted.
    NeedsApproval {
        /// Requested and not granted.
        added: Vec<String>,
        /// Granted and no longer requested.
        removed: Vec<String>,
    },
}

/// Checks the requested `lines` at `digest` against `stored`. Asking for
/// fewer lines never asks again; changed bytes always do.
pub fn check(lines: &[String], digest: &str, dev: bool, stored: Option<&Grant>) -> Verdict {
    let Some(grant) = stored else {
        return Verdict::NeedsApproval {
            added: lines.to_vec(),
            removed: Vec::new(),
        };
    };
    let added: Vec<String> = lines
        .iter()
        .filter(|line| !covered(line, &grant.lines))
        .cloned()
        .collect();
    let digest_ok = grant.digest == digest || (dev && grant.dev);
    if digest_ok && added.is_empty() {
        return Verdict::Granted;
    }
    let removed = grant
        .lines
        .iter()
        .filter(|line| !lines.contains(line))
        .cloned()
        .collect();
    Verdict::NeedsApproval { added, removed }
}

const MODEL_CHEAP: &str = "model:cheap";
const MODEL_CODE: &str = "model:code";

/// The model tier is the one ordered line: a `code` grant covers `cheap`.
fn covered(line: &str, granted: &BTreeSet<String>) -> bool {
    granted.contains(line) || (line == MODEL_CHEAP && granted.contains(MODEL_CODE))
}

// cox renders these lines in `cox-plugin`'s `grant::capability_list`, which
// sits behind tokio, reqwest and cox's store; until it moves into
// `cox-plugin-api`, this is the same rendering, line for line, so a Kerf
// grant reads like a cox one. Every manifest field that widens what a
// plugin can do must yield a line, or it would be allowed unseen.
/// The manifest's requests, one sorted line each: the unit of approval.
pub fn grant_lines(manifest: &PluginManifest) -> Vec<String> {
    let caps = &manifest.capabilities;
    let mut out = BTreeSet::new();
    let lists = [
        ("events", &caps.events),
        ("hooks", &caps.hooks),
        ("tools", &caps.tools),
        ("invoke", &caps.invoke),
        ("net", &caps.net),
        ("fs.read", &caps.fs.read),
        ("fs.write", &caps.fs.write),
        ("decide", &caps.decide),
        ("ui.render", &caps.ui.render),
    ];
    for (kind, items) in lists {
        out.extend(items.iter().map(|item| format!("{kind}:{item}")));
    }
    let flags = [
        ("wasi", manifest.wasi),
        ("context", caps.context),
        ("kv", caps.kv),
        ("ui.status", caps.ui.status),
        ("ui.panel", caps.ui.panel),
        ("ui.overlay", caps.ui.overlay),
        ("ui.commands", caps.ui.commands),
        ("terminal.keys", caps.terminal_keys()),
        ("desktop.inspector", caps.desktop.inspector),
        ("desktop.toolbar", caps.desktop.toolbar),
        ("desktop.palette", caps.desktop.palette),
        ("desktop.notify", caps.desktop.notify),
        ("desktop.canvas", caps.desktop.canvas),
    ];
    out.extend(
        flags
            .iter()
            .filter(|(_, on)| *on)
            .map(|(name, _)| (*name).to_owned()),
    );
    out.extend(caps.ide.grant_lines());
    match caps.model {
        Some(ModelTier::Cheap) => out.insert(MODEL_CHEAP.to_owned()),
        Some(ModelTier::Code) => out.insert(MODEL_CODE.to_owned()),
        None => false,
    };
    for p in &manifest.provider {
        let key = p.api_key_env.as_deref().unwrap_or("-");
        out.insert(format!("provider:{} {} key={key}", p.name, p.base_url));
    }
    for m in &manifest.mcp {
        let target = m
            .url
            .clone()
            .unwrap_or_else(|| argv(m.command.as_deref().unwrap_or(""), &m.args));
        out.insert(format!("mcp:{} {target}", m.name));
    }
    for ls in &manifest.language_servers {
        out.insert(format!("lsp:{} {}", ls.name, argv(&ls.command, &ls.args)));
    }
    for dap in &manifest.debug_adapters {
        out.insert(format!(
            "dap:{} {}",
            dap.name,
            argv(&dap.command, &dap.args)
        ));
    }
    for a in &manifest.external_agents {
        let argv = argv(&a.command, &a.args);
        out.insert(format!("agent:{} {argv} key={}", a.name, a.key_env));
    }
    for a in &manifest.cloud_agents {
        let model = a.model.as_deref().unwrap_or("default");
        out.insert(format!(
            "cloud-agent:{} {} host={} key={} model={model}; sends the prompt and the repository's GitHub URL; \
             the service clones the repository and pushes a branch on its servers, billed to your plan",
            a.name,
            a.backend.name(),
            a.backend.host(),
            a.key_env,
        ));
    }
    for a in &manifest.agents {
        out.insert(format!("subagent:{} {}", a.name, a.file));
    }
    for s in &manifest.skills {
        out.insert(format!("skill:{} {}", s.name, s.dir));
    }
    out.into_iter().collect()
}

fn argv(command: &str, args: &[String]) -> String {
    std::iter::once(command)
        .chain(args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A line as the prompt shows it. The text is the plugin's, so control and
/// bidirectional-override characters are escaped: they could hide or reorder
/// part of a command the user is approving.
pub fn display(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for c in line.chars() {
        let hiding = c.is_control()
            || matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
        if hiding {
            out.push_str(&format!("\\u{{{:x}}}", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
}
