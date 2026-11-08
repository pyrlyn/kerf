// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The Terminal panel: `kerf-terminal`'s tabs behind a [`KerfTerminals`]
//! handle. The host draws the cells it copies out and redraws when
//! `KerfTerminalInfo::revision` moves.

use std::path::{Path, PathBuf};

use kerf_core::{TrustError, TrustStore};
use kerf_terminal::{
    Cell, Color, MAX_WRITE_BYTES, Origin, ProcessState, Size, Spec, TabId, TerminalError, Terminals,
};

use crate::{KerfStatus, bytes, guard, put, text};

/// Longest path or program name the host can pass.
const MAX_PATH_BYTES: usize = 64 * 1024;

/// Bytes of UTF-8 a [`KerfTerminalCell`] holds; a longer grapheme is cut.
pub const KERF_TERMINAL_CELL_TEXT_BYTES: usize = 24;

/// `kerf_terminal_copy_text`: the tab title.
pub const KERF_TERMINAL_TEXT_TITLE: u32 = 0;
/// `kerf_terminal_copy_text`: the opening plugin's id, empty for the user.
pub const KERF_TERMINAL_TEXT_ORIGIN: u32 = 1;
/// `kerf_terminal_copy_text`: the screen as plain text, rows joined by `\n`.
pub const KERF_TERMINAL_TEXT_SCREEN: u32 = 2;

/// `KerfTerminalColor::kind`: the theme's default colour.
pub const KERF_TERMINAL_COLOR_DEFAULT: u8 = 0;
/// `KerfTerminalColor::kind`: palette entry `index`.
pub const KERF_TERMINAL_COLOR_INDEXED: u8 = 1;
/// `KerfTerminalColor::kind`: the 24-bit colour in `rgb`.
pub const KERF_TERMINAL_COLOR_RGB: u8 = 2;

/// The open terminal tabs. Opaque to the host.
pub struct KerfTerminals {
    terminals: Terminals,
    trust_file: PathBuf,
}

/// A cell colour.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KerfTerminalColor {
    /// `KERF_TERMINAL_COLOR_*`.
    pub kind: u8,
    /// The palette entry when `kind` is indexed.
    pub index: u8,
    /// Red, green, blue when `kind` is RGB.
    pub rgb: [u8; 3],
}

/// One screen cell.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KerfTerminalCell {
    /// UTF-8 of the grapheme; `text_len` bytes are used, 0 for a blank.
    pub text: [u8; KERF_TERMINAL_CELL_TEXT_BYTES],
    /// Bytes used in `text`.
    pub text_len: u8,
    /// `kerf_terminal::attr` bits: 1 bold, 2 dim, 4 italic, 8 underline,
    /// 16 inverse, 32 wide, 64 the right half of a wide character.
    pub attrs: u8,
    /// Foreground.
    pub fg: KerfTerminalColor,
    /// Background.
    pub bg: KerfTerminalColor,
}

/// A tab's state; everything but its cells and texts.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KerfTerminalInfo {
    /// Screen lines.
    pub rows: u16,
    /// Screen columns.
    pub cols: u16,
    /// Cursor line on the live screen.
    pub cursor_row: u16,
    /// Cursor column on the live screen.
    pub cursor_col: u16,
    /// Non-zero when the cursor is shown.
    pub cursor_visible: u8,
    /// Non-zero while the process runs.
    pub running: u8,
    /// Non-zero when a plugin opened the tab (the "Plugin" chip).
    pub from_plugin: u8,
    /// The exit code once `running` is 0.
    pub exit_code: u32,
    /// Rows scrolled back from the live screen.
    pub scroll_offset: u32,
    /// Rows of history kept, at most `HISTORY_ROWS` (the 4 MiB output cap).
    pub history_rows: u32,
    /// Bytes read from the process so far.
    pub output_bytes: u64,
    /// Moves on every change the view must redraw for.
    pub revision: u64,
}

impl From<TerminalError> for KerfStatus {
    fn from(e: TerminalError) -> Self {
        match e {
            TerminalError::Trust(TrustError::Untrusted { .. }) => Self::Untrusted,
            TerminalError::Trust(_) | TerminalError::Pty(_) | TerminalError::Exited => Self::Io,
            TerminalError::BadSize { .. } => Self::OutOfRange,
            TerminalError::TooManyTabs { .. }
            | TerminalError::TooLarge { .. }
            | TerminalError::InputFull => Self::TooLarge,
            TerminalError::NoSuchTab(_) => Self::NotFound,
        }
    }
}

/// # Safety
///
/// `terms` is null or a live handle no other call is using.
unsafe fn terminals<'a>(terms: *mut KerfTerminals) -> Result<&'a mut KerfTerminals, KerfStatus> {
    // SAFETY: the caller promised a live, unaliased handle or null.
    unsafe { terms.as_mut() }.ok_or(KerfStatus::NullPointer)
}

