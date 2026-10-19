// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The editor view: `kerf-render` drawing into an offscreen texture or into
//! a surface over the host's native view.

use kerf_core::Render;
use kerf_render::{GpuRenderer, Layout, Offscreen, Size, Theme};

use crate::{KerfEditor, KerfStatus, editor, guard, put, text};

/// Longest font family name the host can pass.
const MAX_FAMILY_BYTES: usize = 256;

/// The editor's look, in logical pixels (`kerf_render::Theme`). Colours are
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
    /// Caret, when Kerf draws it.
    pub caret: [u8; 4],
    /// Caret width.
    pub caret_width: f32,
    /// Non-zero: Kerf draws the caret. Zero: the host draws the system caret
    /// at `kerf_view_caret`. A byte, not `bool`, so any value is defined.
    pub draw_caret: u8,
}

/// A rectangle in physical pixels from the view's top-left corner.
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
    target: Target,
}

enum Target {
    Offscreen(Box<Offscreen>),
    #[cfg(target_os = "macos")]
    Surface(Box<Surface>),
}

#[cfg(target_os = "macos")]
struct Surface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: GpuRenderer,
    scale: f32,
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
        caret: t.caret,
        caret_width: t.caret_width,
        draw_caret: t.draw_caret != 0,
    })
}

/// The view behind a host handle.
///
/// # Safety
///
/// `view` is null or a live handle no other call is using.
unsafe fn view<'a>(view: *mut KerfView) -> Result<&'a mut Target, KerfStatus> {
    // SAFETY: the caller promised a live, unaliased handle or null.
    unsafe { view.as_mut() }
        .map(|v| &mut v.target)
        .ok_or(KerfStatus::NullPointer)
}

fn device(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), KerfStatus> {
    let request = |force_fallback_adapter| {
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            force_fallback_adapter,
            compatible_surface: surface,
            ..Default::default()
        }))
    };
    // A software adapter (WARP, lavapipe) beats no editor at all.
    let adapter = request(false)
        .or_else(|_| request(true))
        .map_err(|_| KerfStatus::Gpu)?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|_| KerfStatus::Gpu)?;
    Ok((adapter, device, queue))
}

fn boxed(target: Target) -> *mut KerfView {
    Box::into_raw(Box::new(KerfView { target }))
}

/// A view that draws into its own texture, for tests, thumbnails and hosts
/// that composite the editor themselves. `theme` may be null.
///
/// # Safety
///
/// `theme` is null or a readable `KerfTheme`; `out` is valid for a write of
/// one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_view_new_offscreen(
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
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let (_, device, queue) = device(&instance, None)?;
        let size = Size {
            width,
            height,
            scale,
        };
        let view = Offscreen::new(&device, &queue, Layout::new(), theme, size);
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out, boxed(Target::Offscreen(Box::new(view)))) }
    })
}

/// A view that draws into the host's `NSView` (it gets a Metal layer).
/// `width` and `height` are the view's size in physical pixels; `theme` may
/// be null.
///
/// # Safety
///
/// `ns_view` is a live `NSView` that outlives the returned view, and this is
/// called on the main thread; `theme` is null or a readable `KerfTheme`;
/// `out` is valid for a write of one pointer.
#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_view_attach_appkit(
    ns_view: *mut std::ffi::c_void,
    width: u32,
    height: u32,
    scale: f32,
    theme: *const KerfTheme,
    out: *mut *mut KerfView,
) -> KerfStatus {
    use wgpu::rwh::{AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle};
    guard(|| {
        let ns_view = std::ptr::NonNull::new(ns_view).ok_or(KerfStatus::NullPointer)?;
        // SAFETY: forwarded from the caller.
        let theme = unsafe { self::theme(theme) }?;
        if out.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::AppKit(AppKitDisplayHandle::new())),
            raw_window_handle: RawWindowHandle::AppKit(AppKitWindowHandle::new(ns_view)),
        };
        // SAFETY: the host promised a live NSView that outlives the surface.
        let surface =
            unsafe { instance.create_surface_unsafe(target) }.map_err(|_| KerfStatus::Gpu)?;
        let (adapter, device, queue) = device(&instance, Some(&surface))?;
        let config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .ok_or(KerfStatus::Gpu)?;
        surface.configure(&device, &config);
        let renderer = GpuRenderer::new(&device, &queue, config.format, Layout::new(), theme);
        let view = Surface {
            surface,
            config,
            device,
            queue,
            renderer,
            scale,
        };
        // SAFETY: forwarded from the caller; checked non-null above.
        unsafe { put(out, boxed(Target::Surface(Box::new(view)))) }
    })
}

