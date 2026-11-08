// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The Source control screen over `kerf-scm`: a [`KerfScm`] keeps the last
//! status and the last diff it loaded, and the host reads their rows by
//! index, so no Rust-owned list ever crosses the boundary.
//!
//! A refusal from the model comes back as the closest [`KerfStatus`]
//! (`NOTHING`, `TOO_LARGE`, `OUT_OF_RANGE`, else `IO`); `kerf_scm_error` gives
//! the exact [`KerfScmError`] and `kerf_scm_error_message` its text.

use std::path::Path;

use kerf_scm::{Change, Error, FileDiff, Group, LineKind, Scm, Status};

use crate::{KerfStatus, guard, put, text};

/// Longest path the host can pass.
const MAX_PATH_BYTES: usize = 64 * 1024;

/// The "Staged" group: HEAD against the index.
pub const KERF_SCM_GROUP_STAGED: u32 = 0;
/// The "Changes" group: the index against the working tree, untracked files
/// included.
pub const KERF_SCM_GROUP_CHANGES: u32 = 1;

/// Why the last `kerf_scm_*` call on a handle failed.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KerfScmError {
    /// The last call succeeded, or failed on its arguments before reaching
    /// the repository.
    None = 0,
    /// No repository holds the folder.
    NotARepository = 1,
    /// The repository has no working tree.
    Bare = 2,
    /// The path is empty, absolute, climbs out with `..`, or is too long.
    BadPath = 3,
    /// The commit message is empty once cleaned up.
    EmptyMessage = 4,
    /// The commit message is over its cap.
    MessageTooLarge = 5,
    /// Nothing is staged.
    NothingToCommit = 6,
    /// `user.name` or `user.email` is not configured.
    NoIdentity = 7,
    /// A merge, rebase or similar is in progress, or the index has conflicts.
    Busy = 8,
    /// The working tree could not be read.
    Io = 9,
    /// libgit2 failed; the message says how.
    Git = 10,
}

/// The status letter of a file row.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KerfScmChange {
    /// `A`
    Added = 0,
    /// `M`
    Modified = 1,
    /// `D`
    Deleted = 2,
    /// `?`
    Untracked = 3,
    /// `U`
    Conflicted = 4,
}

/// What a diff row is.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KerfScmRowKind {
    /// A `@@ -a,b +c,d @@` hunk header.
    Hunk = 0,
    /// An unchanged line.
    Context = 1,
    /// An added line.
    Added = 2,
    /// A removed line.
    Removed = 3,
}

/// The status `kerf_scm_refresh` read.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KerfScmSummary {
    /// Rows in the "Staged" group.
    pub staged: usize,
    /// Rows in the "Changes" group.
    pub changes: usize,
    /// Non-zero: rows were dropped at the file cap or for a non-UTF-8 path.
    pub truncated: u8,
    /// Non-zero: HEAD is detached and the branch is a short commit id.
    pub detached: u8,
}

/// The diff `kerf_scm_load_diff` read.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KerfScmDiffInfo {
    /// Rows: hunk headers and lines.
    pub rows: usize,
    /// Non-zero: binary or too large to diff, so no rows.
    pub binary: u8,
    /// Non-zero: lines were dropped at the cap or cut.
    pub truncated: u8,
}

/// One diff row's numbers.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KerfScmRow {
    /// What it is.
    pub kind: KerfScmRowKind,
    /// 1-based old line number; 0 for an added line or a hunk header.
    pub old_line: u32,
    /// 1-based new line number; 0 for a removed line or a hunk header.
    pub new_line: u32,
}

/// A repository and the screen's last snapshot of it. Opaque to the host.
pub struct KerfScm {
    scm: Scm,
    status: Status,
    rows: Vec<(KerfScmRow, String)>,
    error: Option<Error>,
}

impl KerfScm {
    /// Remember why `result` failed. Calls that reach the model clear the last
    /// reason when they start, so a stale one never outlives a success.
    fn record<T>(&mut self, result: Result<T, Error>) -> Result<T, KerfStatus> {
        result.map_err(|e| {
            let status = status_of(&e);
            self.error = Some(e);
            status
        })
    }
}

