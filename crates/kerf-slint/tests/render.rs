// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The editor component drawn on the CPU, read back pixel by pixel. No GPU,
//! window or event loop: the same path a native host takes.

#![allow(clippy::unwrap_used)]

use kerf_core::{Editor, Render, TextBuffer};
use kerf_slint::{BYTES_PER_PIXEL, Error, Rect, SlintView, Theme};

const WIDTH: u32 = 400;
const HEIGHT: u32 = 120;

/// Colours far apart from each other, so a pixel says which one it is.
fn theme() -> Theme {
    Theme {
        background: [10, 20, 30, 255],
        text: [250, 250, 250, 255],
        selection: [200, 40, 40, 255],
        caret: [0, 220, 0, 255],
        ..Theme::default()
    }
}

fn line_height() -> f32 {
    theme().line_height
}

fn view() -> SlintView {
    SlintView::new(WIDTH, HEIGHT, 1.0, &theme()).unwrap()
}

/// Shows `ed`'s frame and returns the painted pixels.
fn draw(view: &mut SlintView, ed: &Editor) -> Vec<u8> {
    view.render(&ed.frame()).unwrap();
    let mut buf = vec![0; view.frame_len()];
    assert!(view.draw(&mut buf).unwrap(), "the frame was not painted");
    buf
}

fn pixel(buf: &[u8], x: f32, y: f32) -> [u8; 4] {
    let at = (y as usize * WIDTH as usize + x as usize) * BYTES_PER_PIXEL;
    buf[at..at + BYTES_PER_PIXEL].try_into().unwrap()
}

/// Pixels in the band of line `line` that look like the text colour.
fn text_pixels(buf: &[u8], line: usize) -> usize {
    let top = line as f32 * line_height();
    (top as u32..(top + line_height()) as u32)
        .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(buf, x as f32, y as f32)[..3].iter().all(|&c| c > 200))
        .count()
}

fn caret(view: &mut SlintView, ed: &Editor) -> Rect {
    view.render(&ed.frame()).unwrap();
    view.caret().unwrap()
}

#[test]
fn background_text_and_selection_land_on_their_pixels() {
    let mut view = view();
    let mut ed = Editor::new(TextBuffer::with_text("hello\nworld"));
    // "lo", the line break, then "w".
    ed.select(3, 7).unwrap();
    let buf = draw(&mut view, &ed);
    let lh = line_height();
    let right = WIDTH as f32 - 2.0;

    // The selected break fills the rest of the first row.
    assert_eq!(pixel(&buf, right, lh * 0.5), theme().selection);
    // The second row's selection stops after "w"; past it is background.
    assert_eq!(pixel(&buf, right, lh * 1.5), theme().background);
    // Below the last line there is only background.
    assert_eq!(pixel(&buf, right, lh * 3.5), theme().background);
    // Both lines are drawn in the text colour, nothing below them.
    assert!(text_pixels(&buf, 0) > 0, "no text on the first line");
    assert!(text_pixels(&buf, 1) > 0, "no text on the second line");
    assert_eq!(text_pixels(&buf, 3), 0);
    // The first row's fill starts after "hel"; the second row's starts at
    // the left edge, under "w" (an empty prefix measures zero).
    assert_eq!(pixel(&buf, 0.0, 1.0), theme().background);
    assert_eq!(pixel(&buf, 0.0, lh + 1.0), theme().selection);
}

#[test]
fn nothing_selected_draws_no_fill() {
    let mut view = view();
    let ed = Editor::new(TextBuffer::with_text("hello\nworld"));
    let buf = draw(&mut view, &ed);
    let fills = buf
        .as_chunks::<BYTES_PER_PIXEL>()
        .0
        .iter()
        .filter(|px| **px == theme().selection)
        .count();
    assert_eq!(fills, 0);
}

