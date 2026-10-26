// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Language-server and debug-adapter binaries, pinned to a release asset and
//! its SHA-256, installed from GitHub releases through the package manager.
//!
//! Kerf drives the package manager command line rather than linking `pkgmgr-core`: that
//! crate is unpublished, pulls SQLite, an async GitHub client and signature
//! stacks into the IDE, and is GPL-only, which would take the royalty-free
//! licence option away from every Kerf build. A separate process keeps both.
//!
//! [`Installer::install`] writes a one-entry `the package manager.lock` for the host and runs
//! `the package manager sync` on it, in a package manager root that belongs to Kerf alone. the package manager then
//! refuses an asset whose digest differs before unpacking it, and refuses a
//! download over the size cap. The package manager does not cap what an archive unpacks to,
//! so Kerf measures the installed tree afterwards and uninstalls it when it is
//! over [`Limits::max_installed_bytes`]. [`Installer::resolve`] returns the
//! path the core spawns.

// Tests assert with `expect`/`unwrap`; the deny list is for shipped code.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod error;
mod lockfile;
mod spec;

use std::ffi::OsString;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub use error::Error;
pub use spec::{BinarySpec, Pin, host_target};

/// How much of the package manager's error output is kept for [`Error::the package manager`]; the rest is
/// drained and dropped so a chatty failure cannot grow Kerf's memory.
const MAX_STDERR: u64 = 16 * 1024;

/// Size caps for what a release may bring onto disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The largest asset the package manager may download, in MiB; the package manager enforces it.
    pub max_download_mib: u64,
    /// The largest installed tree Kerf keeps, in bytes; Kerf enforces it.
    pub max_installed_bytes: u64,
}

impl Default for Limits {
    /// Above the largest language servers and adapters shipped today
    /// (rust-analyzer, clangd, codelldb are tens of MiB packed).
    fn default() -> Self {
        Limits {
            max_download_mib: 256,
            max_installed_bytes: 1024 * 1024 * 1024,
        }
    }
}

/// Installs and resolves pinned binaries in one the package manager root.
#[derive(Debug, Clone)]
pub struct Installer {
    program: PathBuf,
    root: PathBuf,
    limits: Limits,
    github_api: Option<String>,
}

impl Installer {
    /// An installer that runs `program` (a path, or the package manager to search `PATH`)
    /// against the package manager root `root`. The root should be Kerf's alone: the package manager
    /// links everything it installs there into `root/bin`.
    pub fn new(program: impl Into<PathBuf>, root: impl Into<PathBuf>) -> Self {
        Installer {
            program: program.into(),
            root: root.into(),
            limits: Limits::default(),
            github_api: None,
        }
    }

    /// Replaces the size caps.
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Points the package manager at another GitHub API base (GitHub Enterprise, or a local
    /// stand-in in tests) through `PKGMGR_GITHUB_API`.
    pub fn with_github_api(mut self, base: impl Into<String>) -> Self {
        self.github_api = Some(base.into());
        self
    }

    /// The package manager root this installer works in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Installs `spec` at its pinned tag and returns the binary's path.
    ///
    /// Already installed at that tag, the package manager does nothing and this only
    /// resolves. A digest mismatch or an oversize download leaves nothing
    /// installed and returns [`Error::the package manager`].
    pub fn install(&self, spec: &BinarySpec) -> Result<PathBuf, Error> {
        let pin = spec.validate()?;
        let locks = self.root.join("kerf-locks");
        std::fs::create_dir_all(&locks).map_err(|source| Error::Io {
            path: locks.clone(),
            source,
        })?;
        let lock = locks.join(format!("{}.lock", spec.name));
        lockfile::write_atomic(&lock, &lockfile::render(spec, &host_target(), pin)?)?;

        let mut args: Vec<OsString> = vec!["sync".into(), "--yes".into(), "--file".into()];
        args.push(lock.into_os_string());
        self.ketch(&args)?;

        let size = self.installed_size(&spec.name)?;
        if size > self.limits.max_installed_bytes {
            self.ketch(&["uninstall".into(), "--yes".into(), spec.name.clone().into()])?;
            return Err(Error::TooLarge {
                name: spec.name.clone(),
                size,
                limit: self.limits.max_installed_bytes,
            });
        }
        self.resolve(spec)
    }

    /// The installed binary for `spec`, for the core to spawn.
    ///
    /// The link the package manager made in `root/bin` is followed and must land inside the
    /// root, so a planted link cannot point the core at an arbitrary program.
    pub fn resolve(&self, spec: &BinarySpec) -> Result<PathBuf, Error> {
        spec.validate()?;
        let file = if cfg!(windows) {
            format!("{}.exe", spec.bin)
        } else {
            spec.bin.clone()
        };
        let link = self.root.join("bin").join(file);
        let path = match fs_kit::path::canonicalize(&link) {
            Ok(path) => path,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(Error::NotInstalled {
                    bin: spec.bin.clone(),
                });
            }
            Err(source) => return Err(Error::Io { path: link, source }),
        };
        let root = fs_kit::path::canonicalize(&self.root).map_err(|source| Error::Io {
            path: self.root.clone(),
            source,
        })?;
        if !fs_kit::path::path_starts_with(&path, &root) {
            return Err(Error::OutsideRoot { path });
        }
        if !path.is_file() {
            return Err(Error::NotInstalled {
                bin: spec.bin.clone(),
            });
        }
        Ok(path)
    }

