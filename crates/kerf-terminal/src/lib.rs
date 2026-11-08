// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Integrated terminals the core owns (`docs/architecture.md` §4.4,
//! `ide.terminal`): one PTY per tab (ConPTY on Windows), a VT screen model a
//! host draws from, input, resize and close.
//!
//! A tab starts only through [`Terminals::open`], which passes Workspace Trust
//! first and hands the [`SpawnPermit`](kerf_core::SpawnPermit) to the spawn,
//! so an untrusted folder never gets a shell. Whatever the shell prints is
//! untrusted: it lands in a screen model whose history is capped at
//! [`MAX_OUTPUT_BYTES`], and input waits in a capped queue, so neither a noisy
//! process nor a stuck one grows memory. A tab a plugin opened keeps the
//! plugin's id as its [`Origin`]; checking the plugin's `ide.terminal` grant
//! is the caller's job, as for every host function.
//!
//! No UI: a host reads [`Terminals::info`] and [`Terminals::snapshot`] and
//! redraws when [`Info::revision`] moves.

// Unit tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod session;

use std::path::PathBuf;

use kerf_core::{SpawnKind, TrustError, TrustStore};

use session::Session;

/// Memory the kept output of one tab may use: its scrollback history.
pub const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
/// Widest screen a tab takes.
pub const MAX_COLS: u16 = 256;
/// Tallest screen a tab takes.
pub const MAX_ROWS: u16 = 256;
/// History rows a tab keeps. Rows keep the width they scrolled out with, so
/// the budget is sized for the widest screen and holds across resizes.
pub const HISTORY_ROWS: usize =
    MAX_OUTPUT_BYTES / (std::mem::size_of::<vt100::Cell>() * MAX_COLS as usize);
// The kept history fits the cap even at the widest screen, and stays long
// enough to scroll back through a test run.
const _: () = assert!(
    HISTORY_ROWS * MAX_COLS as usize * std::mem::size_of::<vt100::Cell>() <= MAX_OUTPUT_BYTES
        && HISTORY_ROWS >= 500
);
/// Open tabs at once.
pub const MAX_TABS: usize = 32;
/// Largest input one write takes: a paste.
pub const MAX_WRITE_BYTES: usize = 64 * 1024;
/// Writes waiting for a process that does not read its input.
pub const MAX_PENDING_WRITES: usize = 64;
/// Longest window title kept from an OSC sequence; the rest is cut.
pub const MAX_TITLE_BYTES: usize = 256;
/// Longest plugin id an origin takes.
pub const MAX_PLUGIN_ID_BYTES: usize = 128;

/// Who opened a tab, shown in the tab chip and the footer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// The user.
    User,
    /// A plugin through `ide.terminal`, by plugin id.
    Plugin(String),
}

/// A screen size in character cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    /// Lines.
    pub rows: u16,
    /// Columns.
    pub cols: u16,
}

/// What to start in a new tab.
#[derive(Debug, Clone)]
pub struct Spec {
    /// The folder the process runs in; it must pass Workspace Trust.
    pub cwd: PathBuf,
    /// Program and arguments; empty starts the user's login shell.
    pub argv: Vec<String>,
    /// The first screen size.
    pub size: Size,
    /// Who asked for the tab.
    pub origin: Origin,
}

/// A tab's id, never reused within one [`Terminals`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(pub u64);

/// The process behind a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    /// Still running.
    Running,
    /// Gone; the exit code, 1 when a signal ended it.
    Exited {
        /// Exit code.
        code: u32,
    },
}

/// A cell colour; indexed colours are the host theme's palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    /// The theme's foreground or background.
    #[default]
    Default,
    /// One of the 256 palette entries.
    Indexed(u8),
    /// A 24-bit colour.
    Rgb(u8, u8, u8),
}

/// Cell attribute bits.
pub mod attr {
    /// Bold.
    pub const BOLD: u8 = 1;
    /// Dim.
    pub const DIM: u8 = 2;
    /// Italic.
    pub const ITALIC: u8 = 4;
    /// Underline.
    pub const UNDERLINE: u8 = 8;
    /// Foreground and background swapped.
    pub const INVERSE: u8 = 16;
    /// A double-width character; the next cell is its continuation.
    pub const WIDE: u8 = 32;
    /// The right half of a double-width character, drawn by the cell before.
    pub const WIDE_CONTINUATION: u8 = 64;
}

