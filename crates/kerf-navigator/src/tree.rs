// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The project navigator: a file tree read one folder at a time, on expand.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::Error;

/// Most entries listed per folder; the rest are dropped and the folder is
/// marked [`truncated`](Row::truncated).
pub const MAX_CHILDREN: usize = 10_000;
/// Most nodes loaded in one tree, across all folders expanded or filtered.
pub const MAX_NODES: usize = 200_000;
/// Most files in the project-wide list that Filter and Open Quickly search.
pub const MAX_FILES: usize = 200_000;

/// The invisible root; its children are the top-level rows.
const ROOT: NodeId = NodeId(0);

/// A node of one [`Navigator`]; stable for the navigator's life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(usize);

/// Folder or file. Folders sort first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    /// Can be expanded.
    Folder,
    /// Anything else, symlinks included: following them could loop.
    File,
}

/// One visible line of the navigator, top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The node, for actions.
    pub id: NodeId,
    /// The file name, lossily UTF-8.
    pub name: String,
    /// Folder or file.
    pub kind: NodeKind,
    /// Indent level; top-level rows are 0.
    pub depth: usize,
    /// The disclosure is open.
    pub expanded: bool,
    /// The selected row.
    pub selected: bool,
    /// The folder had more than [`MAX_CHILDREN`] entries, or the tree hit
    /// [`MAX_NODES`] while listing it.
    pub truncated: bool,
}

/// Every file in the project that ignore files leave in, relative to `root`.
#[derive(Debug, Clone, Default)]
pub struct FileList {
    /// The project root.
    pub root: PathBuf,
    /// Relative paths, in walk order.
    pub paths: Vec<PathBuf>,
    /// The project had more than [`MAX_FILES`] files.
    pub truncated: bool,
}

#[derive(Debug)]
struct Node {
    name: OsString,
    /// Lowercased lossy name: the sort key and what Filter matches.
    lower: String,
    kind: NodeKind,
    parent: Option<NodeId>,
    /// `None` until the folder is first listed.
    children: Option<Vec<NodeId>>,
    expanded: bool,
    truncated: bool,
}

/// The navigator's state: loaded nodes, disclosure, selection and filter.
#[derive(Debug)]
pub struct Navigator {
    root: PathBuf,
    nodes: Vec<Node>,
    selected: Option<NodeId>,
    /// Nodes the filter leaves visible; `None` when no filter is set.
    filter: Option<HashSet<NodeId>>,
    files: Option<FileList>,
}

