// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! `kerf [FILE]`: open FILE (or an empty buffer) in the editor window.

use std::path::PathBuf;
use std::process::ExitCode;

use slint::ComponentHandle;

fn main() -> ExitCode {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    let result = kerf_shell::open(path.as_deref())
        .map_err(|err| err.to_string())
        .and_then(|window| window.run().map_err(|err| err.to_string()));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("kerf: {err}");
            ExitCode::FAILURE
        }
    }
}
