// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Compiles `ui/editor.slint` into Rust, so the UI lives in a `.slint` file
//! that Slint's language server and live preview can open.

fn main() -> Result<(), slint_build::CompileError> {
    // The headless UI test finds elements through the testing backend, which
    // needs Slint's element debug info. Tests build in the debug profile; a
    // release binary does not carry it.
    let debug = std::env::var("PROFILE").is_ok_and(|profile| profile == "debug");
    let config = slint_build::CompilerConfiguration::new().with_debug_info(debug);
    slint_build::compile_with_config("ui/editor.slint", config)
}