/// One screen cell.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cell {
    /// The grapheme drawn here; empty for a blank.
    pub text: String,
    /// Foreground.
    pub fg: Color,
    /// Background.
    pub bg: Color,
    /// [`attr`] bits.
    pub attrs: u8,
}

/// Everything about a tab except its cells; cheap to read every frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// The OSC title the process set, else the program name.
    pub title: String,
    /// Who opened the tab.
    pub origin: Origin,
    /// Running or exited.
    pub state: ProcessState,
    /// The screen size.
    pub size: Size,
    /// Cursor row and column on the live screen.
    pub cursor: (u16, u16),
    /// Whether the process hid the cursor.
    pub cursor_visible: bool,
    /// Rows scrolled back from the live screen; 0 shows it.
    pub scroll_offset: usize,
    /// Rows of history kept, at most [`HISTORY_ROWS`].
    pub history_rows: usize,
    /// Bytes read from the process so far.
    pub output_bytes: u64,
    /// Moves on every change a view must redraw for.
    pub revision: u64,
}

/// The cells on screen at the current scroll offset, row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// The screen size; `cells.len() == rows * cols`.
    pub size: Size,
    /// Row-major cells.
    pub cells: Vec<Cell>,
}

impl Snapshot {
    /// Row `row` as plain text with trailing blanks trimmed.
    pub fn row_text(&self, row: u16) -> String {
        let cols = usize::from(self.size.cols);
        let start = usize::from(row) * cols;
        let line: String = self
            .cells
            .get(start..start + cols)
            .unwrap_or_default()
            .iter()
            .map(|c| if c.text.is_empty() { " " } else { &c.text })
            .collect();
        line.trim_end().to_owned()
    }
}

/// Why a terminal call failed.
#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    /// The folder is untrusted or does not resolve; nothing was started.
    #[error(transparent)]
    Trust(#[from] TrustError),
    /// The PTY or the process could not be started or resized.
    #[error("terminal: {0}")]
    Pty(String),
    /// A size of zero or over [`MAX_ROWS`] × [`MAX_COLS`].
    #[error("terminal size {rows}x{cols} is out of range")]
    BadSize {
        /// Rows asked for.
        rows: u16,
        /// Columns asked for.
        cols: u16,
    },
    /// [`MAX_TABS`] are open.
    #[error("too many terminals open ({cap})")]
    TooManyTabs {
        /// The cap.
        cap: usize,
    },
    /// No open tab has this id.
    #[error("no terminal {0:?}")]
    NoSuchTab(TabId),
    /// A write or a plugin id over its cap.
    #[error("{what} is {len} bytes, over the {cap}-byte cap")]
    TooLarge {
        /// What was too large.
        what: &'static str,
        /// Its length.
        len: usize,
        /// The cap.
        cap: usize,
    },
    /// [`MAX_PENDING_WRITES`] writes wait for a process that is not reading.
    #[error("terminal input is full")]
    InputFull,
    /// The process exited or closed its input.
    #[error("terminal process is gone")]
    Exited,
}

/// The open tabs, in the order they were opened.
#[derive(Default)]
pub struct Terminals {
    tabs: Vec<(TabId, Session)>,
    next_id: u64,
}

impl Terminals {
    /// No tabs.
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks trust for `spec.cwd`, then starts the process in a new tab.
    pub fn open(&mut self, trust: &TrustStore, spec: Spec) -> Result<TabId, TerminalError> {
        check_size(spec.size)?;
        if let Origin::Plugin(id) = &spec.origin {
            check_len("plugin id", id.len(), MAX_PLUGIN_ID_BYTES)?;
        }
        if self.tabs.len() >= MAX_TABS {
            return Err(TerminalError::TooManyTabs { cap: MAX_TABS });
        }
        // A shell is a task the user runs; trust gates it the same way.
        let permit = trust.authorize(&spec.cwd, SpawnKind::Task)?;
        let session = Session::spawn(permit, spec)?;
        self.next_id += 1;
        let id = TabId(self.next_id);
        self.tabs.push((id, session));
        Ok(id)
    }

