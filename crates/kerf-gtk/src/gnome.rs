// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The desktop's side of the look: GNOME's monospace font, measured by
//! Pango, and the theme's text colour, turned into a [`Look`] and a
//! `KerfTheme` for `kerf_view_set_theme`.

use gtk::{gio, pango, prelude::*};
use kerf_ffi as ffi;

use crate::look::{self, Look};

/// GNOME's interface settings, where the monospace font lives.
const INTERFACE: &str = "org.gnome.desktop.interface";
const MONOSPACE: &str = "monospace-font-name";

/// GNOME's settings, when the desktop has them.
#[derive(Debug)]
pub struct Gnome(Option<gio::Settings>);

impl Default for Gnome {
    fn default() -> Self {
        // `Settings::new` aborts on a missing schema, so look first.
        let schema = gio::SettingsSchemaSource::default().and_then(|s| s.lookup(INTERFACE, true));
        Self(schema.map(|_| gio::Settings::new(INTERFACE)))
    }
}

impl Gnome {
    /// Calls `changed` when the user picks another monospace font.
    pub fn on_font_changed(&self, changed: impl Fn() + 'static) {
        if let Some(settings) = &self.0 {
            settings.connect_changed(Some(MONOSPACE), move |_, _| changed());
        }
    }

    /// GNOME's monospace font (GNOME Text Editor's default), else Kerf's own
    /// monospace at GNOME Text Editor's default size.
    pub fn font(&self) -> pango::FontDescription {
        let mut font = self
            .0
            .as_ref()
            .map(|s| pango::FontDescription::from_string(&s.string(MONOSPACE)))
            .filter(|font| font.family().is_some() && font.size() > 0)
            .unwrap_or_default();
        if font.size() <= 0 {
            font.set_size((look::FALLBACK_POINTS * pango::SCALE as f32) as i32);
        }
        font
    }

    /// The Adwaita or Adwaita-dark look for `widget`, in GNOME's font as
    /// `widget`'s Pango context measures it.
    pub fn look(&self, widget: &impl IsA<gtk::Widget>, dark: bool) -> Look {
        let font = self.font();
        let unit = |pango: i32| pango as f32 / pango::SCALE as f32;
        let metrics = widget.pango_context().metrics(Some(&font), None);
        let family = font.family().map(|f| f.to_string()).unwrap_or_default();
        Look::adwaita(
            dark,
            family,
            look::points_to_pixels(unit(font.size())),
            unit(metrics.ascent() + metrics.descent()),
        )
    }
}

/// Whether `widget`'s theme is dark, from its text colour.
pub fn is_dark(widget: &impl IsA<gtk::Widget>) -> bool {
    let color = widget.color();
    look::is_dark(color.red(), color.green(), color.blue())
}

/// `look` for the C ABI. It points into `look`, which must outlive its use.
pub fn kerf_theme(look: &Look) -> ffi::KerfTheme {
    ffi::KerfTheme {
        font_family: look.font_family.as_ptr(),
        font_family_len: look.font_family.len(),
        font_size: look.font_size,
        line_height: look.line_height,
        background: look.background,
        text: look.text,
        selection: look.selection,
        caret: look.caret,
        caret_width: 1.0,
        // The host draws GTK's caret over the frame.
        draw_caret: 0,
    }
}
