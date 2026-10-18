// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! A frame drawn on whatever adapter the machine has, read back pixel by
//! pixel. CI runners have software ones: lavapipe on Linux, WARP on Windows.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kerf_core::{Editor, Render, TextBuffer};
use kerf_render::{Layout, Offscreen, Size, Theme};

/// Set in CI, where a missing adapter is a broken runner, not a skip.
const REQUIRE: &str = "KERF_REQUIRE_GPU";

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let request = |force_fallback_adapter| {
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            force_fallback_adapter,
            ..Default::default()
        }))
    };
    let Ok(adapter) = request(false).or_else(|_| request(true)) else {
        assert!(
            std::env::var_os(REQUIRE).is_none(),
            "{REQUIRE} is set but there is no adapter"
        );
        eprintln!("no GPU adapter, offscreen test skipped");
        return None;
    };
    eprintln!("adapter: {:?}", adapter.get_info());
    Some(pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap())
}

/// The view's last frame as rows of RGBA pixels.
fn pixels(view: &Offscreen, width: usize) -> Vec<Vec<[u8; 4]>> {
    let rgba = view.read_rgba().unwrap();
    rgba.chunks(width * 4)
        .map(|line| line.chunks(4).map(|p| [p[0], p[1], p[2], p[3]]).collect())
        .collect()
}

fn near(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 2)
}

#[test]
fn frame_draws_background_text_and_the_caret_on_request() {
    let Some((device, queue)) = device() else {
        return;
    };
    let theme = Theme {
        font_size: 20.0,
        line_height: 24.0,
        background: [30, 30, 30, 255],
        text: [250, 250, 250, 255],
        caret: [255, 0, 0, 255],
        caret_width: 2.0,
        draw_caret: true,
        ..Theme::default()
    };
    let size = Size {
        width: 200,
        height: 60,
        scale: 1.0,
    };
    let mut view = Offscreen::new(&device, &queue, Layout::new(), theme.clone(), size);
    let mut ed = Editor::new(TextBuffer::new());
    ed.insert("Wide\nkerf").unwrap();
    view.render(&ed.frame()).unwrap();
    let caret = view.renderer().caret().unwrap();
    let px = pixels(&view, 200);

    assert!(near(px[59][199], theme.background), "{:?}", px[59][199]);
    let first_line = px[..24].iter().flat_map(|r| &r[..100]);
    assert!(
        first_line.clone().any(|p| p[0] > 150),
        "no text pixels on the first line"
    );
    // Snapping moves a 2 px caret by at most half a pixel; this column is
    // inside it either way.
    let (cx, cy) = (
        (caret.x + 1.0) as usize,
        (caret.y + caret.height / 2.0) as usize,
    );
    assert_eq!(caret.y, 24.0);
    assert!(
        near(px[cy][cx], theme.caret),
        "caret {caret:?}: {:?}",
        px[cy][cx]
    );

    view.renderer().set_theme(Theme {
        draw_caret: false,
        ..theme.clone()
    });
    view.render(&ed.frame()).unwrap();
    let px = pixels(&view, 200);
    assert!(near(px[cy][cx], theme.background), "{:?}", px[cy][cx]);

    view.resize(Size {
        width: 100,
        height: 30,
        scale: 1.0,
    });
    view.render(&ed.frame()).unwrap();
    assert_eq!(view.read_rgba().unwrap().len(), 100 * 30 * 4);
}