    /// Bytes under the package's store directory, symlinks not followed. The
    /// Windows copy in `bin` duplicates one store file, so the store decides.
    fn installed_size(&self, name: &str) -> Result<u64, Error> {
        let dir = self.root.join("store").join(name);
        let mut total: u64 = 0;
        for entry in walkdir::WalkDir::new(&dir) {
            let entry = entry.map_err(|e| Error::Io {
                path: e.path().map_or_else(|| dir.clone(), Path::to_path_buf),
                source: e.into(),
            })?;
            let meta = entry.metadata().map_err(|e| Error::Io {
                path: entry.path().to_path_buf(),
                source: e.into(),
            })?;
            if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
        Ok(total)
    }

    /// Runs the package manager in Kerf's root with settings Kerf owns, and turns a failure
    /// into [`Error::the package manager`] with the head of its error output.
    fn ketch(&self, args: &[OsString]) -> Result<(), Error> {
        let mut cmd = Command::new(&self.program);
        // The user's own the package manager settings must not loosen Kerf's: drop every
        // inherited PKGMGR_* and set the ones that matter here.
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("KETCH_") {
                cmd.env_remove(&key);
            }
        }
        cmd.arg("--root")
            .arg(&self.root)
            .args(["--quiet", "--no-color", "--no-emoji"])
            .args(args)
            // No self-update or registry refresh behind Kerf's back.
            .env("KETCH_AUTO_UPDATE", "false")
            .env("KETCH_REQUIRE_CHECKSUMS", "true")
            .env(
                "KETCH_MAX_DOWNLOAD_MIB",
                self.limits.max_download_mib.to_string(),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        if let Some(api) = &self.github_api {
            cmd.env("KETCH_GITHUB_API", api);
        }

        let spawn = |source| Error::Spawn {
            program: self.program.clone(),
            source,
        };
        let mut child = cmd.spawn().map_err(spawn)?;
        let mut stderr = Vec::new();
        if let Some(mut pipe) = child.stderr.take() {
            (&mut pipe)
                .take(MAX_STDERR)
                .read_to_end(&mut stderr)
                .map_err(spawn)?;
            // Drain the rest so the package manager never blocks on a full pipe.
            io::copy(&mut pipe, &mut io::sink()).map_err(spawn)?;
        }
        let status = child.wait().map_err(spawn)?;
        if status.success() {
            return Ok(());
        }
        Err(Error::Ketch {
            status: status.to_string(),
            stderr: printable(&stderr),
        })
    }
}

/// The package manager's output repeats release and asset names, which are untrusted; keep
/// it to text that cannot drive a terminal.
fn printable(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_drops_escape_sequences() {
        assert_eq!(printable(b"bad\x1b[31m red\r\n"), "bad[31m red");
    }

    #[test]
    fn a_missing_program_is_a_spawn_error() {
        let dir = tempfile::tempdir().unwrap();
        let installer = Installer::new(dir.path().join("no-such-ketch"), dir.path());
        let spec = BinarySpec {
            name: "demo-ls".into(),
            repo: "acme/demo-ls".into(),
            tag: "v1.0.0".into(),
            bin: "demo-ls".into(),
            targets: [(
                host_target(),
                Pin {
                    asset: "demo-ls".into(),
                    sha256: "d".repeat(64),
                },
            )]
            .into(),
        };
        assert!(matches!(installer.install(&spec), Err(Error::Spawn { .. })));
        assert!(matches!(
            installer.resolve(&spec),
            Err(Error::NotInstalled { .. })
        ));
    }
}
