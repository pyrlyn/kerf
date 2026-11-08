// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The project navigator and Open Quickly (`kerf-navigator`). The host
//! addresses rows and hits by index into the last snapshot, which every
//! call that changes them rebuilds.

use std::path::{Component, Path};

use kerf_navigator::{Hit, HitKind, Navigator, OpenQuickly, Row};
use kerf_semantic::Semantic;

use crate::{KerfStatus, guard, put, text};

/// Longest project root or file path a host can pass.
const MAX_PATH_BYTES: usize = 64 * 1024;
/// Longest filter or Open Quickly query a host can pass.
const MAX_QUERY_BYTES: usize = 4 * 1024;

/// Open a collapsed folder, listing it the first time.
pub const KERF_NAVIGATOR_EXPAND: u32 = 0;
/// Close an expanded folder; a selection inside moves to it.
pub const KERF_NAVIGATOR_COLLAPSE: u32 = 1;
/// Select the row.
pub const KERF_NAVIGATOR_SELECT: u32 = 2;

/// A row's file name, or a hit's label (symbol name or relative path).
pub const KERF_NAVIGATOR_TEXT_NAME: u32 = 0;
/// The path relative to the project root.
pub const KERF_NAVIGATOR_TEXT_PATH: u32 = 1;
/// A symbol hit's kind (`function`, `class`, …); empty otherwise.
pub const KERF_NAVIGATOR_TEXT_DETAIL: u32 = 2;

/// A project's navigator and Open Quickly. Opaque to the host.
pub struct KerfNavigator {
    nav: Navigator,
    rows: Vec<Row>,
    quick: OpenQuickly,
    /// Made on the first indexed file: loading grammars costs more than a
    /// navigator that never searches symbols should pay.
    semantic: Option<Semantic>,
}

/// One navigator row. Flags are bytes, not `bool`, so any value is defined.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KerfNavigatorRow {
    /// Indent level; top-level rows are 0.
    pub depth: u32,
    /// Non-zero for a folder.
    pub folder: u8,
    /// Non-zero when the folder's disclosure is open.
    pub expanded: u8,
    /// Non-zero for the selected row.
    pub selected: u8,
    /// Non-zero when the folder's listing hit a cap.
    pub truncated: u8,
}

/// One Open Quickly hit, best first.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KerfQuickHit {
    /// A symbol's 0-based line; 0 for a file.
    pub line: usize,
    /// Match score; higher is better.
    pub score: u32,
    /// Non-zero for a symbol, zero for a file.
    pub symbol: u8,
    /// Non-zero for the selected hit.
    pub selected: u8,
}

/// A half-open range of UTF-8 byte offsets.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KerfByteRange {
    /// First byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
}

/// # Safety
///
/// `nav` is null or a live handle that no other call is using.
unsafe fn navigator<'a>(nav: *mut KerfNavigator) -> Result<&'a mut KerfNavigator, KerfStatus> {
    // SAFETY: the caller promised a live, unaliased handle or null.
    unsafe { nav.as_mut() }.ok_or(KerfStatus::NullPointer)
}

/// Copy `src` into the host's `buf` of `cap` items and its length into
/// `*out_len`, as `kerf_editor_copy_text` does with text.
///
/// # Safety
///
/// `buf` is null or valid for `cap` writes of `T`; `out_len` is valid for a write.
unsafe fn copy_out<T: Copy>(
    src: &[T],
    buf: *mut T,
    cap: usize,
    out_len: *mut usize,
) -> Result<(), KerfStatus> {
    // SAFETY: forwarded from the caller.
    unsafe { put(out_len, src.len()) }?;
    if src.len() > cap {
        return Err(KerfStatus::TooLarge);
    }
    if src.is_empty() {
        return Ok(());
    }
    if buf.is_null() {
        return Err(KerfStatus::NullPointer);
    }
    // SAFETY: `buf` is writable for `cap >= src.len()` items and cannot
    // overlap memory Kerf owns.
    unsafe { std::ptr::copy_nonoverlapping(src.as_ptr(), buf, src.len()) };
    Ok(())
}

fn status(e: &kerf_navigator::Error) -> KerfStatus {
    match e {
        kerf_navigator::Error::NotAFolder { .. } | kerf_navigator::Error::List { .. } => {
            KerfStatus::Io
        }
    }
}

/// A navigator over the folder at `root` (`root_len` bytes of UTF-8), its
/// top level listed, in `*out`. Free it with `kerf_navigator_free`.
///
/// # Safety
///
/// `root` points to `root_len` readable bytes; `out` is valid for a write of
/// one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_navigator_open(
    root: *const u8,
    root_len: usize,
    out: *mut *mut KerfNavigator,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let root = unsafe { text(root, root_len, MAX_PATH_BYTES) }?;
        if out.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let nav = Navigator::open(root).map_err(|e| status(&e))?;
        let handle = Box::new(KerfNavigator {
            rows: nav.rows(),
            nav,
            quick: OpenQuickly::default(),
            semantic: None,
        });
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out, Box::into_raw(handle)) }
    })
}

