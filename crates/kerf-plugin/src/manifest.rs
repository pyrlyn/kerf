// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The one module that reads `plugin.toml`. The shape and its structural
//! checks are the coding agent's (`agent_plugin_api::PluginManifest`, `deny_unknown_fields`
//! at every level); this module adds only what Kerf refuses on top of them.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use cox_plugin_api::PluginManifest;

/// The most `plugin.toml` bytes read. The file is repository content for a
/// project plugin, so its size is untrusted.
pub const MAX_MANIFEST_BYTES: u64 = 256 * 1024;

/// The most module bytes read at activation. Kotlin/Wasm output runs to a
/// few MiB; anything past this is refused rather than buffered.
pub const MAX_WASM_BYTES: u64 = 64 * 1024 * 1024;

/// A validated the coding agent manifest, as `wasm-plugin-host` discovery wants it.
#[derive(Debug, Clone)]
pub struct Manifest(pub PluginManifest);

impl wasm_plugin_host::Manifest for Manifest {
    fn parse(_package_dir: &Path, manifest_path: &Path) -> Result<Self, String> {
        let at = |e: &dyn std::fmt::Display| format!("{}: {e}", manifest_path.display());
        let bytes = read_capped(manifest_path, MAX_MANIFEST_BYTES).map_err(|e| at(&e))?;
        let text = std::str::from_utf8(&bytes).map_err(|e| at(&e))?;
        let manifest: PluginManifest = toml::from_str(text).map_err(|e| at(&e))?;
        manifest.validate().map_err(|e| at(&e))?;
        // `wasm-plugin-host` never turns WASI on, so a wasip1 guest would
        // only fail later at link time with a less useful message.
        if manifest.wasi {
            return Err(at(
                &"wasi = true is not supported: Kerf runs plugins without WASI",
            ));
        }
        Ok(Self(manifest))
    }

    fn id(&self) -> &str {
        &self.0.id
    }

    fn wasm(&self) -> Option<&str> {
        self.0.wasm.as_deref()
    }
}

/// Reads at most `cap` bytes of `path`; a longer file is an error, not a
/// silent truncation.
pub(crate) fn read_capped(path: &Path, cap: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(cap + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > cap {
        return Err(std::io::Error::other(format!("larger than {cap} bytes")));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_plugin_host::Manifest as _;

    const MINIMAL: &str =
        "api = 1\nid = \"demo\"\nversion = \"0.1.0\"\nname = \"Demo\"\nwasm = \"plugin.wat\"\n";

    fn parse(text: &str) -> Result<Manifest, String> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plugin.toml");
        std::fs::write(&path, text).unwrap();
        Manifest::parse(dir.path(), &path)
    }

    #[test]
    fn minimal_manifest_parses() {
        let manifest = parse(MINIMAL).unwrap();
        assert_eq!(manifest.id(), "demo");
        assert_eq!(manifest.wasm(), Some("plugin.wat"));
    }

    #[test]
    fn unknown_field_is_refused() {
        let err = parse(&format!("{MINIMAL}grants_everything = true\n")).unwrap_err();
        assert!(err.contains("grants_everything"), "{err}");
    }

    #[test]
    fn cox_validation_runs() {
        let err = parse(&MINIMAL.replace("api = 1", "api = 2")).unwrap_err();
        assert!(err.contains("api = 2"), "{err}");
    }

    #[test]
    fn wasi_is_refused() {
        let err = parse(&format!("{MINIMAL}wasi = true\n")).unwrap_err();
        assert!(err.contains("WASI"), "{err}");
    }

    #[test]
    fn oversized_manifest_is_refused() {
        let padding = "#".repeat(MAX_MANIFEST_BYTES as usize);
        let err = parse(&format!("{MINIMAL}{padding}\n")).unwrap_err();
        assert!(err.contains("larger than"), "{err}");
    }
}
