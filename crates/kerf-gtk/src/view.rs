// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The code editor widget. Kerf's Slint component draws the text on the CPU
//! and the widget shows the frame as a texture; GTK supplies the input, the
//! input method, the caret rules and the theme, as in any GTK text view.

use std::cell::{Cell, OnceCell, RefCell};
use std::ptr::{NonNull, null_mut};

use gtk::{gdk, glib, graphene, prelude::*, subclass::prelude::*};
use kerf_ffi::{self as ffi, KerfStatus};

use crate::editor::{Command, Editor, Error, check};
use crate::gnome::{self, Gnome};
use crate::look::{self, Blink, Look};

/// Kerf's view handle, freed on the GTK thread that made it.
struct Surface(NonNull<ffi::KerfView>);

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: the handle is live and nothing uses it after this; the
        // widget is disposed on the GTK thread, which made it.
        unsafe { ffi::kerf_view_free(self.0.as_ptr()) }
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct EditorView {
        pub(super) editor: OnceCell<Editor>,
        pub(super) surface: RefCell<Option<Surface>>,
        /// Pixel width, height and scale of the surface.
        pub(super) size: Cell<(u32, u32, i32)>,
        pub(super) look: RefCell<Option<Look>>,
        pub(super) dark: Cell<Option<bool>>,
        pub(super) texture: RefCell<Option<gdk::Texture>>,
        pub(super) caret: Cell<Option<graphene::Rect>>,
        pub(super) error: Cell<Option<Error>>,
        pub(super) im: gtk::IMMulticontext,
        pub(super) gnome: Gnome,
        pub(super) blink: RefCell<Option<(Blink, gtk::TickCallbackId)>>,
        pub(super) alpha: Cell<f32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EditorView {
        const NAME: &'static str = "KerfEditorView";
        type Type = super::EditorView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("kerf-editor");
            klass.set_accessible_role(gtk::AccessibleRole::TextBox);
        }
    }

    impl ObjectImpl for EditorView {
        fn constructed(&self) {
            self.parent_constructed();
            self.alpha.set(1.0);
            self.obj().setup();
        }

        fn dispose(&self) {
            self.im.set_client_widget(None::<&gtk::Widget>);
            if let Some((_, tick)) = self.blink.take() {
                tick.remove();
            }
            self.surface.take();
        }
    }

    impl WidgetImpl for EditorView {
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            self.obj().sync_look();
            self.obj().resize_surface();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }
    }
}