/// Free a navigator. Null is ignored.
///
/// # Safety
///
/// `nav` is null or a live handle not used again after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_navigator_free(nav: *mut KerfNavigator) {
    if !nav.is_null() {
        // SAFETY: the handle came from `Box::into_raw` and is not used again.
        let nav = unsafe { Box::from_raw(nav) };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(nav)));
    }
}

/// Copy the visible rows, top to bottom, into `buf` (`cap` rows) and their
/// count into `*out_len`; sizing works as in `kerf_editor_copy_text`.
///
/// # Safety
///
/// `nav` is a live handle; `buf` is null or valid for `cap` rows of writes;
/// `out_len` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_navigator_rows(
    nav: *mut KerfNavigator,
    buf: *mut KerfNavigatorRow,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let nav = unsafe { navigator(nav) }?;
        let rows: Vec<KerfNavigatorRow> = nav
            .rows
            .iter()
            .map(|r| KerfNavigatorRow {
                depth: u32::try_from(r.depth).unwrap_or(u32::MAX),
                folder: u8::from(r.kind == kerf_navigator::NodeKind::Folder),
                expanded: u8::from(r.expanded),
                selected: u8::from(r.selected),
                truncated: u8::from(r.truncated),
            })
            .collect();
        // SAFETY: forwarded from the caller.
        unsafe { copy_out(&rows, buf, cap, out_len) }
    })
}

/// Copy one `KERF_NAVIGATOR_TEXT_*` of row `row` as UTF-8 into `buf`;
/// sizing works as in `kerf_editor_copy_text`.
///
/// # Safety
///
/// As for `kerf_navigator_rows`, with `buf` valid for `cap` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_navigator_row_text(
    nav: *mut KerfNavigator,
    row: usize,
    field: u32,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let nav = unsafe { navigator(nav) }?;
        let r = nav.rows.get(row).ok_or(KerfStatus::OutOfRange)?;
        let value = match field {
            KERF_NAVIGATOR_TEXT_NAME => r.name.clone(),
            KERF_NAVIGATOR_TEXT_PATH => nav
                .nav
                .relative_path(r.id)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            KERF_NAVIGATOR_TEXT_DETAIL => String::new(),
            _ => return Err(KerfStatus::Unsupported),
        };
        // SAFETY: forwarded from the caller.
        unsafe { copy_out(value.as_bytes(), buf, cap, out_len) }
    })
}

/// Run one `KERF_NAVIGATOR_EXPAND`/`_COLLAPSE`/`_SELECT` on row `row`.
/// `KERF_STATUS_NOTHING` when it had no effect, `KERF_STATUS_IO` when the
/// folder cannot be listed.
///
/// # Safety
///
/// `nav` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_navigator_row_command(
    nav: *mut KerfNavigator,
    row: usize,
    command: u32,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let nav = unsafe { navigator(nav) }?;
        let id = nav.rows.get(row).ok_or(KerfStatus::OutOfRange)?.id;
        let did = match command {
            KERF_NAVIGATOR_EXPAND => nav.nav.expand(id).map_err(|e| status(&e))?,
            KERF_NAVIGATOR_COLLAPSE => nav.nav.collapse(id),
            KERF_NAVIGATOR_SELECT => nav.nav.select(id),
            _ => return Err(KerfStatus::Unsupported),
        };
        nav.rows = nav.nav.rows();
        if did {
            Ok(())
        } else {
            Err(KerfStatus::Nothing)
        }
    })
}

/// Show only rows whose name contains `text` (UTF-8, ignoring case) and
/// their folders, across the whole project; empty text clears the filter.
///
/// # Safety
///
/// `nav` is a live handle; `utf8` points to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_navigator_set_filter(
    nav: *mut KerfNavigator,
    utf8: *const u8,
    len: usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (nav, filter) = unsafe { (navigator(nav)?, text(utf8, len, MAX_QUERY_BYTES)?) };
        nav.nav.set_filter(filter);
        nav.rows = nav.nav.rows();
        Ok(())
    })
}

/// Parse the file at `path` (UTF-8, relative to the root) so Open Quickly
/// finds its symbols; again after it changes. `KERF_STATUS_UNSUPPORTED` for
/// a language Kerf does not parse, `KERF_STATUS_OUT_OF_RANGE` for a path
/// that is absolute or climbs out with `..`.
///
/// # Safety
///
/// `nav` is a live handle; `path` points to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_navigator_index_file(
    nav: *mut KerfNavigator,
    path: *const u8,
    len: usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (nav, rel) = unsafe { (navigator(nav)?, text(path, len, MAX_PATH_BYTES)?) };
        let rel = Path::new(rel);
        if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
            return Err(KerfStatus::OutOfRange);
        }
        let path = nav.nav.root().join(rel);
        let cap = u64::try_from(kerf_semantic::MAX_FILE_BYTES).unwrap_or(u64::MAX);
        let source = kerf_core::read_text(&path, cap).map_err(|e| match e {
            kerf_core::ReadError::Io(_) => KerfStatus::Io,
            kerf_core::ReadError::TooLarge { .. } => KerfStatus::TooLarge,
            kerf_core::ReadError::NotUtf8 => KerfStatus::InvalidUtf8,
        })?;
        let semantic = match &mut nav.semantic {
            Some(s) => s,
            // A grammar that does not load is a build bug, not a host error.
            slot @ None => slot.insert(Semantic::new().map_err(|_| KerfStatus::Panic)?),
        };
        semantic.open(path, source).map_err(|e| match e {
            kerf_semantic::Error::UnsupportedLanguage { .. } => KerfStatus::Unsupported,
            kerf_semantic::Error::FileTooLarge { .. }
            | kerf_semantic::Error::TooManyFiles { .. } => KerfStatus::TooLarge,
            _ => KerfStatus::Io,
        })
    })
}