fn status_of(e: &Error) -> KerfStatus {
    match e {
        Error::EmptyMessage | Error::NothingToCommit => KerfStatus::Nothing,
        Error::MessageTooLarge { .. } => KerfStatus::TooLarge,
        Error::BadPath => KerfStatus::OutOfRange,
        _ => KerfStatus::Io,
    }
}

fn code_of(e: Option<&Error>) -> KerfScmError {
    match e {
        None => KerfScmError::None,
        Some(Error::NotARepository) => KerfScmError::NotARepository,
        Some(Error::Bare) => KerfScmError::Bare,
        Some(Error::BadPath) => KerfScmError::BadPath,
        Some(Error::EmptyMessage) => KerfScmError::EmptyMessage,
        Some(Error::MessageTooLarge { .. }) => KerfScmError::MessageTooLarge,
        Some(Error::NothingToCommit) => KerfScmError::NothingToCommit,
        Some(Error::NoIdentity) => KerfScmError::NoIdentity,
        Some(Error::Busy) => KerfScmError::Busy,
        Some(Error::Io(_)) => KerfScmError::Io,
        Some(Error::Git(_)) => KerfScmError::Git,
    }
}

fn group(group: u32) -> Result<Group, KerfStatus> {
    match group {
        KERF_SCM_GROUP_STAGED => Ok(Group::Staged),
        KERF_SCM_GROUP_CHANGES => Ok(Group::Changes),
        _ => Err(KerfStatus::Unsupported),
    }
}

fn rows(diff: &FileDiff) -> Vec<(KerfScmRow, String)> {
    let mut rows = Vec::new();
    for hunk in &diff.hunks {
        let header = KerfScmRow {
            kind: KerfScmRowKind::Hunk,
            old_line: 0,
            new_line: 0,
        };
        rows.push((header, hunk.header.clone()));
        for line in &hunk.lines {
            let kind = match line.kind {
                LineKind::Context => KerfScmRowKind::Context,
                LineKind::Added => KerfScmRowKind::Added,
                LineKind::Removed => KerfScmRowKind::Removed,
            };
            let row = KerfScmRow {
                kind,
                old_line: line.old_line.unwrap_or(0),
                new_line: line.new_line.unwrap_or(0),
            };
            rows.push((row, line.text.clone()));
        }
    }
    rows
}

/// The handle's state.
///
/// # Safety
///
/// `scm` is null or a live handle from `kerf_scm_open` that no other call is
/// using.
unsafe fn handle<'a>(scm: *mut KerfScm) -> Result<&'a mut KerfScm, KerfStatus> {
    // SAFETY: the caller promised a live, unaliased handle or null.
    unsafe { scm.as_mut() }.ok_or(KerfStatus::NullPointer)
}

/// Copy `text` into the host's buffer, as `kerf_editor_copy_text` does.
///
/// # Safety
///
/// `buf` is null or valid for `cap` bytes of writes; `out_len` is valid for a
/// write.
unsafe fn copy_out(
    text: &str,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> Result<(), KerfStatus> {
    // SAFETY: forwarded from the caller.
    unsafe { put(out_len, text.len()) }?;
    if text.len() > cap {
        return Err(KerfStatus::TooLarge);
    }
    if text.is_empty() {
        return Ok(());
    }
    if buf.is_null() {
        return Err(KerfStatus::NullPointer);
    }
    // SAFETY: `buf` is writable for `cap >= text.len()` bytes and cannot
    // overlap a string Kerf owns.
    unsafe { std::ptr::copy_nonoverlapping(text.as_ptr(), buf, text.len()) };
    Ok(())
}

/// Open the repository whose working tree holds the folder at `path`
/// (`path_len` bytes of UTF-8), searching upward, into `*out`. The status
/// starts empty until `kerf_scm_refresh`. On a refusal, `*out_error`, when
/// not null, says why.
///
/// # Safety
///
/// `path` points to `path_len` readable bytes; `out` is valid for a write of
/// one pointer; `out_error` is null or valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_scm_open(
    path: *const u8,
    path_len: usize,
    out: *mut *mut KerfScm,
    out_error: *mut KerfScmError,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let path = unsafe { text(path, path_len, MAX_PATH_BYTES) }?;
        if out.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let opened = Scm::open(Path::new(path)).map_err(|e| {
            if !out_error.is_null() {
                // SAFETY: the caller promised null or writable; checked above.
                unsafe { out_error.write(code_of(Some(&e))) };
            }
            status_of(&e)
        })?;
        let handle = Box::new(KerfScm {
            scm: opened,
            status: Status::default(),
            rows: Vec::new(),
            error: None,
        });
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out, Box::into_raw(handle)) }
    })
}

