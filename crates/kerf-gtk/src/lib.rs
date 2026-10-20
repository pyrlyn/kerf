// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Kerf's Linux host (`research.md` §7, "Linux host (T17)"): a GTK 4 widget
//! that talks to the core only through `kerf-ffi`'s C ABI, the calls
//! `include/kerf.h` declares for every native host. Kerf's Slint component
//! draws the editor on the CPU; GTK owns the input, the input
//! method and the caret, and the look is GNOME Text Editor's.
//!
//! Only [`look`] builds on other targets, so its rules are tested everywhere.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod look;

#[cfg(target_os = "linux")]
mod editor;
#[cfg(target_os = "linux")]
pub mod gnome;
#[cfg(target_os = "linux")]
mod view;

#[cfg(target_os = "linux")]
pub use editor::{Command, Editor, Error};
#[cfg(target_os = "linux")]
pub use view::EditorView;