/// Rank the project's files and indexed symbols against `query` (UTF-8,
/// ignoring case), keep the best and select the first; the hit count goes
/// in `*out_count`. An empty query has no hits.
///
/// # Safety
///
/// `nav` is a live handle; `query` points to `len` readable bytes;
/// `out_count` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_navigator_quick_query(
    nav: *mut KerfNavigator,
    query: *const u8,
    len: usize,
    out_count: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (nav, query) = unsafe { (navigator(nav)?, text(query, len, MAX_QUERY_BYTES)?) };
        if out_count.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let files = nav.nav.files();
        let count = nav.quick.query(query, files, nav.semantic.as_ref()).len();
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out_count, count) }
    })
}

/// Copy the last query's hits into `buf` (`cap` hits) and their count into
/// `*out_len`; sizing works as in `kerf_editor_copy_text`.
///
/// # Safety
///
/// `nav` is a live handle; `buf` is null or valid for `cap` hits of writes;
/// `out_len` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_navigator_hits(
    nav: *mut KerfNavigator,
    buf: *mut KerfQuickHit,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let nav = unsafe { navigator(nav) }?;
        let selected = nav.quick.selected();
        let hits: Vec<KerfQuickHit> = (nav.quick.hits().iter().enumerate())
            .map(|(i, h)| KerfQuickHit {
                line: h.line,
                score: h.score,
                symbol: u8::from(h.kind == HitKind::Symbol),
                selected: u8::from(selected == Some(i)),
            })
            .collect();
        // SAFETY: forwarded from the caller.
        unsafe { copy_out(&hits, buf, cap, out_len) }
    })
}

fn hit(nav: &KerfNavigator, index: usize) -> Result<&Hit, KerfStatus> {
    nav.quick.hits().get(index).ok_or(KerfStatus::OutOfRange)
}

/// Copy one `KERF_NAVIGATOR_TEXT_*` of hit `index` as UTF-8 into `buf`;
/// sizing works as in `kerf_editor_copy_text`.
///
/// # Safety
///
/// As for `kerf_navigator_row_text`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_navigator_hit_text(
    nav: *mut KerfNavigator,
    index: usize,
    field: u32,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let h = hit(unsafe { navigator(nav) }?, index)?;
        let value = match field {
            KERF_NAVIGATOR_TEXT_NAME => h.label.clone(),
            KERF_NAVIGATOR_TEXT_PATH => h.path.to_string_lossy().into_owned(),
            KERF_NAVIGATOR_TEXT_DETAIL => h.detail.clone(),
            _ => return Err(KerfStatus::Unsupported),
        };
        // SAFETY: forwarded from the caller.
        unsafe { copy_out(value.as_bytes(), buf, cap, out_len) }
    })
}

/// Copy the matched byte ranges of hit `index`'s name text into `buf`
/// (`cap` ranges), for highlighting; sizing works as in `kerf_editor_copy_text`.
///
/// # Safety
///
/// `nav` is a live handle; `buf` is null or valid for `cap` ranges of
/// writes; `out_len` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_navigator_hit_ranges(
    nav: *mut KerfNavigator,
    index: usize,
    buf: *mut KerfByteRange,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let h = hit(unsafe { navigator(nav) }?, index)?;
        let ranges: Vec<KerfByteRange> = (h.ranges.iter())
            .map(|r| KerfByteRange {
                start: r.start,
                end: r.end,
            })
            .collect();
        // SAFETY: forwarded from the caller.
        unsafe { copy_out(&ranges, buf, cap, out_len) }
    })
}

/// Move Open Quickly's selection by `delta` hits (arrow and page keys),
/// stopping at the ends. `KERF_STATUS_NOTHING` when it did not move.
///
/// # Safety
///
/// `nav` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_navigator_quick_move(
    nav: *mut KerfNavigator,
    delta: i32,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let nav = unsafe { navigator(nav) }?;
        let delta = isize::try_from(delta).map_err(|_| KerfStatus::OutOfRange)?;
        if nav.quick.move_selection(delta) {
            Ok(())
        } else {
            Err(KerfStatus::Nothing)
        }
    })
}
