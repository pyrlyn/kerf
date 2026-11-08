// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The Workspace Trust prompt (`docs/architecture.md` §6): what would start
//! once a folder is trusted, read from manifests without running plugin code.

use kerf_plugin::Source;
use kerf_plugin::cox_plugin_api::PluginManifest;
use serde::Serialize;

/// Most items one prompt holds. Project plugins and their declarations are
/// repository content, so their number is untrusted.
pub const MAX_PROMPT_ITEMS: usize = 512;

/// What an [`Item`] would start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// A `[[language_servers]]` process.
    LanguageServer,
    /// A `[[debug_adapters]]` process.
    DebugAdapter,
    /// A task or build the folder defines.
    Task,
    /// A plugin from the folder's `.kerf/plugins/`.
    ProjectPlugin,
}

/// One line of the prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    /// What would start.
    pub kind: ItemKind,
    /// The plugin that declares it; `None` for a task.
    pub plugin: Option<String>,
    /// The declaration's name, or a project plugin's display name.
    pub name: String,
    /// The program exactly as declared. `None` for a project plugin, which
    /// is a sandboxed module, not a process.
    pub command: Option<String>,
    /// The arguments exactly as declared, one element per argument, so the
    /// sheet never has to guess where a quoted argument ends.
    pub args: Vec<String>,
}

/// A task or build a folder defines, as the prompt shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// The task's name.
    pub name: String,
    /// The program.
    pub command: String,
    /// Its arguments.
    pub args: Vec<String>,
}

/// Everything that would start in a folder once it is trusted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Prompt {
    /// Plugins first, in discovery order, then tasks.
    pub items: Vec<Item>,
    /// Items past [`MAX_PROMPT_ITEMS`], so the sheet can say how many more
    /// there are instead of hiding them.
    pub omitted: usize,
}

impl Prompt {
    /// Lists the processes and project plugins `plugins` declare, then
    /// `tasks`. User plugins appear only through their processes: they load
    /// in a restricted folder too, but their servers do not spawn there.
    pub fn build<'a>(
        plugins: impl IntoIterator<Item = (Source, &'a PluginManifest)>,
        tasks: &[Task],
    ) -> Self {
        let mut prompt = Self::default();
        for (source, manifest) in plugins {
            let plugin = || Some(manifest.id.clone());
            if source == Source::Project {
                prompt.push(|| Item {
                    kind: ItemKind::ProjectPlugin,
                    plugin: plugin(),
                    name: manifest.name.clone(),
                    command: None,
                    args: Vec::new(),
                });
            }
            for ls in &manifest.language_servers {
                prompt.push(|| Item {
                    kind: ItemKind::LanguageServer,
                    plugin: plugin(),
                    name: ls.name.clone(),
                    command: Some(ls.command.clone()),
                    args: ls.args.clone(),
                });
            }
            for da in &manifest.debug_adapters {
                prompt.push(|| Item {
                    kind: ItemKind::DebugAdapter,
                    plugin: plugin(),
                    name: da.name.clone(),
                    command: Some(da.command.clone()),
                    args: da.args.clone(),
                });
            }
        }
        for task in tasks {
            prompt.push(|| Item {
                kind: ItemKind::Task,
                plugin: None,
                name: task.name.clone(),
                command: Some(task.command.clone()),
                args: task.args.clone(),
            });
        }
        prompt
    }

    // Takes a closure so items past the cap are counted without being copied.
    fn push(&mut self, item: impl FnOnce() -> Item) {
        if self.items.len() < MAX_PROMPT_ITEMS {
            self.items.push(item());
        } else {
            self.omitted += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(text: &str) -> PluginManifest {
        let head = "api = 1\nid = \"rust-tools\"\nversion = \"0.1.0\"\nname = \"Rust tools\"\n";
        toml::from_str(&format!("{head}{text}")).unwrap()
    }

    #[test]
    fn the_prompt_lists_commands_verbatim_and_project_plugins() {
        let m = manifest(
            "[[language_servers]]\nname = \"ra\"\nlanguages = [\"rust\"]\ncommand = \"bin/rust-analyzer\"\nargs = [\"--log\", \"a b\"]\n\
             [[debug_adapters]]\nname = \"lldb\"\nlanguages = [\"rust\"]\ncommand = \"codelldb\"\nargs = [\"--port\", \"0\"]\n",
        );
        let task = Task {
            name: "build".into(),
            command: "cargo".into(),
            args: vec!["build".into()],
        };
        let prompt = Prompt::build([(Source::Project, &m)], std::slice::from_ref(&task));

        let kinds: Vec<ItemKind> = prompt.items.iter().map(|i| i.kind).collect();
        assert_eq!(
            kinds,
            [
                ItemKind::ProjectPlugin,
                ItemKind::LanguageServer,
                ItemKind::DebugAdapter,
                ItemKind::Task
            ]
        );
        let ra = &prompt.items[1];
        assert_eq!(ra.plugin.as_deref(), Some("rust-tools"));
        assert_eq!(ra.command.as_deref(), Some("bin/rust-analyzer"));
        assert_eq!(ra.args, ["--log", "a b"]);
        assert_eq!(prompt.items[3].command.as_deref(), Some("cargo"));
    }

    #[test]
    fn a_user_plugin_shows_only_its_processes() {
        let m =
            manifest("[[language_servers]]\nname = \"ra\"\nglobs = [\"*.rs\"]\ncommand = \"ra\"\n");
        let prompt = Prompt::build([(Source::User, &m)], &[]);
        assert_eq!(prompt.items.len(), 1);
        assert_eq!(prompt.items[0].kind, ItemKind::LanguageServer);
    }

    #[test]
    fn items_past_the_cap_are_counted() {
        let tasks = vec![
            Task {
                name: "t".into(),
                command: "true".into(),
                args: Vec::new(),
            };
            MAX_PROMPT_ITEMS + 3
        ];
        let prompt = Prompt::build([], &tasks);
        assert_eq!(prompt.items.len(), MAX_PROMPT_ITEMS);
        assert_eq!(prompt.omitted, 3);
    }
}
