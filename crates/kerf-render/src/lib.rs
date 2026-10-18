// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Kerf's GPU editor renderer (`research.md` §7): lays out a
//! [`kerf_core::Frame`] with `cosmic-text` and draws it with `wgpu` and
//! `glyphon` inside a view the native host owns.
//!
//! The host supplies the look in a [`Theme`] (font, colours, caret), so the
//! same renderer looks like Xcode's editor on macOS and Visual Studio's on
//! Windows. It reports the caret rectangle so the host can draw the system
//! caret, and draws its own only when asked.

// Unit tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod gpu;
mod layout;

pub use gpu::{Error, GpuRenderer, OFFSCREEN_FORMAT, Offscreen};
pub use layout::Layout;

/// An sRGB colour, red, green, blue, alpha.
pub type Rgba = [u8; 4];

/// The look of the editor, supplied by the host. Sizes are logical pixels;
/// the renderer multiplies them by [`Size::scale`].
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
    /// Caret colour, used only when [`Theme::draw_caret`] is set.
    pub caret: Rgba,
    /// Caret width.
    pub caret_width: f32,
    /// Draw the caret on the GPU. Hosts with a system caret
    /// (`NSTextInsertionIndicator`) leave it off and place theirs at
    /// [`GpuRenderer::caret`].
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
            caret: [0, 0, 0, 255],
            caret_width: 1.0,
            draw_caret: false,
        }
    }
}

/// The drawable area of the host view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
    /// Physical pixels per logical pixel (2 on a Retina display).
    pub scale: f32,
}

/// A rectangle in physical pixels from the view's top-left corner.
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
