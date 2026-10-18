//! Errors for reading and writing the lockfile, kept apart from the refusal a
//! verify returns: one is a broken file, the other a package that must not load.

use std::path::PathBuf;

use crate::Digest;

/// The lockfile could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Opening, reading or writing the file failed.
    #[error("{}: {source}", path.display())]
    Io {
        /// The lockfile.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The file is larger than [`crate::MAX_BYTES`].
    #[error("{}: larger than {max} bytes", path.display())]
    TooLarge {
        /// The lockfile.
        path: PathBuf,
        /// The cap in bytes.
        max: u64,
    },
    /// The file is not UTF-8, so it cannot be TOML.
    #[error("{}: not UTF-8", path.display())]
    NotUtf8 {
        /// The lockfile.
        path: PathBuf,
    },
    /// The file is not a valid lockfile: bad TOML, an unknown key, an
    /// unsupported format or a malformed digest.
    #[error("{}: {source}", path.display())]
    Malformed {
        /// The lockfile.
        path: PathBuf,
        /// The parse error, with its line and column. Boxed because it is large
        /// next to the other variants.
        source: Box<toml::de::Error>,
    },
    /// The lockfile could not be turned into TOML.
    #[error("{}: {source}", path.display())]
    Serialize {
        /// The lockfile.
        path: PathBuf,
        /// The serializer's error.
        source: toml::ser::Error,
    },
}

/// Why [`crate::Lockfile::verify`] refused a package.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    /// The lockfile has no entry for this plugin.
    #[error("plugin `{id}` is not in the lockfile")]
    NotLocked {
        /// The plugin id.
        id: String,
    },
    /// The package's manifest version is not the pinned one.
    #[error("plugin `{id}` is version {found}, the lockfile pins {locked}")]
    VersionMismatch {
        /// The plugin id.
        id: String,
        /// The pinned version.
        locked: String,
        /// The version in the package's manifest.
        found: String,
    },
    /// The package's bytes are not the ones that were pinned.
    #[error("plugin `{id}` has digest {found}, the lockfile pins {locked}")]
    DigestMismatch {
        /// The plugin id.
        id: String,
        /// The pinned digest.
        locked: Digest,
        /// The digest the host computed over the package.
        found: String,
    },
}
