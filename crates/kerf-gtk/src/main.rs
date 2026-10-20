// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! `kerf-gtk [FILE…]`: Kerf on Linux.

#[cfg(target_os = "linux")]
fn main() -> gtk::glib::ExitCode {
    kerf_gtk::app::run()
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("kerf-gtk is the Linux host; macOS has its own in macos/.");
    std::process::ExitCode::FAILURE
}
