// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The headless host: buffers and the semantic layer of one workspace root,
//! driven by already-decoded requests.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use kerf_core::{EditEvent, TextBuffer};
use kerf_semantic::Semantic;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_json::value::RawValue;

use crate::error::RemoteError;
use crate::protocol::{
    ChangedResult, EditParams, LenResult, Limits, LookupParams, LookupResult, PathParams,
    ReadResult, SymbolInfo, SymbolRole, method,
};

/// Owns the open buffers and their index under one workspace root.
#[derive(Debug)]
pub struct Host {
    root: PathBuf,
    limits: Limits,
    buffers: HashMap<PathBuf, TextBuffer>,
    semantic: Semantic,
}

impl Host {
    /// A host serving files under `root`.
    ///
    /// # Errors
    ///
    /// The semantic layer's grammar failed to load.
    pub fn new(root: impl Into<PathBuf>, limits: Limits) -> Result<Self, kerf_semantic::Error> {
        Ok(Self {
            root: root.into(),
            limits,
            buffers: HashMap::new(),
            semantic: Semantic::new()?,
        })
    }

    /// The caps this host enforces.
    #[must_use]
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// Run one buffer or index method.
    pub(crate) fn call(
        &mut self,
        method: &str,
        params: Option<&RawValue>,
    ) -> Result<Value, RemoteError> {
        match method {
            method::OPEN => reply(self.open(&parse::<PathParams>(params)?.path)?),
            method::READ => {
                let path = parse::<PathParams>(params)?.path;
                let text = self.buffer(&path)?.1.to_text();
                reply(ReadResult { text })
            }
            method::EDIT => reply(self.edit(parse(params)?)?),
            method::UNDO => reply(self.history(&parse::<PathParams>(params)?.path, true)?),
            method::REDO => reply(self.history(&parse::<PathParams>(params)?.path, false)?),
            method::CLOSE => {
                let rel = relative(&parse::<PathParams>(params)?.path)?;
                self.semantic.close(&rel);
                reply(ChangedResult {
                    changed: self.buffers.remove(&rel).is_some(),
                })
            }
            method::LOOKUP => reply(self.lookup(&parse(params)?)),
            other => Err(RemoteError::MethodNotFound(other.to_owned())),
        }
    }

    fn open(&mut self, path: &str) -> Result<LenResult, RemoteError> {
        let rel = relative(path)?;
        if let Some(buf) = self.buffers.get(&rel) {
            return Ok(LenResult {
                len_bytes: buf.len_bytes(),
            });
        }
        if self.buffers.len() >= self.limits.max_buffers {
            return Err(RemoteError::TooManyBuffers {
                cap: self.limits.max_buffers,
            });
        }
        let text = self.read_file(path, &rel)?;
        // A file the layer cannot index (another language, a parse failure)
        // still opens as text: the index is extra, editing is not.
        let _ = self.semantic.open(rel.clone(), text.clone());
        let buf = TextBuffer::with_text(&text);
        let len_bytes = buf.len_bytes();
        self.buffers.insert(rel, buf);
        Ok(LenResult { len_bytes })
    }

    fn edit(&mut self, p: EditParams) -> Result<LenResult, RemoteError> {
        let cap = self.limits.max_buffer_bytes;
        let (rel, buf) = self.buffer(&p.path)?;
        let kept = buf
            .len_bytes()
            .saturating_sub(p.end.saturating_sub(p.start));
        if kept.saturating_add(p.text.len()) > cap {
            return Err(RemoteError::BufferTooLarge { cap });
        }
        let event = buf.apply(p.start..p.end, &p.text)?;
        let len_bytes = buf.len_bytes();
        self.reindex(&rel, &event);
        Ok(LenResult { len_bytes })
    }

    fn history(&mut self, path: &str, undo: bool) -> Result<ChangedResult, RemoteError> {
        let (rel, buf) = self.buffer(path)?;
        let event = if undo { buf.undo() } else { buf.redo() };
        if let Some(event) = &event {
            self.reindex(&rel, event);
        }
        Ok(ChangedResult {
            changed: event.is_some(),
        })
    }

    fn lookup(&self, p: &LookupParams) -> LookupResult {
        let found = match p.role {
            SymbolRole::Definition => self.semantic.definitions(&p.name),
            SymbolRole::Reference => self.semantic.references(&p.name),
        };
        let truncated = found.len() > self.limits.max_symbols;
        let symbols = found
            .into_iter()
            .take(self.limits.max_symbols)
            .map(|(path, s)| SymbolInfo {
                path: path.to_string_lossy().replace('\\', "/"),
                name: s.name.clone(),
                kind: s.kind.clone(),
                line: s.line,
                start: s.name_range.start,
                end: s.name_range.end,
            })
            .collect();
        LookupResult { symbols, truncated }
    }

    /// Keep the index in step with the buffer. A failed incremental update
    /// drops the file's symbols and re-parses it whole, so a lookup never
    /// answers from a stale tree.
    fn reindex(&mut self, rel: &Path, event: &EditEvent) {
        let Some(text) = self.buffers.get(rel).map(TextBuffer::to_text) else {
            return;
        };
        if self.semantic.symbols(rel).is_some()
            && self.semantic.edit(rel, event, text.clone()).is_ok()
        {
            return;
        }
        self.semantic.close(rel);
        let _ = self.semantic.open(rel.to_path_buf(), text);
    }

    fn buffer(&mut self, path: &str) -> Result<(PathBuf, &mut TextBuffer), RemoteError> {
        let rel = relative(path)?;
        match self.buffers.get_mut(&rel) {
            Some(buf) => Ok((rel, buf)),
            None => Err(RemoteError::NotOpen(path.to_owned())),
        }
    }

    /// Reads at most one byte past the cap, so a huge file is refused
    /// without being loaded.
    fn read_file(&self, path: &str, rel: &Path) -> Result<String, RemoteError> {
        let cap = self.limits.max_buffer_bytes;
        let fail = |source| RemoteError::Io {
            path: path.to_owned(),
            source,
        };
        let limit = u64::try_from(cap).unwrap_or(u64::MAX).saturating_add(1);
        let mut bytes = Vec::new();
        File::open(self.root.join(rel))
            .and_then(|f| f.take(limit).read_to_end(&mut bytes))
            .map_err(fail)?;
        if bytes.len() > cap {
            return Err(RemoteError::BufferTooLarge { cap });
        }
        String::from_utf8(bytes).map_err(|e| fail(io::Error::new(io::ErrorKind::InvalidData, e)))
    }
}

/// Only plain names: the root scopes the session. The UI's user already has
/// a shell on this machine, so the root is not a sandbox and symlinks are not
/// chased.
fn relative(path: &str) -> Result<PathBuf, RemoteError> {
    let p = Path::new(path);
    if p.as_os_str().is_empty() || !p.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(RemoteError::BadPath(path.to_owned()));
    }
    Ok(p.to_path_buf())
}

pub(crate) fn parse<T: DeserializeOwned>(params: Option<&RawValue>) -> Result<T, RemoteError> {
    let raw = params.map_or("null", RawValue::get);
    serde_json::from_str(raw).map_err(|e| RemoteError::InvalidParams(e.to_string()))
}

/// Serializing the result types cannot fail; the error arm only keeps the
/// shipped code free of `unwrap`.
pub(crate) fn reply(result: impl Serialize) -> Result<Value, RemoteError> {
    serde_json::to_value(result).map_err(|e| RemoteError::InvalidParams(e.to_string()))
}