/// Free a repository handle. Null is ignored.
///
/// # Safety
///
/// `scm` is null or a live handle not used again after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_scm_free(scm: *mut KerfScm) {
    if !scm.is_null() {
        // SAFETY: the handle came from `Box::into_raw` and is not used again.
        let scm = unsafe { Box::from_raw(scm) };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(scm)));
    }
}

/// Why the last call on `scm` failed; `KERF_SCM_ERROR_NONE` after a success.
///
/// # Safety
///
/// `scm` is a live handle; `out` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_scm_error(scm: *mut KerfScm, out: *mut KerfScmError) -> KerfStatus {
    // SAFETY: forwarded from the caller.
    guard(|| unsafe { put(out, code_of(handle(scm)?.error.as_ref())) })
}

/// The last failure's text as UTF-8, copied as `kerf_editor_copy_text` does;
/// empty after a success.
///
/// # Safety
///
/// `scm` is a live handle; `buf` is null or valid for `cap` bytes of writes;
/// `out_len` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_scm_error_message(
    scm: *mut KerfScm,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let scm = unsafe { handle(scm) }?;
        let message = scm.error.as_ref().map(ToString::to_string);
        // SAFETY: forwarded from the caller.
        unsafe { copy_out(message.as_deref().unwrap_or_default(), buf, cap, out_len) }
    })
}

/// Read the branch and both groups again, and their sizes into `*out`.
///
/// # Safety
///
/// `scm` is a live handle; `out` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_scm_refresh(
    scm: *mut KerfScm,
    out: *mut KerfScmSummary,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let scm = unsafe { handle(scm) }?;
        scm.error = None;
        if out.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let status = scm.scm.status();
        scm.status = scm.record(status)?;
        let s = &scm.status;
        let summary = KerfScmSummary {
            staged: s.staged.len(),
            changes: s.changes.len(),
            truncated: u8::from(s.truncated),
            detached: u8::from(s.detached),
        };
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out, summary) }
    })
}

/// The branch name from the last refresh, copied as `kerf_editor_copy_text`
/// does.
///
/// # Safety
///
/// `scm` is a live handle; `buf` is null or valid for `cap` bytes of writes;
/// `out_len` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_scm_branch(
    scm: *mut KerfScm,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    // SAFETY: forwarded from the caller.
    guard(|| unsafe { copy_out(&handle(scm)?.status.branch, buf, cap, out_len) })
}

/// Row `index` of a `KERF_SCM_GROUP_*` from the last refresh: its letter in
/// `*out_change` and its path copied as `kerf_editor_copy_text` does
/// (`*out_change` is written whenever `*out_len` is).
/// `KERF_STATUS_OUT_OF_RANGE` past the last row.
///
/// # Safety
///
/// `scm` is a live handle; `out_change` and `out_len` are valid for a write;
/// `buf` is null or valid for `cap` bytes of writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_scm_file(
    scm: *mut KerfScm,
    group_id: u32,
    index: usize,
    out_change: *mut KerfScmChange,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let scm = unsafe { handle(scm) }?;
        let row = scm
            .status
            .group(group(group_id)?)
            .get(index)
            .ok_or(KerfStatus::OutOfRange)?;
        let change = match row.change {
            Change::Added => KerfScmChange::Added,
            Change::Modified => KerfScmChange::Modified,
            Change::Deleted => KerfScmChange::Deleted,
            Change::Untracked => KerfScmChange::Untracked,
            Change::Conflicted => KerfScmChange::Conflicted,
        };
        if out_len.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        // SAFETY: forwarded from the caller.
        unsafe { put(out_change, change).and_then(|()| copy_out(&row.path, buf, cap, out_len)) }
    })
}