/// Free a view. Null is ignored.
///
/// # Safety
///
/// `view` is null or a live handle not used again after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_view_free(view: *mut KerfView) {
    if !view.is_null() {
        // SAFETY: the handle came from `Box::into_raw` and is not used again.
        let view = unsafe { Box::from_raw(view) };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(view)));
    }
}

/// New size in physical pixels and scale factor (2 on Retina).
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
        match unsafe { self::view(view) }? {
            Target::Offscreen(o) => o.resize(Size {
                width,
                height,
                scale,
            }),
            #[cfg(target_os = "macos")]
            Target::Surface(s) => {
                let max = s.device.limits().max_texture_dimension_2d;
                s.config.width = width.clamp(1, max);
                s.config.height = height.clamp(1, max);
                s.scale = scale;
                s.surface.configure(&s.device, &s.config);
            }
        }
        Ok(())
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
        let (target, theme) = unsafe { (self::view(view)?, self::theme(theme)?) };
        renderer(target).set_theme(theme);
        Ok(())
    })
}

/// Draw `ed` into the view.
///
/// # Safety
///
/// `view` and `ed` are live handles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kerf_view_render(view: *mut KerfView, ed: *mut KerfEditor) -> KerfStatus {
    guard(|| {
        // SAFETY: forwarded from the caller.
        let (target, editor) = unsafe { (self::view(view)?, editor(ed)?) };
        let frame = editor.frame();
        match target {
            Target::Offscreen(o) => o.render(&frame).map_err(|_| KerfStatus::Gpu),
            #[cfg(target_os = "macos")]
            Target::Surface(s) => s.render(&frame),
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
        let r = renderer(unsafe { self::view(view) }?)
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

/// Copy an offscreen view's last frame into `buf` as sRGB RGBA rows, top
/// row first, `width * height * 4` bytes, written to `*out_len`. As with
/// `kerf_editor_copy_text`, a short `buf` gets `KERF_STATUS_TOO_LARGE`.
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
        // Without AppKit, offscreen is the only kind; the match stays so the
        // macOS arm is not a separate code path.
        #[cfg_attr(
            not(target_os = "macos"),
            allow(clippy::infallible_destructuring_match)
        )]
        // SAFETY: forwarded from the caller.
        let o = match unsafe { self::view(view) }? {
            Target::Offscreen(o) => o,
            #[cfg(target_os = "macos")]
            Target::Surface(_) => return Err(KerfStatus::Unsupported),
        };
        let rgba = o.read_rgba().map_err(|_| KerfStatus::Gpu)?;
        // SAFETY: forwarded from the caller.
        unsafe { put(out_len, rgba.len()) }?;
        if rgba.len() > cap {
            return Err(KerfStatus::TooLarge);
        }
        if buf.is_null() {
            return Err(KerfStatus::NullPointer);
        }
        // SAFETY: `buf` is writable for `cap >= rgba.len()` bytes and cannot
        // overlap a `Vec` Kerf just allocated.
        unsafe { std::ptr::copy_nonoverlapping(rgba.as_ptr(), buf, rgba.len()) };
        Ok(())
    })
}

fn renderer(target: &mut Target) -> &mut GpuRenderer {
    match target {
        Target::Offscreen(o) => o.renderer(),
        #[cfg(target_os = "macos")]
        Target::Surface(s) => &mut s.renderer,
    }
}

#[cfg(target_os = "macos")]
impl Surface {
    fn render(&mut self, frame: &kerf_core::Frame<'_>) -> Result<(), KerfStatus> {
        let size = Size {
            width: self.config.width,
            height: self.config.height,
            scale: self.scale,
        };
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            // Hidden or busy: no pixels this time, but the caret the host
            // draws must still follow the edit. The host redraws later.
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                self.renderer.lay_out(frame, size);
                return Err(KerfStatus::Nothing);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                self.renderer.lay_out(frame, size);
                return Err(KerfStatus::Nothing);
            }
            wgpu::CurrentSurfaceTexture::Validation => return Err(KerfStatus::Gpu),
        };
        let target = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.renderer
            .draw(frame, size, &target)
            .map_err(|_| KerfStatus::Gpu)?;
        self.queue.present(texture);
        Ok(())
    }
}
