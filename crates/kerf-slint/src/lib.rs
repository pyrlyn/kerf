// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Kerf's editor surface as a Slint component (`research.md` §7): draws a
//! [`kerf_core::Frame`] through `slint-bindings-core`'s [`EmbeddedHost`] on
//! the CPU, into an RGBA buffer the native host owns.
//!
//! The host supplies the look in a [`Theme`] and reads the caret rectangle
//! back, so on macOS it keeps the system caret (`NSTextInsertionIndicator`).
//! Everything here is single-threaded, as Slint is: call it from the thread
//! that created the view.

use std::rc::Rc;

use kerf_core::{Frame, Render};
use slint::{Color, ComponentHandle, ModelRc, SharedString, VecModel};
use slint_bindings_core::EmbeddedHost;

mod ui {
    // Code generated from `ui/editor.slint` by `slint-build`; Kerf's lint
    // rules are for code written here, not for Slint's generator.
    #![allow(
        missing_docs,
        clippy::all,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::todo
    )]
    slint::include_modules!();
}

use ui::{EditorComponent, EditorLine};

/// Caps on host-supplied sizes: the theme will cross the C ABI, and a zero,
/// NaN or absurd value must not reach the text layout.
const MAX_FONT: f32 = 512.0;

/// The face used when the theme names none. Slint has no generic monospace
/// family name, so each OS gets a monospace face it ships.
#[cfg(target_os = "macos")]
const MONOSPACE: &str = "Menlo";
#[cfg(target_os = "windows")]
const MONOSPACE: &str = "Consolas";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const MONOSPACE: &str = "DejaVu Sans Mono";

/// Line breaks ropey counts (its default `unicode_lines`), so a drawn line is
/// exactly one of the core's lines and [`Frame::line`] indexes it.
const BREAKS: [char; 7] = [
    '\n', '\r', '\u{0B}', '\u{0C}', '\u{85}', '\u{2028}', '\u{2029}',
];

/// Bytes per pixel in the frames [`SlintView::draw`] writes.
pub const BYTES_PER_PIXEL: usize = 4;

/// An sRGB colour, red, green, blue, alpha.
pub type Rgba = [u8; 4];

/// Why the view failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The embedding refused: a bad scale, a short buffer, a call from
    /// another thread or a Slint platform error.
    #[error(transparent)]
    Slint(#[from] slint_bindings_core::Error),
}

/// The look of the editor, supplied by the host. Sizes are logical points;
/// Slint multiplies them by the scale passed to [`SlintView::resize`].
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Font family name, e.g. "SF Mono"; `None` means the system monospace.
    pub font_family: Option<String>,
    /// Font size.
    pub font_size: f32,
    /// Distance between baselines.
    pub line_height: f32,
    /// Editor background.
    pub background: Rgba,
    /// Default text colour.
    pub text: Rgba,
    /// Fill behind selected text.
    pub selection: Rgba,
    /// Caret colour, used only when [`Theme::draw_caret`] is set.
    pub caret: Rgba,
    /// Caret width.
    pub caret_width: f32,
    /// Draw the caret in the view. Hosts with a system caret
    /// (`NSTextInsertionIndicator`) leave it off and place theirs at
    /// [`SlintView::caret`].
    pub draw_caret: bool,
}

impl Default for Theme {
    /// Kerf's own light theme, for hosts that have no platform theme to read.
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: 13.0,
            line_height: 17.0,
            background: [255, 255, 255, 255],
            text: [29, 29, 31, 255],
            // macOS's light selected-text background.
            selection: [179, 215, 255, 255],
            caret: [0, 0, 0, 255],
            caret_width: 1.0,
            draw_caret: false,
        }
    }
}

/// A rectangle in logical points from the view's top-left corner, the unit
/// AppKit and WinUI place their views in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// The editor component and the frame size it draws at.
pub struct SlintView {
    host: EmbeddedHost<EditorComponent>,
    lines: Rc<VecModel<EditorLine>>,
    line_height: f32,
    caret_width: f32,
    /// The caret's line was among the lines drawn by the last frame.
    caret_shown: bool,
}

impl SlintView {
    /// A view of `width`×`height` physical pixels at `scale` physical pixels
    /// per logical point, showing an empty buffer until the first frame.
    pub fn new(width: u32, height: u32, scale: f32, theme: &Theme) -> Result<Self, Error> {
        let host = EmbeddedHost::new(EditorComponent::new, width, height, scale)?;
        let lines = Rc::new(VecModel::default());
        host.component().set_lines(ModelRc::from(lines.clone()));
        let mut view = Self {
            host,
            lines,
            line_height: 0.0,
            caret_width: 0.0,
            caret_shown: false,
        };
        view.set_theme(theme);
        Ok(view)
    }

    /// Resizes the frame. A zero dimension becomes one pixel. Lines that a
    /// taller view brings into sight appear with the next [`Render::render`].
    pub fn resize(&mut self, width: u32, height: u32, scale: f32) -> Result<(), Error> {
        Ok(self.host.resize(width, height, scale)?)
    }

