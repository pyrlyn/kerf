// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! One buffer and its cursor in Kerf's core, through `include/kerf.h`.

use std::fmt;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::ptr::{NonNull, null_mut};

use kerf_ffi::{self as ffi, KerfStatus};

/// A Kerf call that did not return `KERF_STATUS_OK`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(pub KerfStatus);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.0 {
            KerfStatus::InvalidUtf8 => "The file is not UTF-8 text.",
            KerfStatus::TooLarge => "The file is too large.",
            KerfStatus::Io => "The file could not be read.",
            KerfStatus::Render => "The editor could not be drawn.",
            KerfStatus::WrongThread => "Kerf was called off the GTK thread.",
            _ => "Kerf failed.",
        })
    }
}

impl std::error::Error for Error {}

pub(crate) fn check(status: KerfStatus) -> Result<(), Error> {
    match status {
        KerfStatus::Ok => Ok(()),
        status => Err(Error(status)),
    }
}

/// What [`Editor::run`] can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Delete the character before the cursor, or the selection.
    Backspace,
    /// Move the cursor one character left.
    Left,
    /// Move the cursor one character right.
    Right,
    /// Undo the last edit.
    Undo,
    /// Redo the last undone edit.
    Redo,
    /// Extend the selection one character left.
    SelectLeft,
    /// Extend the selection one character right.
    SelectRight,
}

impl Command {
    fn code(self) -> u32 {
        match self {
            Self::Backspace => ffi::KERF_COMMAND_BACKSPACE,
            Self::Left => ffi::KERF_COMMAND_LEFT,
            Self::Right => ffi::KERF_COMMAND_RIGHT,
            Self::Undo => ffi::KERF_COMMAND_UNDO,
            Self::Redo => ffi::KERF_COMMAND_REDO,
            Self::SelectLeft => ffi::KERF_COMMAND_SELECT_LEFT,
            Self::SelectRight => ffi::KERF_COMMAND_SELECT_RIGHT,
        }
    }
}

/// An editor handle. Neither `Send` nor `Sync`: like the GTK widget that
/// shows it, it stays on the GTK thread.
#[derive(Debug)]
pub struct Editor {
    handle: NonNull<ffi::KerfEditor>,
}

impl Editor {
    fn from_out(out: *mut ffi::KerfEditor) -> Result<Self, Error> {
        NonNull::new(out)
            .map(|handle| Self { handle })
            .ok_or(Error(KerfStatus::NullPointer))
    }

    /// An empty editor.
    pub fn new() -> Result<Self, Error> {
        let mut out = null_mut();
        // SAFETY: `out` is a valid place for one pointer.
        check(unsafe { ffi::kerf_editor_new(&mut out) })?;
        Self::from_out(out)
    }

    /// The UTF-8 file at `path`, cursor at the start. The path's bytes go to
    /// Kerf as they are; it refuses one that is not UTF-8.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let bytes = path.as_os_str().as_bytes();
        let mut out = null_mut();
        // SAFETY: `bytes` is `bytes.len()` readable bytes; `out` is a valid
        // place for one pointer.
        check(unsafe { ffi::kerf_editor_open(bytes.as_ptr(), bytes.len(), &mut out) })?;
        Self::from_out(out)
    }

    pub(crate) fn as_ptr(&self) -> *mut ffi::KerfEditor {
        self.handle.as_ptr()
    }

    /// Typing or a paste, replacing the selection.
    pub fn insert(&self, text: &str) -> Result<(), Error> {
        // SAFETY: a live handle; `text` is `text.len()` readable bytes.
        check(unsafe { ffi::kerf_editor_insert(self.as_ptr(), text.as_ptr(), text.len()) })
    }

    /// Runs `command`; false when it had nothing to do.
    pub fn run(&self, command: Command) -> Result<bool, Error> {
        // SAFETY: a live handle.
        match unsafe { ffi::kerf_editor_command(self.as_ptr(), command.code()) } {
            KerfStatus::Nothing => Ok(false),
            status => check(status).map(|()| true),
        }
    }

    /// The whole text.
    pub fn text(&self) -> Result<String, Error> {
        let mut len = 0;
        // SAFETY: a live handle; a null buffer with capacity 0 asks for the length.
        match unsafe { ffi::kerf_editor_copy_text(self.as_ptr(), null_mut(), 0, &mut len) } {
            KerfStatus::Ok => return Ok(String::new()),
            KerfStatus::TooLarge => {}
            status => return Err(Error(status)),
        }
        let mut bytes = vec![0; len];
        // SAFETY: a live handle; `bytes` is `len` writable bytes.
        check(unsafe {
            ffi::kerf_editor_copy_text(self.as_ptr(), bytes.as_mut_ptr(), len, &mut len)
        })?;
        bytes.truncate(len);
        String::from_utf8(bytes).map_err(|_| Error(KerfStatus::InvalidUtf8))
    }

    /// The selection's fixed end and the cursor as UTF-8 byte offsets; equal
    /// when nothing is selected.
    pub fn selection(&self) -> Result<(usize, usize), Error> {
        let (mut anchor, mut cursor) = (0, 0);
        // SAFETY: a live handle; both are valid places for one `usize`.
        check(unsafe { ffi::kerf_editor_selection(self.as_ptr(), &mut anchor, &mut cursor) })?;
        Ok((anchor, cursor))
    }

    /// Selects from `anchor` to `cursor`, UTF-8 byte offsets in either order.
    pub fn select(&self, anchor: usize, cursor: usize) -> Result<(), Error> {
        // SAFETY: a live handle.
        check(unsafe { ffi::kerf_editor_select(self.as_ptr(), anchor, cursor) })
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        // SAFETY: the handle is live and nothing uses it after this.
        unsafe { ffi::kerf_editor_free(self.as_ptr()) }
    }
}
