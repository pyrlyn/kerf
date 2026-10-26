// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Definitions and references per file, from the grammar's tags query run on
//! the file's existing tree.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use tree_sitter::{Language, Query, QueryCursor, StreamingIterator, Tree};

use crate::{Error, MAX_SYMBOLS_PER_FILE};

/// Patterns the upstream Rust tags query misses: path-qualified and turbofish
/// calls (`Self::new()`, `mem::take::<T>()`), calls inside macro arguments,
/// the type in `impl Trait for Type`, trait method signatures, enum variants,
/// `const` and `static`. The scoped-call pattern comes from the token CLI's
/// `RUST_SCOPED_CALL`, which found the same gap.
///
/// Macro arguments are bare token trees, so a call there is only "a name
/// right before a `(…)` group"; that is a guess, but `format!`, `assert!` and
/// friends hold too many real calls to skip.
const RUST_EXTRA_TAGS: &str = r#"
(call_expression
    function: (scoped_identifier name: (identifier) @name)) @reference.call
(call_expression
    function: (generic_function function: (identifier) @name)) @reference.call
(call_expression
    function: (generic_function
        function: (scoped_identifier name: (identifier) @name))) @reference.call
(call_expression
    function: (generic_function
        function: (field_expression field: (field_identifier) @name))) @reference.call
((token_tree
    (identifier) @name
    .
    (token_tree) @reference.call)
    (#match? @reference.call "^\\("))
(impl_item trait: (_) type: (type_identifier) @name) @reference.implementation
(function_signature_item name: (identifier) @name) @definition.method
(enum_variant name: (identifier) @name) @definition.variant
(const_item name: (identifier) @name) @definition.constant
(static_item name: (identifier) @name) @definition.constant
"#;

/// In-progress matches tree-sitter may keep at once. Without a limit a
/// hostile file can make the cursor's state grow with its nesting depth.
const MATCH_LIMIT: u32 = 1024;

/// Whether a symbol is where a name is defined or where it is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// `fn foo`, `struct Foo`, `mod foo`, …
    Definition,
    /// `foo()`, `x.foo()`, `foo!()`, `impl Foo`, …
    Reference,
}

/// Who found a symbol. tree-sitter is the base; a language server only adds
/// what tree-sitter cannot see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// The grammar's tags query.
    TreeSitter,
    /// A language server's answer, merged by [`Semantic::merge_lsp`](crate::Semantic::merge_lsp).
    LanguageServer,
}

/// One definition or reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    /// The identifier.
    pub name: String,
    /// The tags-query kind: `function`, `method`, `class`, `call`, ….
    pub kind: String,
    /// Definition or reference.
    pub role: Role,
    /// Byte range of the identifier.
    pub name_range: Range<usize>,
    /// Byte range of the whole tagged node (a definition spans its body).
    pub range: Range<usize>,
    /// 0-based line of the identifier.
    pub line: usize,
    /// Who found it.
    pub source: Source,
}

/// Every symbol of one file, as of its last re-index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSymbols {
    /// Bumped across the whole index each time any file is re-indexed, so a
    /// file whose revision did not move was not touched.
    pub revision: u64,
    /// Symbols in source order.
    pub symbols: Vec<Symbol>,
    /// True when the file had more than [`MAX_SYMBOLS_PER_FILE`] symbols and
    /// the rest were dropped.
    pub truncated: bool,
}

/// The compiled tags query and what each capture means.
#[derive(Debug)]
pub(crate) struct TagsQuery {
    query: Query,
    name_capture: u32,
    /// Per capture index: role and kind, or `None` for captures like `@name`.
    roles: Vec<Option<(Role, String)>>,
}

impl TagsQuery {
    pub(crate) fn rust(language: &Language) -> Result<Self, Error> {
        Self::new(
            language,
            &format!("{}\n{RUST_EXTRA_TAGS}", tree_sitter_rust::TAGS_QUERY),
        )
    }

    pub(crate) fn new(language: &Language, source: &str) -> Result<Self, Error> {
        let query = Query::new(language, source).map_err(|e| Error::Grammar(e.to_string()))?;
        let names = query.capture_names();
        let name_capture = names
            .iter()
            .position(|n| *n == "name")
            .and_then(|i| u32::try_from(i).ok())
            .ok_or_else(|| Error::Grammar("tags query has no @name capture".into()))?;
        let roles = names
            .iter()
            .map(|n| {
                if let Some(kind) = n.strip_prefix("definition.") {
                    Some((Role::Definition, kind.to_string()))
                } else {
                    n.strip_prefix("reference.")
                        .map(|kind| (Role::Reference, kind.to_string()))
                }
            })
            .collect();
        Ok(Self {
            query,
            name_capture,
            roles,
        })
    }

