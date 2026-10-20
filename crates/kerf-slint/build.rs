// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Compiles the editor component ahead of time, so shipped builds carry no
//! `.slint` source and no compiler run at start-up.

fn main() -> Result<(), slint_build::CompileError> {
    slint_build::compile("ui/editor.slint")
}
