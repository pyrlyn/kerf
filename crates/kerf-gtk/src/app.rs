// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The application: one window per file, GNOME's header bar with an Open
//! button and a main menu instead of a menu bar, and GNOME's shortcuts.

use gtk::{gio, glib, prelude::*};

use crate::editor::{Editor, Error};
use crate::view::EditorView;

/// The application id GTK and the desktop know Kerf by.
pub const APP_ID: &str = "io.github.pyrlyn.Kerf";

/// Runs the app; files on the command line open in their own windows.
pub fn run() -> glib::ExitCode {
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    app.connect_startup(install);
    app.connect_activate(|app| show(app, Editor::new(), "Untitled"));
    app.connect_open(|app, files, _| {
        for file in files {
            open(app, file);
        }
    });
    app.run()
}

/// GNOME's shortcuts for the window actions (GNOME HIG, "Keyboard").
pub fn install(app: &gtk::Application) {
    let quit = gio::ActionEntry::builder("quit")
        .activate(|app: &gtk::Application, _, _| app.quit())
        .build();
    app.add_action_entries([quit]);
    for (action, accels) in [
        ("win.open", &["<Control>o"][..]),
        ("win.undo", &["<Control>z"]),
        ("win.redo", &["<Shift><Control>z"]),
        ("win.select-all", &["<Control>a"]),
        ("window.close", &["<Control>w"]),
        ("app.quit", &["<Control>q"]),
    ] {
        app.set_accels_for_action(action, accels);
    }
}

/// Opens `file` in a new window, or says why it cannot.
pub fn open(app: &gtk::Application, file: &gio::File) {
    let name = file
        .basename()
        .map(|n| n.display().to_string())
        .unwrap_or_default();
    let editor = file
        .path()
        .ok_or(Error(kerf_ffi::KerfStatus::Io))
        .and_then(|p| Editor::open(&p));
    show(app, editor, &name);
}

fn show(app: &gtk::Application, editor: Result<Editor, Error>, title: &str) {
    match editor.and_then(|editor| window(app, editor, title)) {
        Ok(window) => window.present(),
        Err(error) => {
            let alert = gtk::AlertDialog::builder()
                .message(format!("Kerf could not open “{title}”"))
                .detail(error.to_string())
                .build();
            alert.show(app.active_window().as_ref());
        }
    }
}

/// A window showing `editor`, titled `title`.
pub fn window(
    app: &gtk::Application,
    editor: Editor,
    title: &str,
) -> Result<gtk::ApplicationWindow, Error> {
    let view = EditorView::new(editor)?;
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title(title)
        .default_width(800)
        .default_height(600)
        .child(&view)
        .build();

    let header = gtk::HeaderBar::new();
    let open = gtk::Button::builder()
        .label("Open")
        .action_name("win.open")
        .build();
    header.pack_start(&open);
    let edit = gio::Menu::new();
    edit.append(Some("Undo"), Some("win.undo"));
    edit.append(Some("Redo"), Some("win.redo"));
    let select = gio::Menu::new();
    select.append(Some("Select All"), Some("win.select-all"));
    let menu = gio::Menu::new();
    menu.append_section(None, &edit);
    menu.append_section(None, &select);
    let button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Main Menu")
        .menu_model(&menu)
        .primary(true)
        .build();
    header.pack_end(&button);
    window.set_titlebar(Some(&header));

    let action = |name: &str, run: fn(&EditorView)| {
        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(glib::clone!(
            #[weak]
            view,
            move |_, _| run(&view)
        ));
        window.add_action(&action);
    };
    action("undo", EditorView::undo);
    action("redo", EditorView::redo);
    action("select-all", EditorView::select_all);
    let pick = gio::SimpleAction::new("open", None);
    pick.connect_activate(glib::clone!(
        #[weak]
        app,
        #[weak]
        window,
        move |_, _| choose(&app, &window)
    ));
    window.add_action(&pick);

    view.grab_focus();
    Ok(window)
}

fn choose(app: &gtk::Application, window: &gtk::ApplicationWindow) {
    let dialog = gtk::FileDialog::builder().title("Open").modal(true).build();
    dialog.open(
        Some(window),
        gio::Cancellable::NONE,
        glib::clone!(
            #[weak]
            app,
            move |file| {
                // A dismissed dialog is an error too; there is nothing to say.
                if let Ok(file) = file {
                    open(&app, &file);
                }
            }
        ),
    );
}

/// The editor view inside a window `window` made.
pub fn view_of(window: &gtk::ApplicationWindow) -> Option<EditorView> {
    window.child().and_then(|child| child.downcast().ok())
}