/// Copy `src` into the host's `buf` (`cap` items) and its length into
/// `*out_len`; a short `buf` gets nothing and `KERF_STATUS_TOO_LARGE`.
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

fn color(c: Color) -> KerfTerminalColor {
    match c {
        Color::Default => KerfTerminalColor::default(),
        Color::Indexed(index) => KerfTerminalColor {
            kind: KERF_TERMINAL_COLOR_INDEXED,
            index,
            rgb: [0; 3],
        },
        Color::Rgb(r, g, b) => KerfTerminalColor {
            kind: KERF_TERMINAL_COLOR_RGB,
            index: 0,
            rgb: [r, g, b],
        },
    }
}

fn cell(c: &Cell) -> KerfTerminalCell {
    let mut end = c.text.len().min(KERF_TERMINAL_CELL_TEXT_BYTES);
    while !c.text.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = KerfTerminalCell {
        attrs: c.attrs,
        fg: color(c.fg),
        bg: color(c.bg),
        // At most 24, so it fits.
        text_len: end as u8,
        ..KerfTerminalCell::default()
    };
    out.text[..end].copy_from_slice(&c.text.as_bytes()[..end]);
    out
}

/// A handle with no tabs in `*out`. Trust is read from the trust file at
/// `trust_file` (`trust_file_len` bytes of UTF-8) on every open, so a folder
/// the user trusts or revokes later is judged by its current state. Free it
/// with `kerf_terminals_free`, which stops every tab.
///
/// # Safety
///
/// `trust_file` points to `trust_file_len` readable bytes; `out` is valid for
/// a write of one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_terminals_new(
    trust_file: *const u8,
    trust_file_len: usize,
    out: *mut *mut KerfTerminals,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let path = unsafe { text(trust_file, trust_file_len, MAX_PATH_BYTES) }?;
        if path.is_empty() {
            return Err(KerfStatus::NullPointer);
        }
        let terms = Box::new(KerfTerminals {
            terminals: Terminals::new(),
            trust_file: PathBuf::from(path),
        });
        // SAFETY: forwarded from the caller.
        unsafe { put(out, Box::into_raw(terms)) }
    })
}

/// Stop every tab and free the handle. Null is ignored.
///
/// # Safety
///
/// `terms` is null or a live handle not used again after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_terminals_free(terms: *mut KerfTerminals) {
    if !terms.is_null() {
        // SAFETY: the handle came from `Box::into_raw` and is not used again.
        let terms = unsafe { Box::from_raw(terms) };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(terms)));
    }
}

/// Open a tab of `rows`×`cols` running `program` (empty: the user's shell)
/// in the folder `cwd`, and write its id to `*out_id`. A non-empty
/// `plugin_id` marks the tab as opened by that plugin; the host checked the
/// plugin's `ide.terminal` grant. `KERF_STATUS_UNTRUSTED`, and nothing
/// started, when `cwd` is not in a trusted folder.
///
/// # Safety
///
/// `terms` is a live handle; each pointer/length pair is readable for that
/// many bytes; `out_id` is valid for a write.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn kerf_terminal_open(
    terms: *mut KerfTerminals,
    cwd: *const u8,
    cwd_len: usize,
    program: *const u8,
    program_len: usize,
    plugin_id: *const u8,
    plugin_id_len: usize,
    rows: u16,
    cols: u16,
    out_id: *mut u64,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (terms, cwd, program, plugin) = unsafe {
            (
                terminals(terms)?,
                text(cwd, cwd_len, MAX_PATH_BYTES)?,
                text(program, program_len, MAX_PATH_BYTES)?,
                text(plugin_id, plugin_id_len, MAX_PATH_BYTES)?,
            )
        };
        if out_id.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let trust = TrustStore::load(&terms.trust_file).map_err(|_| KerfStatus::Io)?;
        let spec = Spec {
            cwd: Path::new(cwd).to_path_buf(),
            argv: (!program.is_empty())
                .then(|| program.to_owned())
                .into_iter()
                .collect(),
            size: Size { rows, cols },
            origin: if plugin.is_empty() {
                Origin::User
            } else {
                Origin::Plugin(plugin.to_owned())
            },
        };
        let id = terms.terminals.open(&trust, spec)?;
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out_id, id.0) }
    })
}

/// The open tabs' ids in tab order, through the `copy_text` length contract.
///
/// # Safety
///
/// `terms` is a live handle; `buf` is null or valid for `cap` ids; `out_len`
/// is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_terminal_tabs(
    terms: *mut KerfTerminals,
    buf: *mut u64,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let ids: Vec<u64> = unsafe { terminals(terms) }?
            .terminals
            .tabs()
            .iter()
            .map(|t| t.0)
            .collect();
        // SAFETY: forwarded from the caller.
        unsafe { copy_out(&ids, buf, cap, out_len) }
    })
}

