// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The one place that writes ketch's lockfile format.
//!
//! A `ketch.lock` entry whose `target` is the host is the only way ketch's
//! command line holds a download to a digest the caller chose: `ketch sync`
//! refuses the asset before unpacking if its SHA-256 differs. The format is
//! ketch's (its `docs/LOCKFILE.md`), so Kerf keeps no schema for it and writes
//! only the keys it needs.

use std::path::Path;

use serde::Serialize;

use crate::{BinarySpec, Error, Pin};

/// The lockfile format version this writer produces.
const LOCK_VERSION: u32 = 1;

#[derive(Serialize)]
struct Lock<'a> {
    version: u32,
    package: [Package<'a>; 1],
}

#[derive(Serialize)]
struct Package<'a> {
    name: &'a str,
    source: String,
    version: &'a str,
    tag: &'a str,
    target: &'a str,
    asset: &'a str,
    sha256: &'a str,
}

/// Renders the one-entry lockfile for `spec` on `target`.
pub(crate) fn render(spec: &BinarySpec, target: &str, pin: &Pin) -> Result<String, Error> {
    let lock = Lock {
        version: LOCK_VERSION,
        package: [Package {
            name: &spec.name,
            source: format!("github:{}", spec.repo),
            // `version` is for humans; ketch resolves `tag`.
            version: spec.tag.strip_prefix('v').unwrap_or(&spec.tag),
            tag: &spec.tag,
            target,
            asset: &pin.asset,
            sha256: &pin.sha256,
        }],
    };
    Ok(toml::to_string(&lock)?)
}

/// Writes `text` to `path` atomically, so ketch never reads a half-written
/// lockfile.
pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<(), Error> {
    fs_kit::write::atomic(path, text).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_one_host_entry_in_ketch_format() {
        let pin = Pin {
            asset: "demo-ls-x".into(),
            sha256: "c".repeat(64),
        };
        let spec = BinarySpec {
            name: "demo-ls".into(),
            repo: "acme/demo-ls".into(),
            tag: "v1.2.3".into(),
            bin: "demo-ls".into(),
            targets: [("linux-x86_64".to_string(), pin.clone())].into(),
        };
        let text = render(&spec, "linux-x86_64", &pin).unwrap();
        let value: toml::Table = text.parse().unwrap();
        assert_eq!(value["version"].as_integer(), Some(1));
        let pkg = value["package"].as_array().unwrap()[0].as_table().unwrap();
        assert_eq!(pkg["name"].as_str(), Some("demo-ls"));
        assert_eq!(pkg["source"].as_str(), Some("github:acme/demo-ls"));
        assert_eq!(pkg["version"].as_str(), Some("1.2.3"));
        assert_eq!(pkg["tag"].as_str(), Some("v1.2.3"));
        assert_eq!(pkg["target"].as_str(), Some("linux-x86_64"));
        assert_eq!(pkg["asset"].as_str(), Some("demo-ls-x"));
        assert_eq!(pkg["sha256"].as_str(), Some("c".repeat(64).as_str()));
        assert_eq!(pkg.len(), 7, "no keys ketch would refuse as unknown");
    }
}
