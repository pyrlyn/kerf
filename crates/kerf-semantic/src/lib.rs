// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Kerf's semantic layer: a tree-sitter tree per open file, re-parsed
//! incrementally from [`kerf_core::EditEvent`]s, and a symbol index
//! (definitions and references) that re-indexes only the edited file.
//!
//! Rust and Python so far. Language servers add what tree-sitter cannot see
//! ([`Semantic::merge_lsp`]); every symbol carries its [`Source`]. File text
//! and server answers are untrusted: file size, open files, symbols per file,
//! query state and answer scanning all have hard caps.

// Tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod document;
mod error;
mod index;
mod lsp;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::ops::Range;
use std::path::{Path, PathBuf};

use gen_lsp_types::{DocumentSymbol, PositionEncodingKind};
use kerf_core::EditEvent;
use tree_sitter::{Language, Parser};

use document::Document;
pub use error::Error;
pub use index::{FileSymbols, Role, Source, Symbol};
use index::{Index, TagsQuery};
pub use lsp::MergeReport;

/// Largest file the layer parses. Bigger files are refused, not truncated: a
/// half-parsed file would index symbols that are not there.
pub const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
/// Most files open at once.
pub const MAX_FILES: usize = 50_000;
/// Most symbols kept per file; the rest are dropped and the file is marked
/// [`truncated`](FileSymbols::truncated).
pub const MAX_SYMBOLS_PER_FILE: usize = 100_000;

/// One language: the file extensions it claims, its parser and tags query.
struct Grammar {
    extensions: &'static [&'static str],
    parser: Parser,
    query: TagsQuery,
}

impl Grammar {
    fn new(
        extensions: &'static [&'static str],
        language: &Language,
        query: impl FnOnce(&Language) -> Result<TagsQuery, Error>,
    ) -> Result<Self, Error> {
        let mut parser = Parser::new();
        parser
            .set_language(language)
            .map_err(|e| Error::Grammar(e.to_string()))?;
        Ok(Self {
            extensions,
            parser,
            query: query(language)?,
        })
    }
}

/// Open files, their trees and the symbol index.
pub struct Semantic {
    grammars: Vec<Grammar>,
    docs: HashMap<PathBuf, Document>,
    index: Index,
    /// Source of [`Semantic::version`]s; never reused, so reopening a file
    /// cannot make an old answer look current.
    next_version: u64,
}

impl Semantic {
    /// Loads the grammars and compiles their tags queries.
    ///
    /// # Errors
    ///
    /// [`Error::Grammar`] when a grammar and the tree-sitter runtime disagree
    /// on the ABI or a query does not compile.
    pub fn new() -> Result<Self, Error> {
        let grammars = vec![
            Grammar::new(&["rs"], &tree_sitter_rust::LANGUAGE.into(), TagsQuery::rust)?,
            Grammar::new(&["py", "pyi"], &tree_sitter_python::LANGUAGE.into(), |l| {
                TagsQuery::new(l, tree_sitter_python::TAGS_QUERY)
            })?,
        ];
        Ok(Self {
            grammars,
            docs: HashMap::new(),
            index: Index::default(),
            next_version: 0,
        })
    }

    /// Parses and indexes a file, replacing it if it is already open.
    ///
    /// # Errors
    ///
    /// [`Error::UnsupportedLanguage`], [`Error::FileTooLarge`],
    /// [`Error::TooManyFiles`] or [`Error::Parse`]; nothing changes then.
    pub fn open(&mut self, path: impl Into<PathBuf>, source: String) -> Result<(), Error> {
        let path = path.into();
        let ext = path.extension().and_then(OsStr::to_str);
        let Some(grammar) = self
            .grammars
            .iter()
            .position(|g| ext.is_some_and(|e| g.extensions.contains(&e)))
        else {
            return Err(Error::UnsupportedLanguage { path });
        };
        if !self.docs.contains_key(&path) && self.docs.len() >= MAX_FILES {
            return Err(Error::TooManyFiles { max: MAX_FILES });
        }
        let version = self.next_version + 1;
        let g = &mut self.grammars[grammar];
        let doc = Document::parse(&mut g.parser, grammar, version, source)?;
        self.next_version = version;
        self.index.update(&g.query, &path, &doc.tree, &doc.source);
        self.docs.insert(path, doc);
        Ok(())
    }

