// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! One tab: the PTY, its process, and the threads that move bytes between
//! them and the screen model.

use std::io::{Read, Write};
use std::num::NonZeroU16;
use std::path::Path;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use kerf_core::SpawnPermit;
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::{
    Cell, Color, HISTORY_ROWS, Info, MAX_PENDING_WRITES, MAX_TITLE_BYTES, Origin, ProcessState,
    Size, Snapshot, Spec, TerminalError, attr,
};

/// What the reader thread and the UI thread share.
struct Screen {
    parser: vt100::Parser<Titles>,
    output_bytes: u64,
    revision: u64,
}

/// Catches the OSC window title, which vt100 reports only through a callback.
#[derive(Default)]
struct Titles {
    title: String,
}

impl vt100::Callbacks for Titles {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        let title = String::from_utf8_lossy(title);
        self.title = truncate(&title, MAX_TITLE_BYTES).to_owned();
    }
}

pub(crate) struct Session {
    origin: Origin,
    program: String,
    master: Box<dyn MasterPty + Send>,
    /// `None` only while dropping, when the reaper thread takes it.
    child: Option<Box<dyn Child + Send + Sync>>,
    exit: Option<u32>,
    input: SyncSender<Vec<u8>>,
    screen: Arc<Mutex<Screen>>,
}

fn pty_error(e: impl std::fmt::Display) -> TerminalError {
    TerminalError::Pty(e.to_string())
}