    /// The open tabs' ids, in tab order.
    pub fn tabs(&self) -> Vec<TabId> {
        self.tabs.iter().map(|(id, _)| *id).collect()
    }

    /// Sends `bytes` to the process as typed input or a paste.
    pub fn write(&mut self, id: TabId, bytes: &[u8]) -> Result<(), TerminalError> {
        check_len("terminal input", bytes.len(), MAX_WRITE_BYTES)?;
        self.session(id)?.write(bytes)
    }

    /// Resizes the screen and tells the process.
    pub fn resize(&mut self, id: TabId, size: Size) -> Result<(), TerminalError> {
        check_size(size)?;
        self.session(id)?.resize(size)
    }

    /// Scrolls the view `offset` rows back into history, clamped; answers the
    /// offset applied.
    pub fn scroll(&mut self, id: TabId, offset: usize) -> Result<usize, TerminalError> {
        Ok(self.session(id)?.scroll(offset))
    }

    /// The tab's title, state, cursor and counters.
    pub fn info(&mut self, id: TabId) -> Result<Info, TerminalError> {
        Ok(self.session(id)?.info())
    }

    /// The tab's cells at its scroll offset.
    pub fn snapshot(&mut self, id: TabId) -> Result<Snapshot, TerminalError> {
        Ok(self.session(id)?.snapshot())
    }

    /// Stops the process and removes the tab.
    pub fn close(&mut self, id: TabId) -> Result<(), TerminalError> {
        let at = self
            .tabs
            .iter()
            .position(|(t, _)| *t == id)
            .ok_or(TerminalError::NoSuchTab(id))?;
        drop(self.tabs.remove(at));
        Ok(())
    }

    fn session(&mut self, id: TabId) -> Result<&mut Session, TerminalError> {
        self.tabs
            .iter_mut()
            .find(|(t, _)| *t == id)
            .map(|(_, s)| s)
            .ok_or(TerminalError::NoSuchTab(id))
    }
}

fn check_size(size: Size) -> Result<(), TerminalError> {
    if size.rows == 0 || size.cols == 0 || size.rows > MAX_ROWS || size.cols > MAX_COLS {
        return Err(TerminalError::BadSize {
            rows: size.rows,
            cols: size.cols,
        });
    }
    Ok(())
}

fn check_len(what: &'static str, len: usize, cap: usize) -> Result<(), TerminalError> {
    if len > cap {
        return Err(TerminalError::TooLarge { what, len, cap });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(size: Size, origin: Origin) -> Spec {
        Spec {
            cwd: std::env::temp_dir(),
            argv: Vec::new(),
            size,
            origin,
        }
    }

    #[test]
    fn bad_sizes_and_long_plugin_ids_are_refused_before_trust() {
        let mut terms = Terminals::new();
        let trust = TrustStore::new();
        for size in [
            Size { rows: 0, cols: 80 },
            Size {
                rows: 24,
                cols: MAX_COLS + 1,
            },
        ] {
            assert!(matches!(
                terms.open(&trust, spec(size, Origin::User)),
                Err(TerminalError::BadSize { .. })
            ));
        }
        let long = Origin::Plugin("p".repeat(MAX_PLUGIN_ID_BYTES + 1));
        assert!(matches!(
            terms.open(&trust, spec(Size { rows: 24, cols: 80 }, long)),
            Err(TerminalError::TooLarge { .. })
        ));
        assert!(terms.tabs().is_empty());
    }

    #[test]
    fn unknown_tabs_are_reported() {
        let mut terms = Terminals::new();
        assert!(matches!(
            terms.close(TabId(7)),
            Err(TerminalError::NoSuchTab(TabId(7)))
        ));
        assert!(matches!(
            terms.write(TabId(7), &vec![b'x'; MAX_WRITE_BYTES + 1]),
            Err(TerminalError::TooLarge { .. })
        ));
    }

    #[test]
    fn row_text_trims_trailing_blanks() {
        let mut cells = vec![Cell::default(); 4];
        cells[0].text = "a".into();
        cells[2].text = "b".into();
        let snap = Snapshot {
            size: Size { rows: 1, cols: 4 },
            cells,
        };
        assert_eq!(snap.row_text(0), "a b");
        assert_eq!(snap.row_text(1), "");
    }
}
