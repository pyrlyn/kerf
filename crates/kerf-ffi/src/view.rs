// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The editor view: `kerf-slint` drawing on the CPU into a frame the host
//! copies out and presents in its own view.

use std::thread::{self, ThreadId};

use kerf_core::Render;
use kerf_slint::{SlintView, Theme};

use crate::{KerfEditor, KerfStatus, editor, guard, put, text};

/// Longest font family name the host can pass.
const MAX_FAMILY_BYTES: usize = 256;

/// The editor's look, in logical points (`kerf_slint::Theme`). Colours are
/// sRGB red, green, blue, alpha.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct KerfTheme {
    /// UTF-8 family name such as "SF Mono"; null or empty for the system monospace.
    pub font_family: *const u8,
    /// Bytes in `font_family`.
    pub font_family_len: usize,
    /// Font size.
    pub font_size: f32,
    /// Distance between baselines.
    pub line_height: f32,
    /// Background.
    pub background: [u8; 4],
    /// Text.
    pub text: [u8; 4],
    /// Fill behind selected text.
    pub selection: [u8; 4],
    /// Caret, when Kerf draws it.
    pub caret: [u8; 4],
    /// Caret width.
    pub caret_width: f32,
    /// Non-zero: Kerf draws the caret. Zero: the host draws the system caret
    /// at `kerf_view_caret`. A byte, not `bool`, so any value is defined.
    pub draw_caret: u8,
}

/// A rectangle in logical points from the view's top-left corner, the unit
/// AppKit and WinUI place their views in.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KerfRect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// A view the editor is drawn into. Opaque to the host.
pub struct KerfView {
    /// Slint objects are `!Send`: only the creating thread may touch them.
    owner: ThreadId,
    view: SlintView,
    /// The last frame. Kept between renders because Slint repaints only what
    /// changed into the buffer it painted before.
    frame: Vec<u8>,
}

impl From<kerf_slint::Error> for KerfStatus {
    fn from(_: kerf_slint::Error) -> Self {
        Self::Render
    }
}

/// The theme behind a host pointer; null means Kerf's own.
///
/// # Safety
///
/// `theme` is null or points to a readable `KerfTheme` whose `font_family`
/// points to `font_family_len` readable bytes.
unsafe fn theme(theme: *const KerfTheme) -> Result<Theme, KerfStatus> {
    // SAFETY: the caller promised null or a readable theme.
    let Some(t) = (unsafe { theme.as_ref() }) else {
        return Ok(Theme::default());
    };
    // SAFETY: forwarded from the caller.
    let family = unsafe { text(t.font_family, t.font_family_len, MAX_FAMILY_BYTES) }?;
    Ok(Theme {
        font_family: (!family.is_empty()).then(|| family.to_owned()),
        font_size: t.font_size,
        line_height: t.line_height,
        background: t.background,
        text: t.text,
        selection: t.selection,
        caret: t.caret,
        caret_width: t.caret_width,
        draw_caret: t.draw_caret != 0,
    })
}

/// The view behind a host handle, refused off the thread that made it.
///
/// # Safety
///
/// `view` is null or a live handle no other call is using.
unsafe fn view<'a>(view: *mut KerfView) -> Result<&'a mut KerfView, KerfStatus> {
    // SAFETY: the caller promised a live, unaliased handle or null.
    let view = unsafe { view.as_mut() }.ok_or(KerfStatus::NullPointer)?;
    if view.owner != thread::current().id() {
        return Err(KerfStatus::WrongThread);
    }
    Ok(view)
}

/// A view of `width`×`height` physical pixels at `scale` pixels per point
/// (2 on Retina). `theme` may be null. Every later call on the view comes
/// from the thread that made it, the host's UI thread: Slint is
/// single-threaded, and a call from another thread gets
/// `KERF_STATUS_WRONG_THREAD`.
///
/// # Safety
///
/// `theme` is null or a readable `KerfTheme`; `out` is valid for a write of
/// one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_view_new(
    width: u32,
    height: u32,
    scale: f32,
    theme: *const KerfTheme,
    out: *mut *mut KerfView,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let theme = unsafe { self::theme(theme) }?;
        if out.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let view = SlintView::new(width, height, scale, &theme)?;
        let view = Box::into_raw(Box::new(KerfView {
            owner: thread::current().id(),
            view,
            frame: Vec::new(),
        }));
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out, view) }
    })
}