    /// Applies one edit: `new_source` must be the file's text after `event`.
    /// Re-parses that file from its old tree, re-indexes only that file, and
    /// returns the byte ranges (post-edit) whose syntax changed.
    ///
    /// # Errors
    ///
    /// [`Error::NotOpen`], [`Error::BadEdit`] when the event does not fit the
    /// old or the new text, [`Error::FileTooLarge`] or [`Error::Parse`];
    /// the file keeps its old tree and symbols then.
    pub fn edit(
        &mut self,
        path: &Path,
        event: &EditEvent,
        new_source: String,
    ) -> Result<Vec<Range<usize>>, Error> {
        let Some(doc) = self.docs.get_mut(path) else {
            return Err(Error::NotOpen {
                path: path.to_path_buf(),
            });
        };
        let g = &mut self.grammars[doc.grammar];
        let changed = doc.edit(&mut g.parser, event, new_source)?;
        // Re-indexing from the tree alone also drops the file's language
        // server symbols: their positions belong to the old text.
        self.index.update(&g.query, path, &doc.tree, &doc.source);
        self.next_version += 1;
        doc.version = self.next_version;
        Ok(changed)
    }

    /// The file's current version, to send with a language-server request and
    /// pass back to [`Semantic::merge_lsp`] with the answer.
    #[must_use]
    pub fn version(&self, path: &Path) -> Option<u64> {
        self.docs.get(path).map(|d| d.version)
    }

    /// Adds a language server's answers about one file: definitions from
    /// `textDocument/documentSymbol` and reference ranges from
    /// `textDocument/references` (only locations in this file). tree-sitter
    /// symbols stay; an answer only adds a symbol tree-sitter did not find,
    /// tagged [`Source::LanguageServer`], until the file's next edit.
    ///
    /// # Errors
    ///
    /// [`Error::NotOpen`], [`Error::Stale`] when the file changed after
    /// `version`, or [`Error::UnsupportedEncoding`]; nothing changes then.
    pub fn merge_lsp(
        &mut self,
        path: &Path,
        version: u64,
        encoding: &PositionEncodingKind,
        definitions: &[DocumentSymbol],
        references: &[gen_lsp_types::Range],
    ) -> Result<MergeReport, Error> {
        let Some(doc) = self.docs.get(path) else {
            return Err(Error::NotOpen {
                path: path.to_path_buf(),
            });
        };
        if doc.version != version {
            return Err(Error::Stale {
                path: path.to_path_buf(),
                answered: version,
                current: doc.version,
            });
        }
        let (found, mut report) = lsp::symbols(&doc.source, encoding, definitions, references)?;
        let (added, known) = self.index.merge(path, found);
        report.added = added;
        report.known = known;
        Ok(report)
    }

    /// Drops a file's tree and symbols. Returns whether it was open.
    pub fn close(&mut self, path: &Path) -> bool {
        self.index.remove(path);
        self.docs.remove(path).is_some()
    }

    /// One file's symbols, if it is open.
    #[must_use]
    pub fn symbols(&self, path: &Path) -> Option<&FileSymbols> {
        self.index.file(path)
    }

    /// Every definition named `name` across open files.
    #[must_use]
    pub fn definitions(&self, name: &str) -> Vec<(&Path, &Symbol)> {
        self.index.lookup(name, Role::Definition)
    }

    /// Every reference to `name` across open files.
    #[must_use]
    pub fn references(&self, name: &str) -> Vec<(&Path, &Symbol)> {
        self.index.lookup(name, Role::Reference)
    }

    /// The file's syntax tree as an S-expression, for tests and debugging.
    #[must_use]
    pub fn sexp(&self, path: &Path) -> Option<String> {
        self.docs.get(path).map(|d| d.tree.root_node().to_sexp())
    }
}

impl std::fmt::Debug for Semantic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Semantic")
            .field("files", &self.docs.len())
            .finish_non_exhaustive()
    }
}
