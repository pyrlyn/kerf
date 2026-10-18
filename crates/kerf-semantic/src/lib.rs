// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Kerf's semantic layer: a tree-sitter tree per open file, re-parsed
//! incrementally from [`kerf_core::EditEvent`]s, and a symbol index
//! (definitions and references) that re-indexes only the edited file.
//!
//! Rust is the only language so far. File text comes from repositories and is
//! untrusted: file size, open files, symbols per file and query state all have
//! hard caps.

// Tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod document;
mod error;
mod index;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::ops::Range;
use std::path::{Path, PathBuf};

use kerf_core::EditEvent;
use tree_sitter::Parser;

use document::Document;
pub use error::Error;
pub use index::{FileSymbols, Role, Symbol};
use index::{Index, TagsQuery};

/// Largest file the layer parses. Bigger files are refused, not truncated: a
/// half-parsed file would index symbols that are not there.
pub const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
/// Most files open at once.
pub const MAX_FILES: usize = 50_000;
/// Most symbols kept per file; the rest are dropped and the file is marked
/// [`truncated`](FileSymbols::truncated).
pub const MAX_SYMBOLS_PER_FILE: usize = 100_000;

/// Open files, their trees and the symbol index.
pub struct Semantic {
    parser: Parser,
    query: TagsQuery,
    docs: HashMap<PathBuf, Document>,
    index: Index,
}

impl Semantic {
    /// Loads the Rust grammar and compiles its tags query.
    ///
    /// # Errors
    ///
    /// [`Error::Grammar`] when the grammar and the tree-sitter runtime disagree
    /// on the ABI or the query does not compile.
    pub fn new() -> Result<Self, Error> {
        let language = tree_sitter_rust::LANGUAGE.into();
        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| Error::Grammar(e.to_string()))?;
        let query = TagsQuery::rust(&language)?;
        Ok(Self {
            parser,
            query,
            docs: HashMap::new(),
            index: Index::default(),
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
        if path.extension().and_then(OsStr::to_str) != Some("rs") {
            return Err(Error::UnsupportedLanguage { path });
        }
        if !self.docs.contains_key(&path) && self.docs.len() >= MAX_FILES {
            return Err(Error::TooManyFiles { max: MAX_FILES });
        }
        let doc = Document::parse(&mut self.parser, source)?;
        self.index
            .update(&self.query, &path, &doc.tree, &doc.source);
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
        let changed = doc.edit(&mut self.parser, event, new_source)?;
        self.index.update(&self.query, path, &doc.tree, &doc.source);
        Ok(changed)
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
