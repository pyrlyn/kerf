// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Kerf's C ABI (`include/kerf.h`) for the native hosts: AppKit first, then
//! WinUI and GTK. A host owns the window and its controls; through these
//! functions it opens a file into a [`KerfEditor`], forwards typing and
//! commands, and has a [`KerfView`] draw the editor.
//!
//! Everything from the host is checked: null pointers, lengths, UTF-8 and
//! command numbers. Every function returns a [`KerfStatus`] and runs inside
//! `catch_unwind`, so a panic never unwinds into the host's frames. Handles
//! are not thread-safe; a host calls them from its UI thread.

// Unit tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod view;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use kerf_core::{Editor, ReadError, TextBuffer};

pub use view::{
    KerfRect, KerfTheme, KerfView, kerf_view_caret, kerf_view_free, kerf_view_new,
    kerf_view_read_rgba, kerf_view_render, kerf_view_resize, kerf_view_set_theme,
};

/// Largest file a host can open. A bigger one is refused, not truncated.
pub const KERF_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Largest text one insert call takes: a paste, not a file.
pub const KERF_MAX_INSERT_BYTES: usize = 16 * 1024 * 1024;

/// Delete the character before the cursor.
pub const KERF_COMMAND_BACKSPACE: u32 = 0;
/// Move the cursor one character left.
pub const KERF_COMMAND_LEFT: u32 = 1;
/// Move the cursor one character right.
pub const KERF_COMMAND_RIGHT: u32 = 2;
/// Undo the last edit.
pub const KERF_COMMAND_UNDO: u32 = 3;
/// Redo the last undone edit.
pub const KERF_COMMAND_REDO: u32 = 4;
/// Move the cursor one character left, keeping the selection's anchor.
pub const KERF_COMMAND_SELECT_LEFT: u32 = 5;
/// Move the cursor one character right, keeping the selection's anchor.
pub const KERF_COMMAND_SELECT_RIGHT: u32 = 6;

/// What a call did. Every function returns one; out-parameters are written
/// only on `KERF_STATUS_OK`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KerfStatus {
    /// Done.
    Ok = 0,
    /// A required pointer was null.
    NullPointer = 1,
    /// Text or a path was not UTF-8.
    InvalidUtf8 = 2,
    /// Input or output is over its cap, or the host's buffer is too small.
    TooLarge = 3,
    /// The file could not be read.
    Io = 4,
    /// The command had nothing to do (undo with no history, left at the start).
    Nothing = 5,
    /// The view could not be drawn: a Slint platform error.
    Render = 6,
    /// Kerf panicked; the handle may be in a bad state and should be freed.
    Panic = 7,
    /// Unknown command number, or a call this view kind does not support.
    Unsupported = 8,
    /// An offset is past the end of the text or inside a UTF-8 character.
    OutOfRange = 9,
    /// A view call came from a thread other than the one that made the view;
    /// Slint is single-threaded.
    WrongThread = 10,
}

/// An editor: one buffer and its cursor. Opaque to the host.
pub struct KerfEditor {
    editor: Editor,
}

/// Run `f`, turning a panic into [`KerfStatus::Panic`].
fn guard(f: impl FnOnce() -> Result<(), KerfStatus>) -> KerfStatus {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => KerfStatus::Ok,
        Ok(Err(status)) => status,
        Err(_) => KerfStatus::Panic,
    }
}

/// The host's bytes. A zero length needs no pointer.
///
/// # Safety
///
/// `ptr` is null or points to `len` readable bytes that stay unchanged for
/// the call.
unsafe fn bytes<'a>(ptr: *const u8, len: usize, cap: usize) -> Result<&'a [u8], KerfStatus> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(KerfStatus::NullPointer);
    }
    if len > cap {
        return Err(KerfStatus::TooLarge);
    }
    // SAFETY: non-null, and the caller promised `len` readable bytes; `cap`
    // keeps `len` far below `isize::MAX`.
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// The host's UTF-8 text.
///
/// # Safety
///
/// As for [`bytes`].
unsafe fn text<'a>(ptr: *const u8, len: usize, cap: usize) -> Result<&'a str, KerfStatus> {
    // SAFETY: forwarded from the caller.
    let raw = unsafe { bytes(ptr, len, cap) }?;
    std::str::from_utf8(raw).map_err(|_| KerfStatus::InvalidUtf8)
}

/// Write `value` through the host's out-pointer.
///
/// # Safety
///
/// `out` is null or valid for a write of `T`.
unsafe fn put<T>(out: *mut T, value: T) -> Result<(), KerfStatus> {
    if out.is_null() {
        return Err(KerfStatus::NullPointer);
    }
    // SAFETY: non-null, and the caller promised it is writable.
    unsafe { out.write(value) };
    Ok(())
}

/// The editor behind a host handle.
///
/// # Safety
///
/// `ed` is null or a live handle from `kerf_editor_new`/`kerf_editor_open`
/// that no other call is using.
unsafe fn editor<'a>(ed: *mut KerfEditor) -> Result<&'a mut Editor, KerfStatus> {
    // SAFETY: the caller promised a live, unaliased handle or null.
    unsafe { ed.as_mut() }
        .map(|e| &mut e.editor)
        .ok_or(KerfStatus::NullPointer)
}

fn boxed(editor: Editor) -> *mut KerfEditor {
    Box::into_raw(Box::new(KerfEditor { editor }))
}