/// Load the diff of `path` (`path_len` bytes of UTF-8, from the working
/// tree's root) in a `KERF_SCM_GROUP_*`, replacing the last one, and its size
/// into `*out`.
///
/// # Safety
///
/// `scm` is a live handle; `path` points to `path_len` readable bytes; `out`
/// is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_scm_load_diff(
    scm: *mut KerfScm,
    group_id: u32,
    path: *const u8,
    path_len: usize,
    out: *mut KerfScmDiffInfo,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let scm = unsafe { handle(scm) }?;
        scm.error = None;
        // SAFETY: forwarded from the caller.
        let path = unsafe { text(path, path_len, MAX_PATH_BYTES) }?;
        let group = group(group_id)?;
        if out.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let diff = scm.scm.diff(group, path);
        let diff = scm.record(diff)?;
        scm.rows = rows(&diff);
        let info = KerfScmDiffInfo {
            rows: scm.rows.len(),
            binary: u8::from(diff.binary),
            truncated: u8::from(diff.truncated),
        };
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out, info) }
    })
}

/// Row `index` of the loaded diff: its kind and line numbers in `*out_row`
/// and its text copied as `kerf_editor_copy_text` does (`*out_row` is
/// written whenever `*out_len` is). `KERF_STATUS_OUT_OF_RANGE` past the last
/// row.
///
/// # Safety
///
/// `scm` is a live handle; `out_row` and `out_len` are valid for a write;
/// `buf` is null or valid for `cap` bytes of writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_scm_diff_row(
    scm: *mut KerfScm,
    index: usize,
    out_row: *mut KerfScmRow,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let scm = unsafe { handle(scm) }?;
        let (row, line) = scm.rows.get(index).ok_or(KerfStatus::OutOfRange)?;
        if out_len.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        // SAFETY: forwarded from the caller.
        unsafe { put(out_row, *row).and_then(|()| copy_out(line, buf, cap, out_len)) }
    })
}

/// Run a model action that takes the host's path or message.
///
/// # Safety
///
/// `scm` is a live handle; `utf8` points to `len` readable bytes.
unsafe fn act(
    scm: *mut KerfScm,
    utf8: *const u8,
    len: usize,
    cap: usize,
    action: impl FnOnce(&Scm, &str) -> Result<(), Error>,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let scm = unsafe { handle(scm) }?;
        scm.error = None;
        // SAFETY: forwarded from the caller.
        let input = unsafe { text(utf8, len, cap) }?;
        let result = action(&scm.scm, input);
        scm.record(result)
    })
}

/// Stage `path` (`path_len` bytes of UTF-8, from the working tree's root) as
/// it is on disk; a deleted file is staged as a deletion. Refresh to see it.
///
/// # Safety
///
/// `scm` is a live handle; `path` points to `path_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_scm_stage(
    scm: *mut KerfScm,
    path: *const u8,
    path_len: usize,
) -> KerfStatus {
    // SAFETY: forwarded from the caller.
    unsafe { act(scm, path, path_len, MAX_PATH_BYTES, Scm::stage) }
}

/// Put `path` in the index back as HEAD has it, leaving the file on disk.
/// Refresh to see it.
///
/// # Safety
///
/// `scm` is a live handle; `path` points to `path_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_scm_unstage(
    scm: *mut KerfScm,
    path: *const u8,
    path_len: usize,
) -> KerfStatus {
    // SAFETY: forwarded from the caller.
    unsafe { act(scm, path, path_len, MAX_PATH_BYTES, Scm::unstage) }
}

/// Commit the staged files on the current branch with `message` (`len`
/// bytes of UTF-8). `KERF_STATUS_NOTHING` for an empty message or nothing
/// staged. Refresh to see it.
///
/// # Safety
///
/// `scm` is a live handle; `message` points to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_scm_commit(
    scm: *mut KerfScm,
    message: *const u8,
    len: usize,
) -> KerfStatus {
    let cap = kerf_scm::MAX_MESSAGE_BYTES;
    // SAFETY: forwarded from the caller.
    unsafe { act(scm, message, len, cap, |s, m| s.commit(m).map(drop)) }
}