/// A walker that honours `.gitignore`, `.ignore` and git excludes like
/// ripgrep, with two navigator choices: dotfiles such as `.github` show,
/// and a folder outside git (an export, a copied subtree) still means its
/// `.gitignore`. `.git` itself is never shown.
fn walker(dir: &Path) -> WalkBuilder {
    let mut builder = WalkBuilder::new(dir);
    builder
        .hidden(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git");
    builder
}

impl FileList {
    fn walk(root: &Path) -> Self {
        let mut list = Self {
            root: root.to_path_buf(),
            ..Self::default()
        };
        // Unreadable entries are skipped: one bad folder must not hide the rest.
        for entry in walker(root).build().flatten() {
            if entry.depth() == 0 || entry.file_type().is_none_or(|t| t.is_dir()) {
                continue;
            }
            if list.paths.len() >= MAX_FILES {
                list.truncated = true;
                break;
            }
            if let Ok(rel) = entry.path().strip_prefix(root) {
                list.paths.push(rel.to_path_buf());
            }
        }
        list
    }
}

impl Navigator {
    /// A navigator over `root` with its top level listed.
    ///
    /// # Errors
    ///
    /// [`Error::NotAFolder`] or [`Error::List`].
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, Error> {
        let root = root.into();
        if !root.is_dir() {
            return Err(Error::NotAFolder { path: root });
        }
        let mut nav = Self {
            root,
            nodes: vec![Node {
                name: OsString::new(),
                lower: String::new(),
                kind: NodeKind::Folder,
                parent: None,
                children: None,
                expanded: true,
                truncated: false,
            }],
            selected: None,
            filter: None,
            files: None,
        };
        nav.load(ROOT)?;
        Ok(nav)
    }

    /// The project root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The visible rows, top to bottom.
    #[must_use]
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        // A stack, not recursion: a deep tree must not overflow the host's thread.
        let mut stack: Vec<(NodeId, usize)> = self.visible(ROOT).rev().map(|c| (c, 0)).collect();
        while let Some((id, depth)) = stack.pop() {
            let node = &self.nodes[id.0];
            let expanded = node.kind == NodeKind::Folder
                && if self.filter.is_some() {
                    self.visible(id).next().is_some()
                } else {
                    node.expanded
                };
            rows.push(Row {
                id,
                name: node.name.to_string_lossy().into_owned(),
                kind: node.kind,
                depth,
                expanded,
                selected: self.selected == Some(id),
                truncated: node.truncated,
            });
            if expanded {
                stack.extend(self.visible(id).rev().map(|c| (c, depth + 1)));
            }
        }
        rows
    }

    /// Opens a folder, listing it the first time. `false` when `id` is not a
    /// collapsed folder.
    ///
    /// # Errors
    ///
    /// [`Error::List`] when the folder cannot be read; it stays collapsed.
    pub fn expand(&mut self, id: NodeId) -> Result<bool, Error> {
        match self.node(id) {
            Some(n) if n.kind == NodeKind::Folder && !n.expanded => {}
            _ => return Ok(false),
        }
        self.load(id)?;
        self.nodes[id.0].expanded = true;
        Ok(true)
    }

    /// Closes a folder; a selection inside it moves to the folder. `false`
    /// when `id` is not an expanded folder.
    pub fn collapse(&mut self, id: NodeId) -> bool {
        match self.node(id) {
            Some(n) if n.kind == NodeKind::Folder && n.expanded => {}
            _ => return false,
        }
        self.nodes[id.0].expanded = false;
        if self
            .selected
            .is_some_and(|s| self.ancestors(s).any(|a| a == id))
        {
            self.selected = Some(id);
        }
        true
    }

    /// Selects a node. `false` when it is unknown or already selected.
    pub fn select(&mut self, id: NodeId) -> bool {
        if self.node(id).is_none() || self.selected == Some(id) {
            return false;
        }
        self.selected = Some(id);
        true
    }

    /// The selected node.
    #[must_use]
    pub fn selected(&self) -> Option<NodeId> {
        self.selected
    }

    /// The node's path relative to the root, or `None` for an unknown id.
    #[must_use]
    pub fn relative_path(&self, id: NodeId) -> Option<PathBuf> {
        self.node(id)?;
        let mut names: Vec<&OsStr> = std::iter::once(id)
            .chain(self.ancestors(id))
            .filter(|&a| a != ROOT)
            .map(|a| self.nodes[a.0].name.as_os_str())
            .collect();
        names.reverse();
        Some(names.iter().collect())
    }

    /// Shows only nodes whose name contains `text` (ignoring case), with their
    /// folders, across the whole project; blank text clears the filter.
    /// Lists the folders on the way to each match.
    pub fn set_filter(&mut self, text: &str) {
        let needle = text.trim().to_lowercase();
        if needle.is_empty() {
            self.filter = None;
            return;
        }
        let files = self
            .files
            .take()
            .unwrap_or_else(|| FileList::walk(&self.root));
        let mut shown = HashSet::new();
        for rel in &files.paths {
            let mut chain = Vec::new();
            let mut at = ROOT;
            for part in rel {
                let Some(child) = self.child(at, part) else {
                    break;
                };
                chain.push(child);
                at = child;
                if self.nodes[child.0].lower.contains(&needle) {
                    shown.extend(chain.iter().copied());
                }
            }
        }
        self.files = Some(files);
        self.filter = Some(shown);
    }

    /// Every file in the project, walked once and kept until
    /// [`refresh_files`](Self::refresh_files).
    pub fn files(&mut self) -> &FileList {
        let root = &self.root;
        self.files.get_or_insert_with(|| FileList::walk(root))
    }

    /// Forgets the file list, e.g. after files were created or deleted.
    pub fn refresh_files(&mut self) {
        self.files = None;
    }

    fn node(&self, id: NodeId) -> Option<&Node> {
        (id != ROOT).then(|| self.nodes.get(id.0)).flatten()
    }

    fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        std::iter::successors(self.nodes[id.0].parent, |p| self.nodes[p.0].parent)
    }

    fn visible(&self, id: NodeId) -> impl DoubleEndedIterator<Item = NodeId> + '_ {
        self.nodes[id.0]
            .children
            .iter()
            .flatten()
            .copied()
            .filter(|c| self.filter.as_ref().is_none_or(|f| f.contains(c)))
    }

    /// The child of `parent` named `name`, listing `parent` if needed.
    fn child(&mut self, parent: NodeId, name: &OsStr) -> Option<NodeId> {
        self.load(parent).ok()?;
        let lower = name.to_string_lossy().to_lowercase();
        let children = self.nodes[parent.0].children.as_ref()?;
        [NodeKind::Folder, NodeKind::File]
            .into_iter()
            .find_map(|kind| {
                children
                    .binary_search_by(|c| {
                        let n = &self.nodes[c.0];
                        (n.kind, n.lower.as_str(), n.name.as_os_str()).cmp(&(
                            kind,
                            lower.as_str(),
                            name,
                        ))
                    })
                    .ok()
                    .map(|i| children[i])
            })
    }

    fn load(&mut self, id: NodeId) -> Result<(), Error> {
        if self.nodes[id.0].children.is_some() {
            return Ok(());
        }
        let dir = self
            .relative_path(id)
            .map_or_else(|| self.root.clone(), |rel| self.root.join(rel));
        // The walker skips a folder it cannot read; listing it first turns
        // that into an error the host can show.
        std::fs::read_dir(&dir).map_err(|source| Error::List {
            path: dir.clone(),
            source,
        })?;
        let mut found: Vec<(OsString, NodeKind)> = Vec::new();
        let mut truncated = false;
        for entry in walker(&dir).max_depth(Some(1)).build().flatten() {
            if entry.depth() == 0 {
                continue;
            }
            if found.len() >= MAX_CHILDREN || self.nodes.len() + found.len() >= MAX_NODES {
                truncated = true;
                break;
            }
            let kind = if entry.file_type().is_some_and(|t| t.is_dir()) {
                NodeKind::Folder
            } else {
                NodeKind::File
            };
            found.push((entry.file_name().to_os_string(), kind));
        }
        let mut children: Vec<NodeId> = Vec::with_capacity(found.len());
        for (name, kind) in found {
            children.push(NodeId(self.nodes.len()));
            self.nodes.push(Node {
                lower: name.to_string_lossy().to_lowercase(),
                name,
                kind,
                parent: Some(id),
                children: None,
                expanded: false,
                truncated: false,
            });
        }
        children.sort_by(|a, b| {
            let (a, b) = (&self.nodes[a.0], &self.nodes[b.0]);
            (a.kind, &a.lower, &a.name).cmp(&(b.kind, &b.lower, &b.name))
        });
        let node = &mut self.nodes[id.0];
        node.children = Some(children);
        node.truncated = truncated;
        Ok(())
    }
}
