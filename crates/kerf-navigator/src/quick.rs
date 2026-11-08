// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Open Quickly: files and symbol definitions ranked by fuzzy match, with
//! the matched ranges a host highlights and a keyboard selection.

use std::ops::Range;
use std::path::{Path, PathBuf};

use kerf_semantic::{Role, Semantic};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::FileList;

/// Most hits one query returns.
pub const MAX_HITS: usize = 200;

/// What a hit opens. Symbols rank before files on a tie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HitKind {
    /// A definition from the semantic index.
    Symbol,
    /// A project file.
    File,
}

/// One ranked result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// Symbol or file.
    pub kind: HitKind,
    /// What was matched: the symbol name, or the file's relative path.
    pub label: String,
    /// The file, relative to the project root.
    pub path: PathBuf,
    /// The symbol's tags-query kind (`function`, `class`, …); empty for a file.
    pub detail: String,
    /// The symbol's 0-based line; 0 for a file.
    pub line: usize,
    /// nucleo's score; higher is better.
    pub score: u32,
    /// Matched UTF-8 byte ranges of `label`, sorted, adjacent ones merged.
    pub ranges: Vec<Range<usize>>,
}

/// Open Quickly's state: the last query's hits and the selected one.
#[derive(Debug)]
pub struct OpenQuickly {
    /// Path bonuses (after `/`) for files; plain word bonuses for symbols,
    /// where `_` and camelCase are the boundaries.
    paths: Matcher,
    names: Matcher,
    hits: Vec<Hit>,
    selected: usize,
}

impl Default for OpenQuickly {
    fn default() -> Self {
        Self {
            paths: Matcher::new(Config::DEFAULT.match_paths()),
            names: Matcher::new(Config::DEFAULT),
            hits: Vec::new(),
            selected: 0,
        }
    }
}

impl OpenQuickly {
    /// Ranks `files` and the definitions `semantic` holds for them against
    /// `query`, ignoring case as Xcode does, keeps the best [`MAX_HITS`] and
    /// selects the first. A blank query has no hits.
    pub fn query(&mut self, query: &str, files: &FileList, semantic: Option<&Semantic>) -> &[Hit] {
        self.hits.clear();
        self.selected = 0;
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        if pattern.atoms.is_empty() {
            return &self.hits;
        }
        let mut buf = Vec::new();
        let mut found = Vec::new();
        for rel in &files.paths {
            let label = rel.to_string_lossy();
            if let Some(score) = pattern.score(Utf32Str::new(&label, &mut buf), &mut self.paths) {
                found.push(hit(HitKind::File, label.into_owned(), rel, "", 0, score));
            }
            let symbols = semantic.and_then(|s| s.symbols(&files.root.join(rel)));
            for s in symbols.iter().flat_map(|f| &f.symbols) {
                if s.role != Role::Definition {
                    continue;
                }
                if let Some(score) =
                    pattern.score(Utf32Str::new(&s.name, &mut buf), &mut self.names)
                {
                    found.push(hit(
                        HitKind::Symbol,
                        s.name.clone(),
                        rel,
                        &s.kind,
                        s.line,
                        score,
                    ));
                }
            }
            // Bound memory on a one-letter query over a big project.
            if found.len() >= 2 * MAX_HITS {
                rank(&mut found);
            }
        }
        rank(&mut found);
        let mut chars = Vec::new();
        for h in &mut found {
            let matcher = match h.kind {
                HitKind::File => &mut self.paths,
                HitKind::Symbol => &mut self.names,
            };
            chars.clear();
            pattern.indices(Utf32Str::new(&h.label, &mut buf), matcher, &mut chars);
            h.ranges = byte_ranges(&h.label, &mut chars);
        }
        self.hits = found;
        &self.hits
    }

    /// The last query's hits, best first.
    #[must_use]
    pub fn hits(&self) -> &[Hit] {
        &self.hits
    }

    /// Index of the selected hit; `None` when there are no hits.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        (!self.hits.is_empty()).then_some(self.selected)
    }

    /// Moves the selection by `delta` rows (arrow keys, page keys), stopping
    /// at the ends. `false` when it did not move.
    pub fn move_selection(&mut self, delta: isize) -> bool {
        let Some(last) = self.hits.len().checked_sub(1) else {
            return false;
        };
        let to = self.selected.saturating_add_signed(delta).min(last);
        let moved = to != self.selected;
        self.selected = to;
        moved
    }
}

fn hit(kind: HitKind, label: String, path: &Path, detail: &str, line: usize, score: u32) -> Hit {
    Hit {
        kind,
        label,
        path: path.to_path_buf(),
        detail: detail.to_owned(),
        line,
        score,
        ranges: Vec::new(),
    }
}

/// Best first; shorter labels win ties, then symbols, then a stable order.
fn rank(hits: &mut Vec<Hit>) {
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.label.len().cmp(&b.label.len()))
            .then(a.kind.cmp(&b.kind))
            .then_with(|| (&a.label, &a.path, a.line).cmp(&(&b.label, &b.path, b.line)))
    });
    hits.truncate(MAX_HITS);
}

/// nucleo's char indices as merged UTF-8 byte ranges of `text`.
fn byte_ranges(text: &str, chars: &mut Vec<u32>) -> Vec<Range<usize>> {
    chars.sort_unstable();
    chars.dedup();
    let mut wanted = chars.iter().copied().peekable();
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for (i, (start, c)) in text.char_indices().enumerate() {
        if wanted
            .next_if(|&w| usize::try_from(w).is_ok_and(|w| w == i))
            .is_none()
        {
            continue;
        }
        let end = start + c.len_utf8();
        match ranges.last_mut() {
            Some(r) if r.end == start => r.end = end,
            _ => ranges.push(start..end),
        }
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::byte_ranges;

    #[test]
    fn char_indices_become_merged_byte_ranges() {
        assert_eq!(byte_ranges("añbc", &mut vec![3, 1, 0, 1]), [0..3, 4..5]);
    }
}
