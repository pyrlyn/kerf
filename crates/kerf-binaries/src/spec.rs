// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! What a pinned binary is, and the checks that make an untrusted one safe to
//! hand to the package manager.
//!
//! A spec arrives from a plugin manifest, so every field is checked here before
//! it reaches a file name, a lockfile or a command line.

use std::collections::BTreeMap;

use crate::Error;

/// The longest package or binary name accepted; far beyond any real one.
const MAX_NAME: usize = 64;
/// The longest release tag accepted.
const MAX_TAG: usize = 128;
/// GitHub's own limit for an owner or repository name is 100 (39 for users).
const MAX_REPO_PART: usize = 100;
/// The longest asset file name accepted; most file systems stop at 255 bytes.
const MAX_ASSET: usize = 255;

/// The release asset and its digest for one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    /// Exact asset file name in the release.
    pub asset: String,
    /// Lowercase hex SHA-256 of the asset.
    pub sha256: String,
}

/// A language-server or debug-adapter binary pinned to one release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinarySpec {
    /// Name the package manager installs the package under; also its directory in the store.
    pub name: String,
    /// GitHub `owner/repo` the release belongs to.
    pub repo: String,
    /// Exact release tag, e.g. `v1.2.3`.
    pub tag: String,
    /// Binary the package manager links for the package, without a Windows `.exe` suffix.
    pub bin: String,
    /// Asset and digest per the package manager target key (`macos-aarch64`, `linux-x86_64`,
    /// `windows-x86_64`).
    pub targets: BTreeMap<String, Pin>,
}

impl BinarySpec {
    /// Checks every field and returns the pin for the host, the only one the package manager
    /// will hold to the recorded digest.
    pub fn validate(&self) -> Result<&Pin, Error> {
        check_name("name", &self.name)?;
        check_name("bin", &self.bin)?;
        check_repo(&self.repo)?;
        check_tag(&self.tag)?;
        for (target, pin) in &self.targets {
            check_target(target)?;
            check_asset(&pin.asset)?;
            check_sha256(&pin.sha256)?;
        }
        let target = host_target();
        self.targets
            .get(&target)
            .ok_or(Error::UnsupportedTarget { target })
    }
}

/// The host as the package manager names targets: `<os>-<arch>`, from the same `std` values
/// The package manager itself reads.
pub fn host_target() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn invalid(field: &str, why: &str) -> Error {
    Error::InvalidSpec(format!("{field}: {why}"))
}

/// One path component, usable verbatim: the package manager refuses a lockfile name it would
/// have to rewrite, and Kerf joins `bin` onto the bin directory.
fn check_name(field: &str, value: &str) -> Result<(), Error> {
    if value.is_empty() || value.len() > MAX_NAME {
        return Err(invalid(field, "must be 1 to 64 bytes"));
    }
    if value.starts_with(['.', '-']) {
        return Err(invalid(field, "must not start with `.` or `-`"));
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(invalid(field, "only ASCII letters, digits, `.`, `_`, `-`"));
    }
    Ok(())
}

fn check_repo(value: &str) -> Result<(), Error> {
    let mut parts = value.split('/');
    let (Some(owner), Some(repo), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(invalid("repo", "must be `owner/repo`"));
    };
    for part in [owner, repo] {
        let ok = !part.is_empty()
            && part.len() <= MAX_REPO_PART
            && part != "."
            && part != ".."
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
        if !ok {
            return Err(invalid(
                "repo",
                "must be `owner/repo` with GitHub name characters",
            ));
        }
    }
    Ok(())
}

/// Printable ASCII without spaces: the tag goes into a URL and into the
/// messages the package manager prints.
fn check_tag(value: &str) -> Result<(), Error> {
    if value.is_empty() || value.len() > MAX_TAG {
        return Err(invalid("tag", "must be 1 to 128 bytes"));
    }
    if !value.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(invalid("tag", "only printable ASCII without spaces"));
    }
    Ok(())
}

