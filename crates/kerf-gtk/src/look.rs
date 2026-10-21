// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! GNOME Text Editor's look and GTK's caret rules as plain values, so they are
//! tested without a display. Sources are in `research.md` §7, "Linux host (T17)".

/// An sRGB colour, red, green, blue, alpha (not premultiplied).
pub type Rgba = [u8; 4];

/// GNOME Text Editor's `line-height` default: a factor on the font's own line.
pub const LINE_FACTOR: f32 = 1.2;

/// GNOME Text Editor's `custom-font` default, used when GNOME's monospace font
/// setting is missing.
pub const FALLBACK_POINTS: f32 = 11.0;

/// The editor's colours and font, in GTK's logical pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    /// Font family; empty for Kerf's own monospace.
    pub font_family: String,
    /// Font size.
    pub font_size: f32,
    /// Distance between baselines.
    pub line_height: f32,
    /// Background.
    pub background: Rgba,
    /// Text.
    pub text: Rgba,
    /// Fill behind selected text.
    pub selection: Rgba,
    /// Caret.
    pub caret: Rgba,
}

impl Look {
    /// GtkSourceView's Adwaita or Adwaita-dark scheme, GNOME Text Editor's
    /// default, with GTK's own text selection colour (the scheme sets none).
    /// `natural_line` is the font's ascent plus descent.
    pub fn adwaita(dark: bool, font_family: String, font_size: f32, natural_line: f32) -> Self {
        let (background, text, caret, selection) = if dark {
            // libadwaita-dark, light_5, light_5; darken(#3584e4, 20%) at 0.8.
            (
                [0x1d, 0x1d, 0x20, 255],
                [0xc0, 0xbf, 0xbc, 255],
                [0xc0, 0xbf, 0xbc, 255],
                [21, 83, 158, 204],
            )
        } else {
            // light_1, dark_3, dark_1; #3584e4 at 0.3.
            (
                [0xff, 0xff, 0xff, 255],
                [0x50, 0x4e, 0x55, 255],
                [0x77, 0x76, 0x7b, 255],
                [0x35, 0x84, 0xe4, 77],
            )
        };
        Self {
            font_family,
            font_size,
            line_height: (natural_line.max(font_size) * LINE_FACTOR).round(),
            background,
            text,
            selection,
            caret,
        }
    }
}

/// Pango points at GTK's 96 dpi in logical pixels.
pub fn points_to_pixels(points: f32) -> f32 {
    points * 96.0 / 72.0
}

/// Whether a theme whose text has this colour is dark: light text means a
/// dark background. GTK 4.14 has no public "is the theme dark" call.
pub fn is_dark(red: f32, green: f32, blue: f32) -> bool {
    0.2126 * red + 0.7152 * green + 0.0722 * blue > 0.5
}

/// GTK's caret width for a caret `height` tall: `height * aspect + 1`, in
/// whole pixels, as `gtkrenderlayout.c` draws it.
pub fn caret_width(height: f32, aspect: f64) -> f32 {
    (f64::from(height) * aspect + 1.0).floor() as f32
}

/// GtkTextView's caret blink: a cycle split evenly into visible, fading out,
/// hidden and fading in, stopped (caret on) after a timeout without input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blink {
    start: i64,
    first: i64,
    cycle: i64,
    timeout: i64,
}

impl Blink {
    /// A blink starting at `now` (microseconds, GLib's monotonic clock) with a
    /// `cycle_ms` cycle that stops after `timeout_s` seconds. After an edit
    /// GTK delays the first fade by half a cycle; `delay` does the same.
    pub fn new(now: i64, cycle_ms: i32, timeout_s: i32, delay: bool) -> Self {
        let cycle = i64::from(cycle_ms.max(1)) * 1000;
        Self {
            start: now,
            first: if delay { now + cycle / 2 } else { now },
            cycle,
            timeout: i64::from(timeout_s.max(0)) * 1_000_000,
        }
    }

    /// The caret's opacity at `now`; `None` once it has blinked long enough
    /// and stays fully visible.
    pub fn alpha(&self, now: i64) -> Option<f32> {
        if now > self.start + self.timeout {
            return None;
        }
        if now < self.first {
            return Some(1.0);
        }
        let phase = ((now - self.first) % self.cycle) as f32 / self.cycle as f32;
        Some(if phase < 0.25 {
            1.0
        } else if phase < 0.5 {
            1.0 - 4.0 * (phase - 0.25)
        } else if phase < 0.75 {
            0.0
        } else {
            4.0 * (phase - 0.75)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_height_follows_gnome_text_editor() {
        let look = Look::adwaita(false, String::new(), 14.0, 17.0);
        assert_eq!(look.line_height, 20.0);
        assert_eq!(look.background, [255, 255, 255, 255]);
        let dark = Look::adwaita(true, String::new(), 14.0, 17.0);
        assert_ne!(dark.background, look.background);
    }

    #[test]
    fn eleven_points_are_gnome_pixels() {
        assert!((points_to_pixels(FALLBACK_POINTS) - 14.666_667).abs() < 1e-4);
    }

    #[test]
    fn light_text_is_a_dark_theme() {
        assert!(is_dark(0.75, 0.75, 0.74));
        assert!(!is_dark(0.31, 0.31, 0.33));
    }

    #[test]
    fn caret_width_is_gtks() {
        // The default aspect ratio 0.04: a 20 px caret is 1 px, a 50 px one 3 px.
        assert_eq!(caret_width(20.0, 0.04), 1.0);
        assert_eq!(caret_width(50.0, 0.04), 3.0);
    }

    #[test]
    fn blink_fades_and_stops() {
        let blink = Blink::new(0, 1200, 10, false);
        assert_eq!(blink.alpha(0), Some(1.0));
        assert_eq!(blink.alpha(450_000), Some(0.5));
        assert_eq!(blink.alpha(700_000), Some(0.0));
        assert_eq!(blink.alpha(1_200_000), Some(1.0));
        assert_eq!(blink.alpha(10_000_001), None);
    }

    #[test]
    fn an_edit_delays_the_fade() {
        let blink = Blink::new(0, 1200, 10, true);
        assert_eq!(blink.alpha(500_000), Some(1.0));
        assert_eq!(blink.alpha(600_000 + 700_000), Some(0.0));
    }
}
