// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Drawing on the GPU: clear to the theme background, draw the shaped text
//! through `glyphon`'s atlas, and the caret only when the host asks.

use glyphon::{
    Cache, Color, ContentType, CustomGlyph, RasterizeCustomGlyphRequest, RasterizedCustomGlyph,
    Resolution, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use kerf_core::{Frame, Render};

use crate::{Layout, Rect, Rgba, Size, Theme};

/// Why a frame was not drawn.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The glyph atlas could not take the frame's glyphs.
    #[error("glyph atlas: {0}")]
    Prepare(#[from] glyphon::PrepareError),
    /// A glyph left the atlas between preparing and drawing.
    #[error("text draw: {0}")]
    Draw(#[from] glyphon::RenderError),
    /// Copying the frame back from the GPU failed.
    #[error("pixel readback: {0}")]
    Readback(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Draws frames into any texture of the format it was made for.
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    srgb: bool,
    viewport: Viewport,
    atlas: TextAtlas,
    text: TextRenderer,
    swash: SwashCache,
    layout: Layout,
    theme: Theme,
    caret: Option<Rect>,
}

impl GpuRenderer {
    /// A renderer for targets of `format` (the host's surface format).
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        layout: Layout,
        theme: Theme,
    ) -> Self {
        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let text = TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
        Self {
            device: device.clone(),
            queue: queue.clone(),
            srgb: format.is_srgb(),
            viewport,
            atlas,
            text,
            swash: SwashCache::new(),
            layout,
            theme,
            caret: None,
        }
    }

    /// Replace the look, e.g. when the system switches Light and Dark.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    /// Caret rectangle of the last drawn frame, for a host that draws the
    /// system caret; `None` when it is outside the view.
    #[must_use]
    pub fn caret(&self) -> Option<Rect> {
        self.caret
    }

    /// Draw `frame` into `target`, a view of `size`.
    pub fn draw(
        &mut self,
        frame: &Frame<'_>,
        size: Size,
        target: &wgpu::TextureView,
    ) -> Result<Option<Rect>, Error> {
        self.caret = self.layout.update(frame, &self.theme, size);
        self.viewport.update(
            &self.queue,
            Resolution {
                width: size.width,
                height: size.height,
            },
        );
        // The caret is a solid glyph, so it goes through the same pipeline
        // as the text instead of a second one of our own.
        let caret: Vec<CustomGlyph> = self
            .caret
            .filter(|_| self.theme.draw_caret)
            .map(|r| CustomGlyph {
                id: 0,
                left: r.x,
                top: r.y,
                width: r.width,
                height: r.height,
                color: Some(color(self.theme.caret)),
                snap_to_physical_pixel: true,
                metadata: 0,
            })
            .into_iter()
            .collect();
        let area = TextArea {
            buffer: &self.layout.text,
            left: 0.0,
            top: 0.0,
            scale: 1.0,
            bounds: TextBounds {
                left: 0,
                top: 0,
                right: i32::try_from(size.width).unwrap_or(i32::MAX),
                bottom: i32::try_from(size.height).unwrap_or(i32::MAX),
            },
            default_color: color(self.theme.text),
            custom_glyphs: &caret,
        };
        self.text.prepare_with_custom(
            &self.device,
            &self.queue,
            &mut self.layout.fonts,
            &mut self.atlas,
            &self.viewport,
            [area],
            &mut self.swash,
            solid,
        )?;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kerf editor"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear(self.theme.background, self.srgb)),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.text.render(&self.atlas, &self.viewport, &mut pass)?;
        }
        self.queue.submit(Some(encoder.finish()));
        self.atlas.trim();
        Ok(self.caret)
    }
}

/// Format of [`Offscreen`]'s texture: sRGB, so pixels read back in the same
/// numbers as the theme's colours.
pub const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// A renderer with its own texture: for tests, thumbnails, and hosts that
/// composite the editor themselves.
pub struct Offscreen {
    renderer: GpuRenderer,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: Size,
}

impl Offscreen {
    /// A texture of `size`, clamped to what the device can hold.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: Layout,
        theme: Theme,
        size: Size,
    ) -> Self {
        let renderer = GpuRenderer::new(device, queue, OFFSCREEN_FORMAT, layout, theme);
        let (texture, view, size) = target(device, size);
        Self {
            renderer,
            texture,
            view,
            size,
        }
    }

    /// A new texture of `size`; the renderer and its loaded fonts stay.
    pub fn resize(&mut self, size: Size) {
        (self.texture, self.view, self.size) = target(&self.renderer.device, size);
    }

    /// The renderer, for the caret rectangle and theme changes.
    pub fn renderer(&mut self) -> &mut GpuRenderer {
        &mut self.renderer
    }

    /// The last frame as tightly packed sRGB RGBA rows, top row first.
    pub fn read_rgba(&self) -> Result<Vec<u8>, Error> {
        let (device, queue) = (&self.renderer.device, &self.renderer.queue);
        let (width, height) = (self.size.width, self.size.height);
        let packed = width as usize * 4;
        // Copies need rows padded to the alignment; the padding is cut below.
        let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kerf readback"),
            size: u64::from(row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            self.texture.size(),
        );
        queue.submit(Some(encoder.finish()));
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |mapped| {
            let _ = tx.send(mapped);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(readback)?;
        rx.recv().map_err(readback)?.map_err(readback)?;
        let data = buffer.get_mapped_range(..).map_err(readback)?;
        Ok(data
            .chunks(row as usize)
            .flat_map(|line| &line[..packed])
            .copied()
            .collect())
    }
}

fn target(device: &wgpu::Device, size: Size) -> (wgpu::Texture, wgpu::TextureView, Size) {
    let max = device.limits().max_texture_dimension_2d;
    let size = Size {
        width: size.width.clamp(1, max),
        height: size.height.clamp(1, max),
        ..size
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kerf offscreen"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OFFSCREEN_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view, size)
}

fn readback(e: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::Readback(Box::new(e))
}

impl Render for Offscreen {
    type Error = Error;

    fn render(&mut self, frame: &Frame<'_>) -> Result<(), Error> {
        self.renderer.draw(frame, self.size, &self.view).map(drop)
    }
}

fn color([r, g, b, a]: Rgba) -> Color {
    Color::rgba(r, g, b, a)
}

/// Clear values are linear; an sRGB target encodes them on store, so the
/// theme's sRGB numbers are decoded first or the background comes out pale.
fn clear([r, g, b, a]: Rgba, srgb: bool) -> wgpu::Color {
    let channel = |c: u8| {
        let c = f64::from(c) / 255.0;
        if !srgb {
            c
        } else if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    wgpu::Color {
        r: channel(r),
        g: channel(g),
        b: channel(b),
        a: f64::from(a) / 255.0,
    }
}

/// Every custom glyph is the caret: a fully covered mask the atlas tints.
fn solid(req: RasterizeCustomGlyphRequest) -> Option<RasterizedCustomGlyph> {
    Some(RasterizedCustomGlyph {
        data: vec![u8::MAX; usize::from(req.width) * usize::from(req.height)],
        content_type: ContentType::Mask,
    })
}