/// Send `len` bytes to the tab's process as typed keys or a paste.
/// `KERF_STATUS_TOO_LARGE` when the write or the pending input is over its cap.
///
/// # Safety
///
/// `terms` is a live handle; `data` points to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_terminal_write(
    terms: *mut KerfTerminals,
    id: u64,
    data: *const u8,
    len: usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (terms, data) = unsafe { (terminals(terms)?, bytes(data, len, MAX_WRITE_BYTES)?) };
        Ok(terms.terminals.write(TabId(id), data)?)
    })
}

/// Resize the tab's screen and tell its process.
///
/// # Safety
///
/// `terms` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_terminal_resize(
    terms: *mut KerfTerminals,
    id: u64,
    rows: u16,
    cols: u16,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let terms = unsafe { terminals(terms) }?;
        Ok(terms.terminals.resize(TabId(id), Size { rows, cols })?)
    })
}

/// Scroll the view `offset` rows back into history (0: the live screen);
/// the offset applied, clamped to the history kept, goes to `*out_offset`.
///
/// # Safety
///
/// `terms` is a live handle; `out_offset` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_terminal_scroll(
    terms: *mut KerfTerminals,
    id: u64,
    offset: usize,
    out_offset: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let terms = unsafe { terminals(terms) }?;
        if out_offset.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let applied = terms.terminals.scroll(TabId(id), offset)?;
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out_offset, applied) }
    })
}

/// Stop the tab's process and remove the tab.
///
/// # Safety
///
/// `terms` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_terminal_close(terms: *mut KerfTerminals, id: u64) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        Ok(unsafe { terminals(terms) }?.terminals.close(TabId(id))?)
    })
}

/// The tab's state into `*out`.
///
/// # Safety
///
/// `terms` is a live handle; `out` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_terminal_info(
    terms: *mut KerfTerminals,
    id: u64,
    out: *mut KerfTerminalInfo,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let terms = unsafe { terminals(terms) }?;
        if out.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let i = terms.terminals.info(TabId(id))?;
        let (running, exit_code) = match i.state {
            ProcessState::Running => (1, 0),
            ProcessState::Exited { code } => (0, code),
        };
        let info = KerfTerminalInfo {
            rows: i.size.rows,
            cols: i.size.cols,
            cursor_row: i.cursor.0,
            cursor_col: i.cursor.1,
            cursor_visible: i.cursor_visible.into(),
            running,
            from_plugin: matches!(i.origin, Origin::Plugin(_)).into(),
            exit_code,
            // Both are bounded by HISTORY_ROWS.
            scroll_offset: u32::try_from(i.scroll_offset).unwrap_or(u32::MAX),
            history_rows: u32::try_from(i.history_rows).unwrap_or(u32::MAX),
            output_bytes: i.output_bytes,
            revision: i.revision,
        };
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out, info) }
    })
}

/// The cells on screen at the scroll offset, row-major (`rows * cols`
/// cells), through the `copy_text` length contract.
///
/// # Safety
///
/// `terms` is a live handle; `buf` is null or valid for `cap` cells;
/// `out_len` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_terminal_cells(
    terms: *mut KerfTerminals,
    id: u64,
    buf: *mut KerfTerminalCell,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let snap = unsafe { terminals(terms) }?.terminals.snapshot(TabId(id))?;
        let cells: Vec<KerfTerminalCell> = snap.cells.iter().map(cell).collect();
        // SAFETY: forwarded from the caller.
        unsafe { copy_out(&cells, buf, cap, out_len) }
    })
}

/// One `KERF_TERMINAL_TEXT_*` as UTF-8 into `buf` (`cap` bytes, no NUL),
/// with the same length contract as `kerf_editor_copy_text`.
///
/// # Safety
///
/// `terms` is a live handle; `buf` is null or valid for `cap` bytes of
/// writes; `out_len` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_terminal_copy_text(
    terms: *mut KerfTerminals,
    id: u64,
    which: u32,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let terms = &mut unsafe { terminals(terms) }?.terminals;
        let id = TabId(id);
        let value = match which {
            KERF_TERMINAL_TEXT_TITLE => terms.info(id)?.title,
            KERF_TERMINAL_TEXT_ORIGIN => match terms.info(id)?.origin {
                Origin::User => String::new(),
                Origin::Plugin(p) => p,
            },
            KERF_TERMINAL_TEXT_SCREEN => {
                let snap = terms.snapshot(id)?;
                let rows: Vec<String> = (0..snap.size.rows).map(|r| snap.row_text(r)).collect();
                rows.join("\n")
            }
            _ => return Err(KerfStatus::Unsupported),
        };
        // SAFETY: forwarded from the caller.
        unsafe { copy_out(value.as_bytes(), buf, cap, out_len) }
    })
}