/// A new empty editor in `*out`. Free it with `kerf_editor_free`.
///
/// # Safety
///
/// `out` is valid for a write of one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_editor_new(out: *mut *mut KerfEditor) -> KerfStatus {
    // SAFETY: forwarded from the caller.
    guard(|| unsafe { put(out, boxed(Editor::default())) })
}

/// Open the UTF-8 file at `path` (`path_len` bytes of UTF-8, no NUL needed)
/// into a new editor in `*out`, cursor at the start.
///
/// # Safety
///
/// `path` points to `path_len` readable bytes; `out` is valid for a write of
/// one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_editor_open(
    path: *const u8,
    path_len: usize,
    out: *mut *mut KerfEditor,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let path = unsafe { text(path, path_len, 64 * 1024) }?;
        if out.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let content =
            kerf_core::read_text(Path::new(path), KERF_MAX_FILE_BYTES).map_err(|e| match e {
                ReadError::Io(_) => KerfStatus::Io,
                ReadError::TooLarge { .. } => KerfStatus::TooLarge,
                ReadError::NotUtf8 => KerfStatus::InvalidUtf8,
            })?;
        let editor = Editor::new(TextBuffer::with_text(&content));
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out, boxed(editor)) }
    })
}

/// Free an editor. Null is ignored.
///
/// # Safety
///
/// `ed` is null or a live handle not used again after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_editor_free(ed: *mut KerfEditor) {
    if !ed.is_null() {
        // SAFETY: the handle came from `Box::into_raw` and is not used again.
        let ed = unsafe { Box::from_raw(ed) };
        let _ = catch_unwind(AssertUnwindSafe(move || drop(ed)));
    }
}

/// Insert `len` bytes of UTF-8 at the cursor, as typing or a paste.
///
/// # Safety
///
/// `ed` is a live handle; `utf8` points to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_editor_insert(
    ed: *mut KerfEditor,
    utf8: *const u8,
    len: usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (editor, input) = unsafe { (editor(ed)?, text(utf8, len, KERF_MAX_INSERT_BYTES)?) };
        // The editor keeps its cursor on a character boundary, so inserting
        // there cannot fail; a failure would be a core bug, reported as one.
        editor
            .insert(input)
            .map(drop)
            .map_err(|_| KerfStatus::Panic)
    })
}

/// Run one `KERF_COMMAND_*`. `KERF_STATUS_NOTHING` when it had no effect.
///
/// # Safety
///
/// `ed` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_editor_command(ed: *mut KerfEditor, command: u32) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let editor = unsafe { editor(ed) }?;
        let did = match command {
            KERF_COMMAND_BACKSPACE => editor.backspace().is_some(),
            KERF_COMMAND_LEFT => editor.move_left(),
            KERF_COMMAND_RIGHT => editor.move_right(),
            KERF_COMMAND_UNDO => editor.undo().is_some(),
            KERF_COMMAND_REDO => editor.redo().is_some(),
            KERF_COMMAND_SELECT_LEFT => editor.select_left(),
            KERF_COMMAND_SELECT_RIGHT => editor.select_right(),
            _ => return Err(KerfStatus::Unsupported),
        };
        if did {
            Ok(())
        } else {
            Err(KerfStatus::Nothing)
        }
    })
}

/// The cursor as a UTF-8 byte offset into the text.
///
/// # Safety
///
/// `ed` is a live handle; `out` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_editor_cursor(ed: *mut KerfEditor, out: *mut usize) -> KerfStatus {
    // SAFETY: forwarded from the caller.
    guard(|| unsafe { put(out, editor(ed)?.cursor()) })
}

/// The selection as UTF-8 byte offsets: its fixed end in `*out_anchor` and
/// the cursor in `*out_cursor`, equal when nothing is selected.
///
/// # Safety
///
/// `ed` is a live handle; both out-pointers are valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_editor_selection(
    ed: *mut KerfEditor,
    out_anchor: *mut usize,
    out_cursor: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let editor = unsafe { editor(ed) }?;
        if out_cursor.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        // SAFETY: forwarded from the caller; both checked non-null before
        // either is written.
        unsafe { put(out_anchor, editor.anchor()).and_then(|()| put(out_cursor, editor.cursor())) }
    })
}

/// Select from `anchor` to `cursor`, UTF-8 byte offsets in either order;
/// equal offsets place the cursor with nothing selected.
/// `KERF_STATUS_OUT_OF_RANGE`, and no change, when either is past the end or
/// inside a character.
///
/// # Safety
///
/// `ed` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_editor_select(
    ed: *mut KerfEditor,
    anchor: usize,
    cursor: usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        unsafe { editor(ed) }?
            .select(anchor, cursor)
            .map_err(|_| KerfStatus::OutOfRange)
    })
}

/// Copy the whole text as UTF-8 into `buf` (`cap` bytes, no NUL written)
/// and its length into `*out_len`. When `cap` is too small, nothing is
/// copied, `*out_len` still gets the length needed and the status is
/// `KERF_STATUS_TOO_LARGE`; a null `buf` with `cap` 0 asks for the length.
///
/// # Safety
///
/// `ed` is a live handle; `buf` is null or valid for `cap` bytes of writes;
/// `out_len` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_editor_copy_text(
    ed: *mut KerfEditor,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let text = unsafe { editor(ed) }?.buffer().to_text();
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
        // overlap a `String` Kerf just allocated.
        unsafe { std::ptr::copy_nonoverlapping(text.as_ptr(), buf, text.len()) };
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::{KerfStatus, guard};

    #[test]
    fn a_panic_becomes_a_status() {
        let status = guard(|| panic!("core bug"));
        assert_eq!(status, KerfStatus::Panic);
    }
}