    /// Replaces the look. Sizes that are not finite or out of range fall back
    /// or are clamped, so a broken host value cannot break the layout.
    pub fn set_theme(&mut self, theme: &Theme) {
        let ui = self.host.component();
        let font = sane(theme.font_size, 1.0, MAX_FONT, 13.0);
        self.line_height = sane(theme.line_height, 1.0, MAX_FONT * 2.0, font);
        self.caret_width = sane(theme.caret_width, 1.0, MAX_FONT, 1.0);
        ui.set_family(theme.font_family.as_deref().unwrap_or(MONOSPACE).into());
        ui.set_text_size(font);
        ui.set_line_height(self.line_height);
        ui.set_caret_width(self.caret_width);
        ui.set_background_color(color(theme.background));
        ui.set_text_color(color(theme.text));
        ui.set_selection_color(color(theme.selection));
        ui.set_caret_color(color(theme.caret));
        ui.set_draw_caret(theme.draw_caret);
    }

    /// Frame size in physical pixels.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        self.host.size()
    }

    /// Bytes a buffer passed to [`Self::draw`] must hold.
    #[must_use]
    pub fn frame_len(&self) -> usize {
        self.host.frame_len()
    }

    /// Paints into `out` (premultiplied RGBA8, rows top to bottom, no
    /// padding) and returns whether it did. When nothing changed since the
    /// last call, `out` is left as it was and the host keeps showing it.
    pub fn draw(&mut self, out: &mut [u8]) -> Result<bool, Error> {
        Ok(self.host.render(out)?.redrawn)
    }

    /// The caret of the last frame in logical points, or `None` when its
    /// line is outside the view.
    #[must_use]
    pub fn caret(&self) -> Option<Rect> {
        if !self.caret_shown {
            return None;
        }
        let ui = self.host.component();
        let y = ui.get_caret_y();
        let (_, height) = self.host.size();
        if y >= height as f32 / ui.window().scale_factor() {
            return None;
        }
        Some(Rect {
            x: ui.get_caret_x(),
            y,
            width: self.caret_width,
            height: self.line_height,
        })
    }

    /// Lines that fit in the view, plus one partly visible at the bottom.
    fn visible_lines(&self) -> usize {
        let (_, height) = self.host.size();
        let points = height as f32 / self.host.component().window().scale_factor();
        // Both are finite and positive: the scale is checked by the host and
        // the line height clamped by `set_theme`.
        (points / self.line_height).ceil() as usize + 1
    }
}

impl Render for SlintView {
    type Error = Error;

    fn render(&mut self, frame: &Frame<'_>) -> Result<(), Error> {
        let buffer = frame.buffer;
        let count = buffer.len_lines().min(self.visible_lines());
        let selection = frame.selection().map(|range| {
            let start = buffer.line_col(range.start);
            let end = buffer.line_col(range.end);
            (start, end)
        });
        let mut caret_prefix = SharedString::new();
        let mut rows = Vec::with_capacity(count);
        for i in 0..count {
            let text = without_break(buffer.line(i).unwrap_or_default());
            if i == frame.line {
                caret_prefix = prefix(&text, frame.col).into();
            }
            rows.push(row(text, i, selection));
        }
        self.caret_shown = frame.line < count;
        // Every visible line is rebuilt each frame; redraw-on-change and line
        // caching come with scrolling.
        self.lines.set_vec(rows);
        let ui = self.host.component();
        ui.set_caret_prefix(caret_prefix);
        ui.set_caret_line(i32::try_from(frame.line).unwrap_or(i32::MAX));
        Ok(())
    }
}

/// One drawn line and the part of the selection on it.
fn row(text: String, i: usize, selection: Option<((usize, usize), (usize, usize))>) -> EditorLine {
    let mut line = EditorLine {
        text: SharedString::new(),
        selected: false,
        selection_before: SharedString::new(),
        selected_text: SharedString::new(),
        selection_to_end: false,
    };
    if let Some(((start_line, start_col), (end_line, end_col))) = selection
        && (start_line..=end_line).contains(&i)
    {
        let from = if i == start_line { start_col } else { 0 };
        let to = if i == end_line { end_col } else { text.len() };
        let before = prefix(&text, from);
        line.selected = true;
        line.selection_before = before.into();
        line.selected_text = prefix(&text, to).get(before.len()..).unwrap_or("").into();
        line.selection_to_end = i < end_line;
    }
    line.text = text.into();
    line
}

/// `text` up to byte `col`, or all of it when `col` is past the end or not
/// on a character boundary (a cursor between a CR and its LF).
fn prefix(text: &str, col: usize) -> &str {
    text.get(..col).unwrap_or(text)
}

fn without_break(mut line: String) -> String {
    if line.ends_with("\r\n") {
        line.truncate(line.len() - 2);
    } else if line.ends_with(BREAKS) {
        line.pop();
    }
    line
}

fn color([r, g, b, a]: Rgba) -> Color {
    Color::from_argb_u8(a, r, g, b)
}

fn sane(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}
