// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! `kerf-host [ROOT]`: serves one UI over stdio, so `ssh host kerf-host ROOT`
//! is the whole remote transport.

use std::io;
use std::process::ExitCode;

use kerf_remote::{Connection, Host, Limits};

fn main() -> ExitCode {
    let root = std::env::args_os().nth(1).unwrap_or_else(|| ".".into());
    let host = match Host::new(root, Limits::default()) {
        Ok(host) => host,
        Err(e) => {
            eprintln!("kerf-host: {e}");
            return ExitCode::FAILURE;
        }
    };
    match Connection::new(host).serve(io::stdin().lock(), io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kerf-host: {e}");
            ExitCode::FAILURE
        }
    }
}
