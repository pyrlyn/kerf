// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Kerf's window: a Slint shell over `kerf-core` (`research.md` §6).
//!
//! The window draws what [`SlintView`] sets and forwards keys; [`Shell`] maps
//! keys to [`Editor`] calls and redraws through the core's [`Render`] trait.
//! Nothing here edits text itself.

use std::cell::RefCell;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use kerf_core::{Editor, Frame, Render, TextBuffer};
use slint::ComponentHandle;
use slint::platform::Key;

// slint-build's output carries no docs and follows its own style.
#[allow(
    missing_docs,
    clippy::all,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]
mod ui {
    slint::include_modules!();
}

pub use ui::EditorWindow;

/// Largest file the shell opens. A file from a repository is untrusted
/// input, and the whole buffer is pushed to the window on every redraw.
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Why a window could not be opened.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// The file could not be read.
    #[error("cannot read {path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The underlying error.
        source: io::Error,
    },
    /// The file is larger than [`MAX_FILE_BYTES`].
    #[error("{path} is larger than {cap} bytes")]
    TooLarge {
        /// The file.
        path: PathBuf,
        /// The cap it exceeds.
        cap: u64,
    },
    /// The file is not UTF-8 text.
    #[error("{path} is not UTF-8 text")]
    NotUtf8 {
        /// The file.
        path: PathBuf,
    },
    /// Slint could not create the window.
    #[error("cannot create the window: {0}")]
    Platform(#[from] slint::PlatformError),
}

/// Read `path` as UTF-8 text, refusing files over [`MAX_FILE_BYTES`].
pub fn read_text(path: &Path) -> Result<String, OpenError> {
    let io_err = |source| OpenError::Io {
        path: path.to_owned(),
        source,
    };
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(io_err)?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(io_err)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(OpenError::TooLarge {
            path: path.to_owned(),
            cap: MAX_FILE_BYTES,
        });
    }
    String::from_utf8(bytes).map_err(|_| OpenError::NotUtf8 {
        path: path.to_owned(),
    })
}

/// What one key does to the editor.
#[derive(Debug, PartialEq, Eq)]
pub enum Action<'a> {
    /// Type this text at the cursor.
    Insert(&'a str),
    /// Delete the character before the cursor.
    Backspace,
    /// Move the cursor one character left.
    Left,
    /// Move the cursor one character right.
    Right,
    /// Undo the last edit.
    Undo,
    /// Redo the last undone edit.
    Redo,
}

/// Map a Slint key event to an action. Slint reports Command as `control`
/// on macOS, so one table covers every platform.
#[must_use]
pub fn action_for(text: &str, control: bool, shift: bool) -> Option<Action<'_>> {
    if control {
        return match text.to_ascii_lowercase().as_str() {
            "z" if shift => Some(Action::Redo),
            "z" => Some(Action::Undo),
            "y" => Some(Action::Redo),
            _ => None,
        };
    }
    let mut chars = text.chars();
    let first = chars.next()?;
    if chars.next().is_none() {
        match first {
            c if c == char::from(Key::Backspace) => return Some(Action::Backspace),
            c if c == char::from(Key::LeftArrow) => return Some(Action::Left),
            c if c == char::from(Key::RightArrow) => return Some(Action::Right),
            c if c == char::from(Key::Return) || c == char::from(Key::Tab) => {
                return Some(Action::Insert(text));
            }
            _ => {}
        }
    }
    // Slint sends other special keys as control codes or in the private-use
    // block (arrows are U+F700…); they are not text to insert.
    let typed = |c: char| !c.is_control() && !('\u{E000}'..='\u{F8FF}').contains(&c);
    text.chars().all(typed).then_some(Action::Insert(text))
}

/// Draws [`Frame`]s into an [`EditorWindow`].
pub struct SlintView {
    window: slint::Weak<EditorWindow>,
}

impl Render for SlintView {
    fn render(&mut self, frame: &Frame<'_>) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        // The whole text on every redraw: redraw-on-request with a viewport
        // is a later task (plan.md T12).
        window.set_content(frame.buffer.to_text().into());
        let line_start = frame.cursor - frame.col;
        let before = frame.buffer.slice(line_start..frame.cursor);
        window.set_before_cursor(before.unwrap_or_default().into());
        window.set_cursor_line(i32::try_from(frame.line).unwrap_or(i32::MAX));
        window.set_status(format!("Ln {}, Col {}", frame.line + 1, frame.col + 1).into());
    }
}

/// The editor and the surface it is drawn on.
pub struct Shell<R> {
    editor: Editor,
    view: R,
}

impl<R: Render> Shell<R> {
    /// A shell over `editor`, drawn once so the surface shows it at once.
    pub fn new(editor: Editor, view: R) -> Self {
        let mut shell = Self { editor, view };
        shell.view.render(&shell.editor.frame());
        shell
    }

    /// Apply one key and redraw. Answers whether the key was used.
    pub fn handle_key(&mut self, text: &str, control: bool, shift: bool) -> bool {
        let Some(action) = action_for(text, control, shift) else {
            return false;
        };
        let changed = match action {
            Action::Insert(text) => self.editor.insert(text).is_ok(),
            Action::Backspace => self.editor.backspace().is_some(),
            Action::Left => self.editor.move_left(),
            Action::Right => self.editor.move_right(),
            Action::Undo => self.editor.undo().is_some(),
            Action::Redo => self.editor.redo().is_some(),
        };
        if changed {
            self.view.render(&self.editor.frame());
        }
        true
    }
}

/// Create the window for `path` (an empty buffer without one), wired to a
/// [`Shell`]. The caller shows it, or runs it with `run()`.
pub fn open(path: Option<&Path>) -> Result<EditorWindow, OpenError> {
    let text = path.map(read_text).transpose()?.unwrap_or_default();
    let window = EditorWindow::new()?;
    let name = path
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    window.set_file_name(name.into());
    let view = SlintView {
        window: window.as_weak(),
    };
    // The window holds the shell and the shell only a weak handle back, so
    // dropping the window frees both.
    let shell = Rc::new(RefCell::new(Shell::new(
        Editor::new(TextBuffer::with_text(&text)),
        view,
    )));
    window.on_key(move |text, control, shift| shell.borrow_mut().handle_key(&text, control, shift));
    Ok(window)
}

#[cfg(test)]
mod tests {
    use slint::platform::Key;

    use super::{Action, action_for};

    fn key(k: Key) -> String {
        char::from(k).to_string()
    }

    #[test]
    fn printable_text_and_enter_are_inserted() {
        assert_eq!(action_for("é", false, false), Some(Action::Insert("é")));
        assert_eq!(action_for("\n", false, false), Some(Action::Insert("\n")));
    }

    #[test]
    fn special_keys_are_not_inserted() {
        assert_eq!(action_for(&key(Key::Escape), false, false), None);
        assert_eq!(action_for(&key(Key::UpArrow), false, false), None);
        assert_eq!(
            action_for(&key(Key::Backspace), false, false),
            Some(Action::Backspace)
        );
        assert_eq!(
            action_for(&key(Key::LeftArrow), false, false),
            Some(Action::Left)
        );
    }

    #[test]
    fn control_z_undoes_and_shift_or_y_redoes() {
        assert_eq!(action_for("z", true, false), Some(Action::Undo));
        assert_eq!(action_for("Z", true, true), Some(Action::Redo));
        assert_eq!(action_for("y", true, false), Some(Action::Redo));
        assert_eq!(action_for("s", true, false), None);
    }
}
