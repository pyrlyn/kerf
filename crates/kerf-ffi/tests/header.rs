// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! `include/kerf.h` is what the hosts compile against; it must match the
//! Rust declarations, so the header is generated and a stale one fails here.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

#[test]
fn committed_header_matches_the_rust_declarations() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config = cbindgen::Config::from_file(dir.join("cbindgen.toml")).unwrap();
    let mut generated = Vec::new();
    cbindgen::Builder::new()
        .with_crate(dir)
        .with_config(config)
        .generate()
        .unwrap()
        .write(&mut generated);
    let path = dir.join("include/kerf.h");
    if std::env::var_os("KERF_BLESS").is_some() {
        std::fs::write(&path, &generated).unwrap();
    }
    // A Windows checkout may turn LF into CRLF; the header is the same.
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        committed.as_bytes() == generated.as_slice(),
        "include/kerf.h is stale: run KERF_BLESS=1 cargo test -p kerf-ffi --test header"
    );
}
