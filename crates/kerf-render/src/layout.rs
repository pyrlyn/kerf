// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Text layout without a GPU: shapes a frame and finds the caret, so hosts
//! and tests can place the caret before anything is drawn.

use glyphon::cosmic_text::{BufferLine, LineEnding};
use glyphon::{Attrs, AttrsList, Buffer, Cursor, Family, FontSystem, Metrics, Shaping, Wrap};
use kerf_core::Frame;

use crate::{Rect, Size, Theme};

/// Caps on host-supplied numbers: the theme and size cross the C ABI, and a
/// zero, NaN or absurd value would panic in `cosmic-text` or fill the glyph
/// atlas.
const MAX_SCALE: f32 = 8.0;
const MAX_FONT_PX: f32 = 512.0;

/// Line breaks ropey counts (its default `unicode_lines`), so layout lines
/// are the core's lines and [`Frame::line`] indexes them directly.
/// `cosmic-text` alone splits only on CR and LF, and reads "\n\r" as one break.
const BREAKS: [char; 7] = [
    '\n', '\r', '\u{0B}', '\u{0C}', '\u{85}', '\u{2028}', '\u{2029}',
];

/// Shaped text of the last frame, and the fonts it was shaped with.
pub struct Layout {
    pub(crate) fonts: FontSystem,
    pub(crate) text: Buffer,
}

impl Layout {
    /// A layout over the system's fonts. Loading them takes a while, so a
    /// host makes one and keeps it.
    #[must_use]
    pub fn new() -> Self {
        Self::with_fonts(FontSystem::new())
    }

    /// A layout over `fonts`, for hosts that bundle or filter their own.
    #[must_use]
    pub fn with_fonts(mut fonts: FontSystem) -> Self {
        let mut text = Buffer::new(&mut fonts, Metrics::new(13.0, 17.0));
        // Code is not reflowed; long lines scroll sideways, as in Xcode and VS.
        text.set_wrap(Wrap::None);
        Self { fonts, text }
    }

    /// Shape `frame` for a view of `size` and return the caret rectangle,
    /// or `None` when the caret's line is outside the view.
    pub fn update(&mut self, frame: &Frame<'_>, theme: &Theme, size: Size) -> Option<Rect> {
        let scale = sane(size.scale, 1.0 / MAX_SCALE, MAX_SCALE, 1.0);
        let font_px = sane(theme.font_size * scale, 1.0, MAX_FONT_PX, 13.0);
        let line_px = sane(theme.line_height * scale, 1.0, MAX_FONT_PX * 2.0, font_px);
        self.text.set_metrics(Metrics::new(font_px, line_px));
        self.text
            .set_size(Some(size.width as f32), Some(size.height as f32));

        let family = theme
            .font_family
            .as_deref()
            .map_or(Family::Monospace, Family::Name);
        let attrs = AttrsList::new(&Attrs::new().family(family));
        let buffer = frame.buffer;
        // `set_text` is what marks the text as replaced; without that mark
        // `shape_until_scroll` keeps the old shaping and the new lines stay
        // unlaid. The lines themselves come from the core, split its way.
        // Rebuilt every frame; redraw-on-request and line caching come later.
        self.text
            .set_text("", &Attrs::new(), Shaping::Advanced, None);
        self.text.lines = (0..buffer.len_lines())
            .map(|i| {
                let line = without_break(buffer.line(i).unwrap_or_default());
                // The ending only matters to `cosmic-text`'s own editing, which
                // Kerf does not use; the core keeps the real one.
                BufferLine::new(
                    line,
                    LineEnding::default(),
                    attrs.clone(),
                    Shaping::Advanced,
                )
            })
            .collect();
        self.text.shape_until_scroll(&mut self.fonts, false);

        let (x, y) = self
            .text
            .cursor_position(&Cursor::new(frame.line, frame.col))?;
        Some(Rect {
            x,
            y,
            width: sane(theme.caret_width * scale, 1.0, MAX_FONT_PX, 1.0),
            height: line_px,
        })
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

fn without_break(mut line: String) -> String {
    if line.ends_with("\r\n") {
        line.truncate(line.len() - 2);
    } else if line.ends_with(BREAKS) {
        line.pop();
    }
    line
}

fn sane(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::Layout;
    use crate::{Size, Theme};
    use kerf_core::{Editor, TextBuffer};

    const VIEW: Size = Size {
        width: 800,
        height: 600,
        scale: 1.0,
    };

    fn caret_after(layout: &mut Layout, text: &str, size: Size) -> crate::Rect {
        let mut ed = Editor::new(TextBuffer::new());
        ed.insert(text).unwrap();
        layout.update(&ed.frame(), &Theme::default(), size).unwrap()
    }

    #[test]
    fn caret_follows_the_text_on_its_line() {
        let mut layout = Layout::new();
        let start = caret_after(&mut layout, "", VIEW);
        let one = caret_after(&mut layout, "a", VIEW);
        let two = caret_after(&mut layout, "aa", VIEW);
        assert_eq!((start.x, start.y), (0.0, 0.0));
        assert!(one.x > 0.0);
        // Monospace: every column is the same width.
        assert!((two.x - 2.0 * one.x).abs() < 0.5, "{one:?} {two:?}");
        assert_eq!(two.height, Theme::default().line_height);
    }

    #[test]
    fn caret_on_the_second_line_sits_one_line_down() {
        let mut layout = Layout::new();
        let caret = caret_after(&mut layout, "first\nb", VIEW);
        let one = caret_after(&mut layout, "b", VIEW);
        assert_eq!(caret.y, Theme::default().line_height);
        assert_eq!(caret.x, one.x);
    }

    #[test]
    fn layout_lines_are_the_core_lines() {
        // ropey breaks at U+2028 and twice in "\n\r"; cosmic-text alone would not.
        let mut layout = Layout::new();
        let line = Theme::default().line_height;
        assert_eq!(caret_after(&mut layout, "a\u{2028}", VIEW).y, line);
        let caret = caret_after(&mut layout, "a\n\r", VIEW);
        assert_eq!((caret.x, caret.y), (0.0, 2.0 * line));
    }

    #[test]
    fn scale_turns_logical_sizes_into_physical_pixels() {
        let mut layout = Layout::new();
        let one = caret_after(&mut layout, "a\nb", VIEW);
        let two = caret_after(&mut layout, "a\nb", Size { scale: 2.0, ..VIEW });
        assert_eq!(two.y, 2.0 * one.y);
        assert_eq!(two.width, 2.0 * one.width);
        assert!((two.x - 2.0 * one.x).abs() < 1.0, "{one:?} {two:?}");
    }

    #[test]
    fn caret_below_the_view_is_not_reported() {
        let mut layout = Layout::new();
        let mut ed = Editor::new(TextBuffer::new());
        ed.insert(&"\n".repeat(100)).unwrap();
        let short = Size { height: 40, ..VIEW };
        assert!(
            layout
                .update(&ed.frame(), &Theme::default(), short)
                .is_none()
        );
    }

    #[test]
    fn broken_host_numbers_do_not_panic() {
        let mut layout = Layout::new();
        let theme = Theme {
            font_size: f32::NAN,
            line_height: 0.0,
            caret_width: -3.0,
            ..Theme::default()
        };
        let ed = Editor::new(TextBuffer::with_text("x"));
        let size = Size {
            width: 0,
            height: 0,
            scale: f32::INFINITY,
        };
        let _ = layout.update(&ed.frame(), &theme, size);
    }
}