fn pty_size(size: Size) -> PtySize {
    PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn nonzero(size: Size) -> (NonZeroU16, NonZeroU16) {
    // `check_size` refused zero before any session call; MIN is unreachable.
    (
        NonZeroU16::new(size.rows).unwrap_or(NonZeroU16::MIN),
        NonZeroU16::new(size.cols).unwrap_or(NonZeroU16::MIN),
    )
}

fn lock(screen: &Mutex<Screen>) -> MutexGuard<'_, Screen> {
    // A panic inside vt100 must not wedge the tab; the state it left is
    // still a valid grid.
    screen.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Session {
    /// Starts the process in `permit.cwd()`; the permit is consumed so one
    /// trust check covers one spawn.
    pub(crate) fn spawn(permit: SpawnPermit, spec: Spec) -> Result<Self, TerminalError> {
        let pair = native_pty_system()
            .openpty(pty_size(spec.size))
            .map_err(pty_error)?;
        let mut cmd = match spec.argv.split_first() {
            Some((program, args)) => {
                let mut cmd = CommandBuilder::new(program);
                cmd.args(args);
                cmd
            }
            None => CommandBuilder::new_default_prog(),
        };
        let program = match spec.argv.first() {
            Some(p) => p.clone(),
            None => cmd.get_shell(),
        };
        cmd.cwd(permit.cwd());
        // The screen model speaks xterm; programs that check pick colours.
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        let child = pair.slave.spawn_command(cmd).map_err(pty_error)?;
        // The parent's copy of the slave would keep the reader from seeing
        // EOF after the process exits.
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().map_err(pty_error)?;
        let writer = pair.master.take_writer().map_err(pty_error)?;

        let (rows, cols) = nonzero(spec.size);
        let screen = Arc::new(Mutex::new(Screen {
            parser: vt100::Parser::new_with_callbacks(rows, cols, HISTORY_ROWS, Titles::default()),
            output_bytes: 0,
            revision: 0,
        }));
        let (input, queue) = sync_channel(MAX_PENDING_WRITES);
        spawn_reader(reader, Arc::clone(&screen))?;
        spawn_writer(writer, queue)?;
        Ok(Self {
            origin: spec.origin,
            program,
            master: pair.master,
            child: Some(child),
            exit: None,
            input,
            screen,
        })
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        if bytes.is_empty() {
            return Ok(());
        }
        // The writer thread blocks on a full PTY, never the UI thread.
        self.input.try_send(bytes.to_vec()).map_err(|e| match e {
            TrySendError::Full(_) => TerminalError::InputFull,
            TrySendError::Disconnected(_) => TerminalError::Exited,
        })
    }

    pub(crate) fn resize(&mut self, size: Size) -> Result<(), TerminalError> {
        self.master.resize(pty_size(size)).map_err(pty_error)?;
        let (rows, cols) = nonzero(size);
        let mut screen = lock(&self.screen);
        screen.parser.set_size(rows, cols);
        screen.revision += 1;
        Ok(())
    }

    pub(crate) fn scroll(&mut self, offset: usize) -> usize {
        let mut screen = lock(&self.screen);
        screen.parser.screen_mut().set_scrollback(offset);
        screen.revision += 1;
        screen.parser.screen().scrollback()
    }

    fn state(&mut self) -> ProcessState {
        if self.exit.is_none() {
            // An error here means the process is gone without a status.
            self.exit = match self.child.as_mut().map(|c| c.try_wait()) {
                Some(Ok(None)) => None,
                None => Some(1),
                Some(Ok(Some(status))) => Some(status.exit_code()),
                Some(Err(_)) => Some(1),
            };
        }
        match self.exit {
            None => ProcessState::Running,
            Some(code) => ProcessState::Exited { code },
        }
    }

    pub(crate) fn info(&mut self) -> Info {
        let state = self.state();
        let screen = lock(&self.screen);
        let vt = screen.parser.screen();
        let (rows, cols) = vt.size();
        let osc = &screen.parser.callbacks().title;
        let title = if osc.is_empty() {
            program_name(&self.program)
        } else {
            osc.clone()
        };
        Info {
            title,
            origin: self.origin.clone(),
            state,
            size: Size {
                rows: rows.get(),
                cols: cols.get(),
            },
            cursor: vt.cursor_position(),
            cursor_visible: !vt.hide_cursor(),
            scroll_offset: vt.scrollback(),
            history_rows: vt.scrollback_len(),
            output_bytes: screen.output_bytes,
            // Exit changes the footer, so it counts as a change too.
            revision: screen.revision + u64::from(self.exit.is_some()),
        }
    }

    pub(crate) fn snapshot(&self) -> Snapshot {
        let screen = lock(&self.screen);
        let vt = screen.parser.screen();
        let (rows, cols) = vt.size();
        let mut cells = Vec::with_capacity(usize::from(rows.get()) * usize::from(cols.get()));
        for row in 0..rows.get() {
            for col in 0..cols.get() {
                cells.push(vt.cell(row, col).map(cell).unwrap_or_default());
            }
        }
        Snapshot {
            size: Size {
                rows: rows.get(),
                cols: cols.get(),
            },
            cells,
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let running = self.state() == ProcessState::Running;
        let Some(mut child) = self.child.take() else {
            return;
        };
        if running {
            let _ = child.kill();
        }
        // Reap off the UI thread so a closed tab leaves no zombie and a slow
        // exit never blocks a close.
        let _ = thread::Builder::new()
            .name("kerf-terminal-reap".into())
            .spawn(move || child.wait());
    }
}

fn spawn_reader(
    mut reader: Box<dyn Read + Send>,
    screen: Arc<Mutex<Screen>>,
) -> Result<(), TerminalError> {
    let read = move || {
        let mut buf = vec![0u8; 64 * 1024];
        // EOF or an error (EIO on Linux once the process is gone) ends the tab's output.
        while let Ok(n @ 1..) = reader.read(&mut buf) {
            let mut screen = lock(&screen);
            screen.parser.process(&buf[..n]);
            screen.output_bytes += n as u64;
            screen.revision += 1;
        }
    };
    thread::Builder::new()
        .name("kerf-terminal-read".into())
        .spawn(read)
        .map(drop)
        .map_err(pty_error)
}

fn spawn_writer(
    mut writer: Box<dyn Write + Send>,
    queue: Receiver<Vec<u8>>,
) -> Result<(), TerminalError> {
    let write = move || {
        for chunk in queue {
            if writer
                .write_all(&chunk)
                .and_then(|()| writer.flush())
                .is_err()
            {
                break;
            }
        }
    };
    thread::Builder::new()
        .name("kerf-terminal-write".into())
        .spawn(write)
        .map(drop)
        .map_err(pty_error)
}

fn cell(c: &vt100::Cell) -> Cell {
    let flags = [
        (c.bold(), attr::BOLD),
        (c.dim(), attr::DIM),
        (c.italic(), attr::ITALIC),
        (c.underline(), attr::UNDERLINE),
        (c.inverse(), attr::INVERSE),
        (c.is_wide(), attr::WIDE),
        (c.is_wide_continuation(), attr::WIDE_CONTINUATION),
    ];
    Cell {
        text: c.contents().to_owned(),
        fg: color(c.fgcolor()),
        bg: color(c.bgcolor()),
        attrs: flags
            .iter()
            .filter(|(on, _)| *on)
            .fold(0, |a, (_, b)| a | b),
    }
}

fn color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Default => Color::Default,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn program_name(program: &str) -> String {
    Path::new(program)
        .file_stem()
        .map_or_else(|| program.to_owned(), |s| s.to_string_lossy().into_owned())
}

fn truncate(s: &str, cap: usize) -> &str {
    let mut end = s.len().min(cap);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_cut_on_a_character_boundary() {
        let mut parser = vt100::Parser::new_with_callbacks(
            NonZeroU16::MIN,
            NonZeroU16::MIN,
            0,
            Titles::default(),
        );
        let long = "é".repeat(MAX_TITLE_BYTES);
        parser.process(format!("\x1b]2;{long}\x07").as_bytes());
        let title = &parser.callbacks().title;
        assert_eq!(title.len(), MAX_TITLE_BYTES);
        assert!(title.chars().all(|c| c == 'é'));
    }

    #[test]
    fn the_title_falls_back_to_the_program_name() {
        assert_eq!(program_name("/bin/zsh"), "zsh");
        assert_eq!(program_name("cargo"), "cargo");
    }
}