#[test]
fn caret_follows_moves_and_edits() {
    let mut view = view();
    let mut ed = Editor::new(TextBuffer::new());
    let start = caret(&mut view, &ed);
    assert_eq!(
        start,
        Rect {
            x: 0.0,
            y: 0.0,
            width: theme().caret_width,
            height: line_height(),
        }
    );

    ed.insert("ab").unwrap();
    let two = caret(&mut view, &ed);
    assert!(two.x > 0.0 && two.y == 0.0, "{two:?}");

    ed.move_left();
    let one = caret(&mut view, &ed);
    assert!(one.x > 0.0 && one.x < two.x, "{one:?} {two:?}");

    // A trailing space still moves the caret: the measured prefix keeps it.
    ed.move_right();
    ed.insert(" ").unwrap();
    assert!(caret(&mut view, &ed).x > two.x);

    ed.insert("\n").unwrap();
    let below = caret(&mut view, &ed);
    assert_eq!((below.x, below.y), (0.0, line_height()));

    ed.backspace();
    assert_eq!(caret(&mut view, &ed).y, 0.0);
}

#[test]
fn caret_is_in_logical_points_at_any_scale() {
    let mut ed = Editor::new(TextBuffer::new());
    ed.insert("a\nbc").unwrap();
    let mut one = view();
    let mut two = SlintView::new(WIDTH * 2, HEIGHT * 2, 2.0, &theme()).unwrap();
    let (a, b) = (caret(&mut one, &ed), caret(&mut two, &ed));
    assert_eq!((a.y, a.height), (b.y, b.height));
    assert!((a.x - b.x).abs() < 1.0, "{a:?} {b:?}");
}

#[test]
fn caret_below_the_view_is_not_reported() {
    let mut view = SlintView::new(WIDTH, 40, 1.0, &theme()).unwrap();
    let mut ed = Editor::new(TextBuffer::new());
    ed.insert(&"\n".repeat(100)).unwrap();
    view.render(&ed.frame()).unwrap();
    assert_eq!(view.caret(), None);
}

#[test]
fn caret_is_drawn_only_when_the_theme_asks() {
    let ed = Editor::new(TextBuffer::with_text("x"));
    let y = line_height() * 0.5;

    let mut system = view();
    assert_eq!(pixel(&draw(&mut system, &ed), 0.0, y), theme().background);

    let drawn = Theme {
        draw_caret: true,
        caret_width: 2.0,
        ..theme()
    };
    let mut own = SlintView::new(WIDTH, HEIGHT, 1.0, &drawn).unwrap();
    assert_eq!(pixel(&draw(&mut own, &ed), 0.0, y), theme().caret);
}

#[test]
fn resize_changes_the_frame_length() {
    let mut view = view();
    let ed = Editor::new(TextBuffer::with_text("x"));
    let old = draw(&mut view, &ed);
    assert_eq!(old.len(), (WIDTH * HEIGHT) as usize * BYTES_PER_PIXEL);

    view.resize(WIDTH * 2, HEIGHT * 2, 2.0).unwrap();
    assert_eq!(view.size(), (WIDTH * 2, HEIGHT * 2));
    assert_eq!(view.frame_len(), old.len() * 4);
    let mut short = old;
    assert!(matches!(view.draw(&mut short), Err(Error::Slint(_))));
    let mut buf = vec![0; view.frame_len()];
    assert!(view.draw(&mut buf).unwrap());
}

#[test]
fn broken_host_numbers_do_not_panic() {
    let theme = Theme {
        font_size: f32::NAN,
        line_height: 0.0,
        caret_width: -3.0,
        ..theme()
    };
    let mut view = SlintView::new(0, 0, 1.0, &theme).unwrap();
    let ed = Editor::new(TextBuffer::with_text("x\ny"));
    view.render(&ed.frame()).unwrap();
    let mut buf = vec![0; view.frame_len()];
    view.draw(&mut buf).unwrap();
    assert!(view.resize(10, 10, f32::INFINITY).is_err());
}
