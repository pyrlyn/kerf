// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

use std::path::Path;

use git2::{DiffOptions, Patch, Repository};

use crate::{Error, Group, MAX_DIFF_FILE_BYTES, MAX_DIFF_LINES, cut};

/// What a diff line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Unchanged.
    Context,
    /// Only on the new side.
    Added,
    /// Only on the old side.
    Removed,
}

/// One diff line, without its line break.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// What it is.
    pub kind: LineKind,
    /// 1-based line number on the old side, absent for an added line.
    pub old_line: Option<u32>,
    /// 1-based line number on the new side, absent for a removed line.
    pub new_line: Option<u32>,
    /// The text, lossily decoded and cut to [`crate::MAX_LINE_BYTES`].
    pub text: String,
}

/// One hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// The `@@ -a,b +c,d @@ context` line, without its line break.
    pub header: String,
    /// Its lines.
    pub lines: Vec<Line>,
}

/// One file's diff.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileDiff {
    /// The hunks; empty when nothing changed or the file is binary.
    pub hunks: Vec<Hunk>,
    /// Binary, or over [`MAX_DIFF_FILE_BYTES`]: no lines to show.
    pub binary: bool,
    /// Lines were dropped over [`MAX_DIFF_LINES`], or some were cut.
    pub truncated: bool,
}

pub(crate) fn file_diff(repo: &Repository, group: Group, path: &Path) -> Result<FileDiff, Error> {
    let mut opts = DiffOptions::new();
    // The host's path is a literal file name, never a glob.
    opts.pathspec(path)
        .disable_pathspec_match(true)
        .ignore_submodules(true)
        .max_size(i64::try_from(MAX_DIFF_FILE_BYTES).unwrap_or(i64::MAX));
    let diff = match group {
        Group::Staged => {
            let head = match repo.head() {
                Ok(head) => Some(head.peel_to_tree()?),
                Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
                Err(e) => return Err(e.into()),
            };
            repo.diff_tree_to_index(head.as_ref(), None, Some(&mut opts))?
        }
        Group::Changes => {
            opts.include_untracked(true)
                .recurse_untracked_dirs(true)
                .show_untracked_content(true);
            repo.diff_index_to_workdir(None, Some(&mut opts))?
        }
    };
    let mut out = FileDiff::default();
    if diff.deltas().len() == 0 {
        return Ok(out);
    }
    let Some(patch) = Patch::from_diff(&diff, 0)? else {
        out.binary = true;
        return Ok(out);
    };
    if patch.delta().flags().is_binary() {
        out.binary = true;
        return Ok(out);
    }
    let mut kept = 0;
    'hunks: for h in 0..patch.num_hunks() {
        let (hunk, count) = patch.hunk(h)?;
        let header = String::from_utf8_lossy(hunk.header());
        let (header, header_cut) = cut(header.trim_end_matches(['\r', '\n']));
        out.truncated |= header_cut;
        let mut lines = Vec::new();
        for l in 0..count {
            if kept == MAX_DIFF_LINES {
                out.truncated = true;
                out.hunks.push(Hunk { header, lines });
                break 'hunks;
            }
            let line = patch.line_in_hunk(h, l)?;
            let kind = match line.origin() {
                ' ' => LineKind::Context,
                '+' => LineKind::Added,
                '-' => LineKind::Removed,
                // "\ No newline at end of file" markers carry no text of the file.
                _ => continue,
            };
            kept += 1;
            let text = String::from_utf8_lossy(line.content());
            let (text, text_cut) = cut(text.trim_end_matches(['\r', '\n']));
            out.truncated |= text_cut;
            lines.push(Line {
                kind,
                old_line: line.old_lineno(),
                new_line: line.new_lineno(),
                text,
            });
        }
        out.hunks.push(Hunk { header, lines });
    }
    Ok(out)
}