glib::wrapper! {
    /// The editor widget: one [`Editor`] drawn by Kerf, typed into through GTK.
    pub struct EditorView(ObjectSubclass<imp::EditorView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl EditorView {
    /// A widget showing `editor`.
    pub fn new(editor: Editor) -> Result<Self, Error> {
        let view: Self = glib::Object::new();
        let imp = view.imp();
        let mut out = null_mut();
        // SAFETY: a null theme is Kerf's own until the widget is styled; `out`
        // is a valid place for one pointer. This is the GTK thread.
        check(unsafe { ffi::kerf_view_new(1, 1, 1.0, std::ptr::null(), &mut out) })?;
        let surface = NonNull::new(out).ok_or(Error(KerfStatus::NullPointer))?;
        imp.surface.replace(Some(Surface(surface)));
        imp.size.set((1, 1, 1));
        // `set` fails only when an editor is already there, and `new` is the
        // only caller.
        let _ = imp.editor.set(editor);
        Ok(view)
    }

    fn setup(&self) {
        self.set_focusable(true);
        self.set_hexpand(true);
        self.set_vexpand(true);
        self.set_cursor_from_name(Some("text"));

        let im = &self.imp().im;
        im.set_client_widget(Some(self));
        // The input method shows its own preedit window and commits text,
        // as macOS's host does: showing it inline needs a marked range in the core.
        im.set_use_preedit(false);
        im.connect_commit(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_, text| view.insert(text)
        ));
        let keys = gtk::EventControllerKey::new();
        keys.set_im_context(Some(im));
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = view)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, state| match view.key(key, state) {
                true => glib::Propagation::Stop,
                false => glib::Propagation::Proceed,
            }
        ));
        self.add_controller(keys);

        let focus = gtk::EventControllerFocus::new();
        focus.connect_enter(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_| {
                view.im_context().focus_in();
                view.restart_blink(false);
            }
        ));
        focus.connect_leave(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_| {
                view.im_context().focus_out();
                view.stop_blink();
            }
        ));
        self.add_controller(focus);

        let click = gtk::GestureClick::new();
        click.connect_pressed(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_, _, _, _| {
                view.grab_focus();
            }
        ));
        self.add_controller(click);

        self.connect_scale_factor_notify(|view| view.resize_surface());
        self.imp().gnome.on_font_changed(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move || {
                view.imp().dark.set(None);
                view.sync_look();
            }
        ));
    }

    /// The editor this widget shows.
    pub fn editor(&self) -> Option<&Editor> {
        self.imp().editor.get()
    }

    /// The input-method context key presses go through; its `commit` signal
    /// is how text arrives.
    pub fn im_context(&self) -> gtk::IMContext {
        self.imp().im.clone().upcast()
    }

    /// The look in use, after the widget has been styled.
    pub fn look(&self) -> Option<Look> {
        self.imp().look.borrow().clone()
    }

    /// The frame on screen, as Kerf last drew it.
    pub fn frame(&self) -> Option<gdk::Texture> {
        self.imp().texture.borrow().clone()
    }

    /// The caret in widget coordinates; `None` with a selection, as in GTK's
    /// text views, or when it is outside the view.
    pub fn caret(&self) -> Option<graphene::Rect> {
        self.imp().caret.get()
    }

    /// The last render's failure.
    pub fn render_error(&self) -> Option<Error> {
        self.imp().error.get()
    }

    /// Typing or a paste: one edit, then a redraw.
    pub fn insert(&self, text: &str) {
        self.edit(|editor| editor.insert(text));
    }

    /// Undoes the last edit.
    pub fn undo(&self) {
        self.edit(|editor| editor.run(Command::Undo).map(drop));
    }

    /// Redoes the last undone edit.
    pub fn redo(&self) {
        self.edit(|editor| editor.run(Command::Redo).map(drop));
    }

    /// Selects the whole text.
    pub fn select_all(&self) {
        self.edit(|editor| editor.select(0, editor.text()?.len()));
    }

    /// A key the input method passed on; true when the editor used it.
    /// Shortcuts with Control belong to the window's actions.
    pub fn key(&self, key: gdk::Key, state: gdk::ModifierType) -> bool {
        if state.contains(gdk::ModifierType::CONTROL_MASK) {
            return false;
        }
        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let command = match key {
            gdk::Key::BackSpace => Command::Backspace,
            gdk::Key::Left | gdk::Key::KP_Left if shift => Command::SelectLeft,
            gdk::Key::Left | gdk::Key::KP_Left => Command::Left,
            gdk::Key::Right | gdk::Key::KP_Right if shift => Command::SelectRight,
            gdk::Key::Right | gdk::Key::KP_Right => Command::Right,
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => {
                self.insert("\n");
                return true;
            }
            gdk::Key::Tab => {
                self.insert("\t");
                return true;
            }
            // Other keys need core support (lines, words).
            _ => return false,
        };
        self.edit(|editor| editor.run(command).map(drop));
        true
    }

    /// One edit, then a redraw; a refused edit (e.g. an oversized paste)
    /// rings the bell. GTK keeps the caret solid while the user types.
    fn edit(&self, change: impl FnOnce(&Editor) -> Result<(), Error>) {
        let Some(editor) = self.editor() else { return };
        if change(editor).is_err() {
            self.error_bell();
        }
        self.render();
        self.restart_blink(true);
    }

    /// Picks the Adwaita or Adwaita-dark look from the theme's text colour
    /// and hands it to Kerf when it changed.
    fn sync_look(&self) {
        let dark = gnome::is_dark(self);
        if self.imp().dark.replace(Some(dark)) == Some(dark) {
            return;
        }
        let look = self.imp().gnome.look(self, dark);
        let theme = gnome::kerf_theme(&look);
        if let Some(surface) = self.imp().surface.borrow().as_ref() {
            // SAFETY: a live handle on its own thread; `theme` and the family
            // bytes it points to outlive the call.
            let _ = unsafe { ffi::kerf_view_set_theme(surface.0.as_ptr(), &theme) };
        }
        self.imp().look.replace(Some(look));
        self.render();
    }

    fn resize_surface(&self) {
        let scale = self.scale_factor().max(1);
        let pixels = |logical: i32| (logical.max(1) * scale) as u32;
        let size = (pixels(self.width()), pixels(self.height()), scale);
        if self.imp().size.replace(size) == size {
            return;
        }
        if let Some(surface) = self.imp().surface.borrow().as_ref() {
            // SAFETY: a live handle on its own thread.
            let _ =
                unsafe { ffi::kerf_view_resize(surface.0.as_ptr(), size.0, size.1, scale as f32) };
        }
        self.render();
    }

    /// Draws the editor now and moves the caret and the input method to it.
    pub fn render(&self) {
        let imp = self.imp();
        let (Some(editor), Some(surface)) =
            (imp.editor.get(), imp.surface.borrow().as_ref().map(|s| s.0))
        else {
            return;
        };
        // SAFETY: both handles are live; this is the view's own thread.
        let status = unsafe { ffi::kerf_view_render(surface.as_ptr(), editor.as_ptr()) };
        imp.error.set(match status {
            KerfStatus::Ok => self.present(surface).err(),
            KerfStatus::Nothing => None,
            status => Some(Error(status)),
        });
        let selected = editor.selection().map(|(a, c)| a != c).unwrap_or(false);
        let mut rect = ffi::KerfRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
        // SAFETY: a live handle; `rect` is a valid place for one rectangle.
        let shown = !selected
            && unsafe { ffi::kerf_view_caret(surface.as_ptr(), &mut rect) } == KerfStatus::Ok;
        let caret = shown.then(|| graphene::Rect::new(rect.x, rect.y, rect.width, rect.height));
        imp.caret.set(caret);
        if let Some(caret) = caret {
            self.im_context().set_cursor_location(&gdk::Rectangle::new(
                caret.x() as i32,
                caret.y() as i32,
                caret.width().ceil() as i32,
                caret.height().ceil() as i32,
            ));
        }
        self.queue_draw();
    }

    /// Copies Kerf's frame out into a texture.
    fn present(&self, surface: NonNull<ffi::KerfView>) -> Result<(), Error> {
        let (width, height, _) = self.imp().size.get();
        let mut len = 0;
        // SAFETY: a live handle; a null buffer with capacity 0 asks for the length.
        let _ = unsafe { ffi::kerf_view_read_rgba(surface.as_ptr(), null_mut(), 0, &mut len) };
        let mut pixels = vec![0; len];
        // SAFETY: a live handle; `pixels` is `len` writable bytes.
        check(unsafe {
            ffi::kerf_view_read_rgba(surface.as_ptr(), pixels.as_mut_ptr(), len, &mut len)
        })?;
        if len != width as usize * height as usize * 4 {
            return Err(Error(KerfStatus::Render));
        }
        let texture = gdk::MemoryTexture::new(
            width as i32,
            height as i32,
            // Slint's software renderer writes premultiplied RGBA.
            gdk::MemoryFormat::R8g8b8a8Premultiplied,
            &glib::Bytes::from_owned(pixels),
            width as usize * 4,
        );
        self.imp().texture.replace(Some(texture.upcast()));
        Ok(())
    }

    fn draw(&self, snapshot: &gtk::Snapshot) {
        let imp = self.imp();
        // A theme switch restyles the widget without a new allocation; pick
        // it up after this frame rather than re-rendering inside a snapshot.
        if imp
            .dark
            .get()
            .is_some_and(|dark| dark != gnome::is_dark(self))
        {
            glib::idle_add_local_once(glib::clone!(
                #[weak(rename_to = view)]
                self,
                move || view.sync_look()
            ));
        }
        let bounds = graphene::Rect::new(0.0, 0.0, self.width() as f32, self.height() as f32);
        if let Some(texture) = imp.texture.borrow().as_ref() {
            snapshot.append_texture(texture, &bounds);
        }
        let (Some(caret), Some(look)) = (imp.caret.get(), imp.look.borrow().clone()) else {
            return;
        };
        if !self.has_focus() || imp.alpha.get() <= 0.0 {
            return;
        }
        let aspect = self.settings().gtk_cursor_aspect_ratio();
        let [r, g, b, a] = look.caret.map(|c| f32::from(c) / 255.0);
        snapshot.append_color(
            &gdk::RGBA::new(r, g, b, a * imp.alpha.get()),
            &graphene::Rect::new(
                caret.x(),
                caret.y(),
                look::caret_width(caret.height(), aspect),
                caret.height(),
            ),
        );
    }

    fn stop_blink(&self) {
        if let Some((_, tick)) = self.imp().blink.take() {
            tick.remove();
        }
        self.imp().alpha.set(1.0);
        self.queue_draw();
    }

    /// Starts GtkTextView's blink again, or keeps the caret solid when the
    /// user turned blinking off.
    fn restart_blink(&self, delay: bool) {
        self.stop_blink();
        let settings = self.settings();
        if !settings.is_gtk_cursor_blink() || !self.has_focus() {
            return;
        }
        let blink = Blink::new(
            glib::monotonic_time(),
            settings.gtk_cursor_blink_time(),
            settings.gtk_cursor_blink_timeout(),
            delay,
        );
        let tick = self.add_tick_callback(move |view, clock| {
            let imp = view.imp();
            let alpha = blink.alpha(clock.frame_time());
            if imp.alpha.replace(alpha.unwrap_or(1.0)) != alpha.unwrap_or(1.0) {
                view.queue_draw();
            }
            match alpha {
                Some(_) => glib::ControlFlow::Continue,
                None => {
                    imp.blink.take();
                    glib::ControlFlow::Break
                }
            }
        });
        self.imp().blink.replace(Some((blink, tick)));
    }
}
