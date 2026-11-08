// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Activation triggers, derived from the manifest's declarative tables so the
//! core knows when to load a plugin without running any of its code
//! (`docs/architecture.md` §3.4).

use cox_plugin_api::PluginManifest;

/// Something in the IDE that may activate a dormant plugin.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Trigger {
    /// A buffer of this language id opened (`[[language_servers]] languages`).
    Language(String),
    /// A debug session for this language id started
    /// (`[[debug_adapters]] languages`).
    Debug(String),
}

/// Every trigger `manifest` declares, sorted and without duplicates.
///
/// Capabilities with no static trigger of their own (tools, hooks, events,
/// commands) are not listed: they activate on first use, which
/// [`crate::Plugins::call`] covers.
pub fn triggers(manifest: &PluginManifest) -> Vec<Trigger> {
    let languages = manifest
        .language_servers
        .iter()
        .flat_map(|server| &server.languages)
        .map(|lang| Trigger::Language(lang.clone()));
    let debug = manifest
        .debug_adapters
        .iter()
        .flat_map(|adapter| &adapter.languages)
        .map(|lang| Trigger::Debug(lang.clone()));
    let mut all: Vec<Trigger> = languages.chain(debug).collect();
    all.sort();
    all.dedup();
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triggers_come_from_both_tables_deduplicated() {
        let manifest: PluginManifest = toml::from_str(
            r#"
            api = 1
            id = "rust-tools"
            version = "0.1.0"
            name = "Rust tools"
            [[language_servers]]
            name = "ra"
            command = "rust-analyzer"
            languages = ["rust"]
            [[language_servers]]
            name = "ra-alt"
            command = "rust-analyzer"
            languages = ["rust", "toml"]
            [[debug_adapters]]
            name = "lldb"
            command = "codelldb"
            languages = ["rust"]
            "#,
        )
        .unwrap();
        assert_eq!(
            triggers(&manifest),
            [
                Trigger::Language("rust".into()),
                Trigger::Language("toml".into()),
                Trigger::Debug("rust".into()),
            ]
        );
    }
}