/// Only the three targets Kerf ships on; the package manager turns an unknown key into a
/// cross-target entry that no longer holds the download to its digest.
fn check_target(value: &str) -> Result<(), Error> {
    match value {
        "macos-aarch64" | "linux-x86_64" | "windows-x86_64" => Ok(()),
        _ => Err(invalid(
            "targets",
            "keys are `macos-aarch64`, `linux-x86_64` or `windows-x86_64`",
        )),
    }
}

fn check_asset(value: &str) -> Result<(), Error> {
    if value.is_empty() || value.len() > MAX_ASSET {
        return Err(invalid("asset", "must be 1 to 255 bytes"));
    }
    if value == "." || value == ".." {
        return Err(invalid("asset", "must be a file name"));
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_graphic() && b != b'/' && b != b'\\')
    {
        return Err(invalid(
            "asset",
            "one file name of printable ASCII, no separators",
        ));
    }
    Ok(())
}

/// Lowercase only, so the digest Kerf writes compares equal to the one the package manager
/// computes without either side normalising.
fn check_sha256(value: &str) -> Result<(), Error> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("sha256", "must be 64 lowercase hex characters"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> BinarySpec {
        BinarySpec {
            name: "demo-ls".into(),
            repo: "acme/demo-ls".into(),
            tag: "v1.0.0".into(),
            bin: "demo-ls".into(),
            targets: [(
                host_target(),
                Pin {
                    asset: "demo-ls-host".into(),
                    sha256: "a".repeat(64),
                },
            )]
            .into(),
        }
    }

    #[test]
    fn a_valid_spec_returns_the_host_pin() {
        let spec = spec();
        assert_eq!(spec.validate().unwrap().asset, "demo-ls-host");
    }

    #[test]
    fn a_spec_without_the_host_target_is_unsupported() {
        let mut spec = spec();
        spec.targets.clear();
        assert!(matches!(
            spec.validate(),
            Err(Error::UnsupportedTarget { .. })
        ));
    }

    #[test]
    fn names_that_are_not_one_plain_component_are_refused() {
        for bad in ["", "../x", "a/b", ".hidden", "-flag", "a b", "a\u{1b}b"] {
            let mut s = spec();
            s.name = bad.into();
            assert!(s.validate().is_err(), "name {bad:?}");
            let mut s = spec();
            s.bin = bad.into();
            assert!(s.validate().is_err(), "bin {bad:?}");
        }
        let mut s = spec();
        s.name = "x".repeat(65);
        assert!(s.validate().is_err());
    }

    #[test]
    fn repos_must_be_owner_slash_repo() {
        for bad in ["acme", "acme/", "/x", "a/b/c", "../x", "a/..", "a b/c"] {
            let mut s = spec();
            s.repo = bad.into();
            assert!(s.validate().is_err(), "repo {bad:?}");
        }
    }

    #[test]
    fn tags_assets_and_digests_are_checked() {
        let mut s = spec();
        s.tag = "v1 0".into();
        assert!(s.validate().is_err());

        for bad in ["", "..", "a/b", "a\\b", "a\nb"] {
            let mut s = spec();
            s.targets.values_mut().for_each(|p| p.asset = bad.into());
            assert!(s.validate().is_err(), "asset {bad:?}");
        }

        for bad in ["A".repeat(64), "a".repeat(63), "g".repeat(64)] {
            let mut s = spec();
            s.targets.values_mut().for_each(|p| p.sha256 = bad.clone());
            assert!(s.validate().is_err(), "sha256 {bad:?}");
        }
    }

    #[test]
    fn unknown_target_keys_are_refused() {
        let mut s = spec();
        s.targets.insert(
            "plan9-mips".into(),
            Pin {
                asset: "x".into(),
                sha256: "b".repeat(64),
            },
        );
        assert!(matches!(s.validate(), Err(Error::InvalidSpec(_))));
    }
}
