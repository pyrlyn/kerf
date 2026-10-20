// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The Linux host's path through a real GTK display (Xvfb in CI): open a
//! file into a window, type through the input method and keys, select, undo,
//! redo, and read the frame and the caret back. Without a display it skips,
//! unless `KERF_REQUIRE_DISPLAY` is set, which CI does.

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
    let cases: [(&str, fn()); 6] = [
        ("gnome_look", linux::gnome_look),
        ("open_type_undo_redo", linux::open_type_undo_redo),
        ("keys_select_and_edit", linux::keys_select_and_edit),
        (
            "input_method_commits_text",
            linux::input_method_commits_text,
        ),
        ("frame_and_caret", linux::frame_and_caret),
        ("window_actions", linux::window_actions),
    ];
    for (name, case) in cases {
        case();
        println!("test {name} ... ok");
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::io::Write;
    use std::time::{Duration, Instant};

    use gtk::{gdk, gio, glib, prelude::*};
    use kerf_gtk::gnome::{self, Gnome};
    use kerf_gtk::{Editor, EditorView, app};

    fn file(text: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::Builder::new().suffix(".rs").tempfile().unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file
    }

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

    /// `view` in a shown window, drawn once.
    fn shown(view: &EditorView) -> gtk::Window {
        let window = window(view);
        wait(|| view.frame().is_some_and(|f| f.width() > 1));
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

    fn text(view: &EditorView) -> String {
        view.editor().unwrap().text().unwrap()
    }

    pub fn open_type_undo_redo() {
        let file = file("fn main() {}\n");
        let view = EditorView::new(Editor::open(file.path()).unwrap()).unwrap();
        let window = shown(&view);
        assert_eq!(text(&view), "fn main() {}\n");
        view.insert("// ");
        assert_eq!(text(&view), "// fn main() {}\n");
        view.undo();
        assert_eq!(text(&view), "fn main() {}\n");
        view.redo();
        assert_eq!(text(&view), "// fn main() {}\n");
        assert_eq!(view.render_error(), None);
        window.destroy();
    }

    pub fn keys_select_and_edit() {
        let view = EditorView::new(Editor::new().unwrap()).unwrap();
        let window = shown(&view);
        view.insert("abc");
        assert!(view.key(gdk::Key::Left, gdk::ModifierType::empty()));
        assert!(view.key(gdk::Key::Left, gdk::ModifierType::SHIFT_MASK));
        assert_eq!(view.editor().unwrap().selection().unwrap(), (2, 1));
        // GTK's text views show no caret over a selection.
        assert_eq!(view.caret(), None);
        assert!(view.key(gdk::Key::BackSpace, gdk::ModifierType::empty()));
        assert_eq!(text(&view), "ac");
        assert!(view.key(gdk::Key::Return, gdk::ModifierType::empty()));
        assert!(view.key(gdk::Key::Tab, gdk::ModifierType::empty()));
        assert_eq!(text(&view), "a\n\tc");
        // Control shortcuts belong to the window's actions.
        assert!(!view.key(gdk::Key::z, gdk::ModifierType::CONTROL_MASK));
        view.select_all();
        assert_eq!(view.editor().unwrap().selection().unwrap(), (0, 4));
        window.destroy();
    }

    pub fn input_method_commits_text() {
        let view = EditorView::new(Editor::new().unwrap()).unwrap();
        let window = shown(&view);
        // The signal an input method emits when composition ends, here from
        // the context the widget's key controller feeds.
        view.im_context().emit_by_name::<()>("commit", &[&"é日本"]);
        assert_eq!(text(&view), "é日本");
        assert_eq!(view.editor().unwrap().selection().unwrap(), (8, 8));
        window.destroy();
    }

    pub fn frame_and_caret() {
        let view = EditorView::new(Editor::new().unwrap()).unwrap();
        let window = shown(&view);
        let look = view.look().expect("styled");
        let frame = view.frame().unwrap();
        let scale = view.scale_factor();
        assert_eq!(
            (frame.width(), frame.height()),
            (view.width() * scale, view.height() * scale)
        );
        // A corner pixel is the theme's background; `download` writes Cairo's
        // ARGB32, premultiplied BGRA bytes on little-endian machines.
        let stride = frame.width() as usize * 4;
        let mut pixels = vec![0; stride * frame.height() as usize];
        frame.download(&mut pixels, stride);
        let [r, g, b, a] = look.background;
        let corner = stride * (frame.height() as usize - 1) + stride - 4;
        assert_eq!(pixels[corner..corner + 4], [b, g, r, a]);

        let start = view.caret().expect("caret at the start");
        view.insert("abc");
        let after = view.caret().expect("caret after typing");
        assert!(after.x() > start.x(), "{after:?} not right of {start:?}");
        assert!(after.height() > 0.0);
        view.insert("\n");
        assert!(view.caret().unwrap().y() > after.y());
        window.destroy();
    }

    pub fn window_actions() {
        let app = gtk::Application::builder()
            .application_id("io.github.pyrlyn.Kerf.Test")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(gio::Cancellable::NONE).unwrap();
        app::install(&app);
        let file = file("let x = 1;\n");
        let window = app::window(&app, Editor::open(file.path()).unwrap(), "x.rs").unwrap();
        window.present();
        let view = app::view_of(&window).unwrap();
        wait(|| view.frame().is_some_and(|f| f.width() > 1));
        assert_eq!(window.title().as_deref(), Some("x.rs"));
        assert_eq!(app.accels_for_action("win.undo"), ["<Control>z"]);
        view.insert("// ");
        WidgetExt::activate_action(&window, "win.undo", None).unwrap();
        assert_eq!(text(&view), "let x = 1;\n");
        WidgetExt::activate_action(&window, "win.redo", None).unwrap();
        assert_eq!(text(&view), "// let x = 1;\n");
        WidgetExt::activate_action(&window, "win.select-all", None).unwrap();
        assert_eq!(view.editor().unwrap().selection().unwrap(), (0, 14));
        window.destroy();
    }
}
