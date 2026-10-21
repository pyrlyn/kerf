// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The Linux host on a real GTK display (Xvfb in CI): GNOME's look as a
//! styled widget sees it. Without a display it skips, unless
//! `KERF_REQUIRE_DISPLAY` is set, which CI does.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[cfg(not(target_os = "linux"))]
fn main() {}

#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = gtk::init() {
        assert!(
            std::env::var_os("KERF_REQUIRE_DISPLAY").is_none(),
            "KERF_REQUIRE_DISPLAY is set but GTK has no display: {error}"
        );
        eprintln!("skipped: no display ({error})");
        return;
    }
    let cases: [(&str, fn()); 1] = [("gnome_look", linux::gnome_look)];
    for (name, case) in cases {
        case();
        println!("test {name} ... ok");
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::time::{Duration, Instant};

    use gtk::{glib, prelude::*};
    use kerf_gtk::gnome::{self, Gnome};

    /// Runs GTK's loop until `done` or a timeout.
    fn wait(done: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let context = glib::MainContext::default();
        while !done() {
            assert!(Instant::now() < deadline, "GTK did not get there in time");
            if !context.iteration(false) {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

    /// `child` in a shown window, allocated.
    fn window(child: &impl IsA<gtk::Widget>) -> gtk::Window {
        let window = gtk::Window::builder()
            .default_width(480)
            .default_height(240)
            .child(child)
            .build();
        window.present();
        wait(|| child.width() > 1);
        window
    }

    pub fn gnome_look() {
        let label = gtk::Label::new(Some("Kerf"));
        let window = window(&label);
        // GTK's own theme, which the test display runs, is light: dark text.
        assert!(!gnome::is_dark(&label));
        let gnome = Gnome::default();
        assert!(gnome.font().size() > 0);
        let look = gnome.look(&label, false);
        assert!(look.font_size > 10.0, "{look:?}");
        assert!(look.line_height >= look.font_size, "{look:?}");
        assert_eq!(look.background, [255, 255, 255, 255]);
        assert_ne!(gnome.look(&label, true).background, look.background);
        let theme = gnome::kerf_theme(&look);
        assert_eq!(theme.font_family_len, look.font_family.len());
        assert_eq!(theme.draw_caret, 0);
        window.destroy();
    }
}
