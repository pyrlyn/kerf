//! The plugin lockfile: a committed record of which plugin ids, versions and
//! package digests a workspace runs.
//!
//! The coding agent has no lockfile, so the format is Kerf's and kept small. The digest is
//! The coding agent's package digest as is (SHA-256 hex over the package tree, the
//! `package_digest` that `agent-plugin` and `wasm-plugin-host` compute); this
//! crate stores and compares it and never defines a second one.
//!
//! ```toml
//! format = 1
//!
//! [plugins.rust-tools]
//! version = "0.3.1"
//! digest = "<64 lowercase hex characters>"
//! ```
//!
//! This crate is the one place that reads and writes the file. The plugin
//! host computes a package's digest and calls [`Lockfile::verify`] before it
//! loads the package; it never parses the file itself.

// Unit tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod error;

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub use error::{Error, VerifyError};

/// The lockfile's name at the workspace root.
pub const FILE_NAME: &str = "kerf-plugins.lock";

/// The largest lockfile [`Lockfile::read`] accepts. The file comes from the
/// repository, so it is untrusted input and gets a hard cap; thousands of
/// entries fit well under it.
pub const MAX_BYTES: u64 = 1024 * 1024;

const HEADER: &str = "# Kerf plugin lockfile. Written by Kerf; commit it.\n\n";

// A table keyed by id makes a duplicate id a TOML error instead of a second
// entry the check would have to choose between, and keeps the written file
// sorted, so a diff shows only real changes.
/// The plugins a workspace is pinned to, keyed by plugin id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Lockfile {
    /// Lockfile format version. Only `1` exists.
    format: Format,
    /// Pinned plugins, keyed by the plugin id from its manifest.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    plugins: BTreeMap<String, Entry>,
}

/// One pinned plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The version from the plugin's manifest.
    pub version: String,
    /// The coding agent's package digest of the installed package.
    pub digest: Digest,
}

/// The format version. Its own type so that a file from a newer Kerf fails to
/// parse instead of being read with the wrong meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
struct Format;

impl TryFrom<u32> for Format {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Format),
            other => Err(format!(
                "lockfile format {other} is not supported, expected 1"
            )),
        }
    }
}

impl From<Format> for u32 {
    fn from(_: Format) -> Self {
        1
    }
}

/// A package digest: 64 lowercase hex characters, the form the coding agent's
/// `package_digest` returns.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Digest(String);

impl Digest {
    /// Parses a digest, refusing anything but 64 lowercase hex characters.
    /// Uppercase is refused rather than folded so that one package has exactly
    /// one spelling in the file.
    pub fn parse(text: &str) -> Result<Self, InvalidDigest> {
        let valid = text.len() == 64
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if valid {
            Ok(Digest(text.to_owned()))
        } else {
            Err(InvalidDigest)
        }
    }

    /// The digest as hex text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Digest {
    type Error = InvalidDigest;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Digest::parse(&value)
    }
}

impl From<Digest> for String {
    fn from(digest: Digest) -> Self {
        digest.0
    }
}

/// A digest that is not 64 lowercase hex characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a package digest is 64 lowercase hex characters")]
pub struct InvalidDigest;

impl Default for Lockfile {
    fn default() -> Self {
        Self::new()
    }
}

impl Lockfile {
    /// An empty lockfile: nothing pinned.
    pub fn new() -> Self {
        Lockfile {
            format: Format,
            plugins: BTreeMap::new(),
        }
    }

    /// Reads and validates the lockfile at `path`. Every error names the file.
    pub fn read(path: &Path) -> Result<Self, Error> {
        let io = |source| Error::Io {
            path: path.to_path_buf(),
            source,
        };
        let mut bytes = Vec::new();
        // One byte past the cap tells an oversized file from one exactly at it
        // without reading the rest of it.
        File::open(path)
            .map_err(io)?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(io)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(Error::TooLarge {
                path: path.to_path_buf(),
                max: MAX_BYTES,
            });
        }
        let text = String::from_utf8(bytes).map_err(|_| Error::NotUtf8 {
            path: path.to_path_buf(),
        })?;
        toml::from_str(&text).map_err(|source| Error::Malformed {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
    }

    /// Writes the lockfile to `path` atomically, so a crash mid-write leaves
    /// the old file rather than a truncated one that would refuse every plugin.
    pub fn write(&self, path: &Path) -> Result<(), Error> {
        let body = toml::to_string(self).map_err(|source| Error::Serialize {
            path: path.to_path_buf(),
            source,
        })?;
        fs_kit::write::atomic(path, format!("{HEADER}{body}")).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Pins `id` to `entry`, replacing any earlier pin, and returns the old one.
    pub fn pin(&mut self, id: impl Into<String>, entry: Entry) -> Option<Entry> {
        self.plugins.insert(id.into(), entry)
    }

    /// Removes the pin for `id` and returns it.
    pub fn unpin(&mut self, id: &str) -> Option<Entry> {
        self.plugins.remove(id)
    }

    /// The pin for `id`, if any.
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.plugins.get(id)
    }

    /// The pinned plugins in id order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Entry)> {
        self.plugins.iter().map(|(id, entry)| (id.as_str(), entry))
    }

    /// Checks a package the host is about to load against its pin. The host
    /// passes the id and version from the package's manifest and the digest it
    /// computed over the package; anything but an exact match is refused.
    pub fn verify(&self, id: &str, version: &str, digest: &str) -> Result<(), VerifyError> {
        let entry = self
            .plugins
            .get(id)
            .ok_or_else(|| VerifyError::NotLocked { id: id.to_owned() })?;
        if entry.version != version {
            return Err(VerifyError::VersionMismatch {
                id: id.to_owned(),
                locked: entry.version.clone(),
                found: version.to_owned(),
            });
        }
        if entry.digest.as_str() != digest {
            return Err(VerifyError::DigestMismatch {
                id: id.to_owned(),
                locked: entry.digest.clone(),
                found: digest.to_owned(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
impl schemars::JsonSchema for Format {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Format".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "integer", "const": 1 })
    }
}

#[cfg(test)]
impl schemars::JsonSchema for Digest {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Digest".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string", "pattern": "^[0-9a-f]{64}$" })
    }
}

#[cfg(test)]
mod tests;