    /// Symbols of one parsed file, in source order.
    fn extract(&self, tree: &Tree, source: &str) -> (Vec<Symbol>, bool) {
        let mut cursor = QueryCursor::new();
        cursor.set_match_limit(MATCH_LIMIT);
        // One identifier can match several patterns (a method is also a
        // function); the earliest pattern is the most specific one.
        let mut best: HashMap<(usize, usize), (usize, Symbol)> = HashMap::new();
        let mut truncated = false;
        let mut matches = cursor.matches(&self.query, tree.root_node(), source.as_bytes());
        while let Some(m) = matches.next() {
            let mut name = None;
            let mut tagged = None;
            for capture in m.captures() {
                if capture.index == self.name_capture {
                    name = Some(capture.node);
                } else if let Some(Some(role)) = self.roles.get(capture.index as usize) {
                    tagged = Some((role, capture.node));
                }
            }
            let (Some(name), Some(((role, kind), node))) = (name, tagged) else {
                continue;
            };
            let Some(text) = source.get(name.byte_range()) else {
                continue;
            };
            let key = (name.start_byte(), name.end_byte());
            if best.get(&key).is_some_and(|(p, _)| *p <= m.pattern_index) {
                continue;
            }
            if !best.contains_key(&key) && best.len() >= MAX_SYMBOLS_PER_FILE {
                truncated = true;
                break;
            }
            let symbol = Symbol {
                name: text.to_string(),
                kind: kind.clone(),
                role: *role,
                name_range: name.byte_range(),
                range: node.byte_range(),
                line: name.start_position().row,
                source: Source::TreeSitter,
            };
            best.insert(key, (m.pattern_index, symbol));
        }
        let mut symbols: Vec<Symbol> = best.into_values().map(|(_, s)| s).collect();
        symbols.sort_by_key(|s| (s.name_range.start, s.name_range.end));
        (symbols, truncated || cursor.did_exceed_match_limit())
    }
}

/// Symbols of every open file, with a name lookup across files.
#[derive(Debug, Default)]
pub(crate) struct Index {
    files: HashMap<PathBuf, FileSymbols>,
    /// Which files hold at least one symbol with this name.
    by_name: HashMap<String, BTreeSet<PathBuf>>,
    revision: u64,
}

impl Index {
    /// Replaces one file's symbols; no other file is read or written.
    pub(crate) fn update(&mut self, query: &TagsQuery, path: &Path, tree: &Tree, source: &str) {
        let (symbols, truncated) = query.extract(tree, source);
        self.remove(path);
        for symbol in &symbols {
            self.by_name
                .entry(symbol.name.clone())
                .or_default()
                .insert(path.to_path_buf());
        }
        self.revision += 1;
        let file = FileSymbols {
            revision: self.revision,
            symbols,
            truncated,
        };
        self.files.insert(path.to_path_buf(), file);
    }

    /// Adds symbols a language server found to an indexed file. A symbol at a
    /// name range the file already has is skipped, so tree-sitter's stays.
    /// Returns how many were added and how many were already known.
    pub(crate) fn merge(&mut self, path: &Path, found: Vec<Symbol>) -> (usize, usize) {
        let Some(file) = self.files.get_mut(path) else {
            return (0, 0);
        };
        let mut seen: HashSet<(usize, usize)> = file
            .symbols
            .iter()
            .map(|s| (s.name_range.start, s.name_range.end))
            .collect();
        let (mut added, mut known) = (0, 0);
        for symbol in found {
            if !seen.insert((symbol.name_range.start, symbol.name_range.end)) {
                known += 1;
                continue;
            }
            if file.symbols.len() >= MAX_SYMBOLS_PER_FILE {
                file.truncated = true;
                break;
            }
            self.by_name
                .entry(symbol.name.clone())
                .or_default()
                .insert(path.to_path_buf());
            file.symbols.push(symbol);
            added += 1;
        }
        file.symbols
            .sort_by_key(|s| (s.name_range.start, s.name_range.end));
        self.revision += 1;
        file.revision = self.revision;
        (added, known)
    }

    pub(crate) fn remove(&mut self, path: &Path) {
        let Some(old) = self.files.remove(path) else {
            return;
        };
        for symbol in &old.symbols {
            if let Some(paths) = self.by_name.get_mut(&symbol.name) {
                paths.remove(path);
                if paths.is_empty() {
                    self.by_name.remove(&symbol.name);
                }
            }
        }
    }

    pub(crate) fn file(&self, path: &Path) -> Option<&FileSymbols> {
        self.files.get(path)
    }

    /// Symbols named `name` with `role`, by path, then source order.
    pub(crate) fn lookup(&self, name: &str, role: Role) -> Vec<(&Path, &Symbol)> {
        let Some(paths) = self.by_name.get(name) else {
            return Vec::new();
        };
        paths
            .iter()
            .filter_map(|p| self.files.get(p).map(|f| (p.as_path(), f)))
            .flat_map(|(p, f)| {
                f.symbols
                    .iter()
                    .filter(move |s| s.role == role && s.name == name)
                    .map(move |s| (p, s))
            })
            .collect()
    }
}