/// Free a view on the thread that made it. Null is ignored; from another
/// thread the view is leaked, because dropping Slint objects there is
/// undefined behaviour.
///
/// # Safety
///
/// `view` is null or a live handle not used again after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_view_free(view: *mut KerfView) {
    // SAFETY: forwarded from the caller.
    if unsafe { self::view(view) }.is_ok() {
        // SAFETY: the handle came from `Box::into_raw` and is not used again.
        let view = unsafe { Box::from_raw(view) };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(view)));
    }
}

/// New size in physical pixels and scale factor (2 on Retina). A zero
/// dimension becomes one pixel.
///
/// # Safety
///
/// `view` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_view_resize(
    view: *mut KerfView,
    width: u32,
    height: u32,
    scale: f32,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        Ok(unsafe { self::view(view) }?
            .view
            .resize(width, height, scale)?)
    })
}

/// Replace the look, e.g. when the system switches Light and Dark.
///
/// # Safety
///
/// `view` is a live handle; `theme` is null or a readable `KerfTheme`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_view_set_theme(
    view: *mut KerfView,
    theme: *const KerfTheme,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (view, theme) = unsafe { (self::view(view)?, self::theme(theme)?) };
        view.view.set_theme(&theme);
        Ok(())
    })
}

/// Draw `ed` into the view's frame. `KERF_STATUS_NOTHING` when Slint
/// repainted nothing, so the host can keep showing the last frame.
///
/// # Safety
///
/// `view` and `ed` are live handles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_view_render(view: *mut KerfView, ed: *mut KerfEditor) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (view, editor) = unsafe { (self::view(view)?, editor(ed)?) };
        view.view.render(&editor.frame())?;
        // A resize leaves the old frame; Slint repaints all of a new size.
        view.frame.resize(view.view.frame_len(), 0);
        if view.view.draw(&mut view.frame)? {
            Ok(())
        } else {
            Err(KerfStatus::Nothing)
        }
    })
}

/// The caret rectangle of the last render, for the host's system caret.
/// `KERF_STATUS_NOTHING` when the caret is outside the view.
///
/// # Safety
///
/// `view` is a live handle; `out` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_view_caret(view: *mut KerfView, out: *mut KerfRect) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let r = unsafe { self::view(view) }?
            .view
            .caret()
            .ok_or(KerfStatus::Nothing)?;
        let rect = KerfRect {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        };
        // SAFETY: forwarded from the caller.
        unsafe { put(out, rect) }
    })
}

/// Copy the last rendered frame into `buf`: premultiplied RGBA8 rows, top
/// row first, no padding, `width * height * 4` bytes, written to `*out_len`.
/// As with `kerf_editor_copy_text`, a short `buf` gets
/// `KERF_STATUS_TOO_LARGE`. Before the first render the frame is empty.
///
/// # Safety
///
/// `view` is a live handle; `buf` is null or valid for `cap` bytes of
/// writes; `out_len` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_view_read_rgba(
    view: *mut KerfView,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let rgba = &unsafe { self::view(view) }?.frame;
        // SAFETY: forwarded from the caller.
        unsafe { put(out_len, rgba.len()) }?;
        if rgba.len() > cap {
            return Err(KerfStatus::TooLarge);
        }
        if buf.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        // SAFETY: `buf` is writable for `cap >= rgba.len()` bytes, and the
        // host cannot hold a pointer into the view's own frame.
        unsafe { std::ptr::copy_nonoverlapping(rgba.as_ptr(), buf, rgba.len()) };
        Ok(())
    })
}
