// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! Installs through the real the package manager against a local stand-in for the GitHub
//! API, so no test touches the network. The package manager comes from `mise.toml`; set
//! `KERF_KETCH` to use another build.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use kerf_binaries::{BinarySpec, Error, Installer, Limits, Pin, host_target};
use sha2::{Digest, Sha256};

const REPO: &str = "acme/demo-ls";

type Routes = Arc<Mutex<HashMap<String, Vec<u8>>>>;

/// A loopback HTTP server answering GET with fixed bodies, enough of the
/// GitHub API for `the package manager sync`: a release by tag and its asset download.
struct Remote {
    base: String,
    routes: Routes,
}

impl Remote {
    fn start() -> Remote {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let routes: Routes = Arc::default();
        let served = Arc::clone(&routes);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let served = Arc::clone(&served);
                std::thread::spawn(move || answer(stream, &served));
            }
        });
        Remote { base, routes }
    }

    fn github_api(&self) -> String {
        format!("{}/gh", self.base)
    }

    fn serve(&self, path: &str, body: Vec<u8>) {
        self.routes.lock().unwrap().insert(path.to_string(), body);
    }

    /// Publishes `bytes` as the host's asset of release `tag`, with the digest
    /// GitHub would report for it, and returns the pin a spec would carry.
    fn release(&self, tag: &str, bytes: &[u8]) -> Pin {
        let asset = format!(
            "demo-ls-{}{}",
            host_target(),
            if cfg!(windows) { ".exe" } else { "" }
        );
        let sha256 = hex::encode(Sha256::digest(bytes));
        let download = format!("/dl/{tag}/{asset}");
        self.serve(&download, bytes.to_vec());
        let release = format!(
            r#"{{"tag_name":"{tag}","draft":false,"prerelease":false,
                "published_at":"2026-10-10T00:00:00Z",
                "assets":[{{"name":"{asset}","browser_download_url":"{base}{download}",
                "url":"{base}/gh/repos/{REPO}/releases/assets/1","size":{size},
                "digest":"sha256:{sha256}"}}]}}"#,
            base = self.base,
            size = bytes.len(),
        );
        self.serve(
            &format!("/gh/repos/{REPO}/releases/tags/{tag}"),
            release.into_bytes(),
        );
        Pin { asset, sha256 }
    }
}

fn answer(mut stream: TcpStream, routes: &Routes) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    // Headers are read and ignored; the request has no body.
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
        line.clear();
    }
    let path = request_line.split_whitespace().nth(1).unwrap_or("");
    let body = routes.lock().unwrap().get(path).cloned();
    let response = match body {
        Some(body) => {
            let mut head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .into_bytes();
            head.extend(body);
            head
        }
        None => {
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
        }
    };
    let _ = stream.write_all(&response);
}

/// The bytes of a fixture binary. Unix gets a script; Windows gets a file that
/// starts like a PE image so it is taken for an executable.
fn program(version: &str) -> Vec<u8> {
    if cfg!(windows) {
        format!("MZ demo-ls {version}\n").into_bytes()
    } else {
        format!("#!/bin/sh\necho demo-ls {version}\n").into_bytes()
    }
}

fn spec(tag: &str, pin: Pin) -> BinarySpec {
    BinarySpec {
        name: "demo-ls".into(),
        repo: REPO.into(),
        tag: tag.into(),
        bin: "demo-ls".into(),
        targets: [(host_target(), pin)].into(),
    }
}

fn ketch() -> PathBuf {
    std::env::var_os("KERF_KETCH").map_or_else(|| PathBuf::from("ketch"), PathBuf::from)
}

struct Fixture {
    remote: Remote,
    _dir: tempfile::TempDir,
    installer: Installer,
}

fn fixture() -> Fixture {
    let remote = Remote::start();
    let dir = tempfile::tempdir().unwrap();
    let installer =
        Installer::new(ketch(), dir.path().join("ketch")).with_github_api(remote.github_api());
    Fixture {
        remote,
        _dir: dir,
        installer,
    }
}

fn install(installer: &Installer, spec: &BinarySpec) -> Result<PathBuf, Error> {
    match installer.install(spec) {
        Err(Error::Spawn { program, source }) => panic!(
            "ketch is needed for these tests: run `mise install` or set KERF_KETCH \
             ({}: {source})",
            program.display()
        ),
        other => other,
    }
}

fn assert_not_installed(installer: &Installer, spec: &BinarySpec) {
    assert!(
        matches!(installer.resolve(spec), Err(Error::NotInstalled { .. })),
        "{:?}",
        installer.resolve(spec)
    );
}

#[test]
fn installs_a_pinned_binary_and_resolves_its_path() {
    let f = fixture();
    let bytes = program("1.0.0");
    let spec = spec("v1.0.0", f.remote.release("v1.0.0", &bytes));

    let path = install(&f.installer, &spec).unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let root = fs_kit::path::canonicalize(f.installer.root()).unwrap();
    assert!(
        fs_kit::path::path_starts_with(&path, &root),
        "{}",
        path.display()
    );
    assert_eq!(f.installer.resolve(&spec).unwrap(), path);

    // Installing the same pin again is a no-op that resolves the same path.
    assert_eq!(install(&f.installer, &spec).unwrap(), path);
}

#[test]
fn rejects_an_asset_whose_digest_differs_from_the_pin() {
    let f = fixture();
    let mut pin = f.remote.release("v1.0.0", &program("1.0.0"));
    // The release itself is consistent; only the pin disagrees, as when a
    // tag's asset is replaced after the spec was written.
    pin.sha256 = hex::encode(Sha256::digest(b"what the spec author saw"));
    let spec = spec("v1.0.0", pin);

    let err = install(&f.installer, &spec).unwrap_err();

    match &err {
        Error::Ketch { stderr, .. } => assert!(stderr.contains("does not match"), "{stderr}"),
        other => panic!("expected a ketch refusal, got {other:?}"),
    }
    assert_not_installed(&f.installer, &spec);
}

#[test]
fn refuses_a_download_over_the_size_cap() {
    let f = fixture();
    let mut bytes = program("1.0.0");
    bytes.resize(2 * 1024 * 1024, b'#');
    let spec = spec("v1.0.0", f.remote.release("v1.0.0", &bytes));
    let installer = f.installer.clone().with_limits(Limits {
        max_download_mib: 1,
        ..Limits::default()
    });

    match install(&installer, &spec) {
        Err(Error::Ketch { stderr, .. }) => {
            assert!(stderr.contains("max_download_mib"), "{stderr}")
        }
        other => panic!("expected a ketch refusal, got {other:?}"),
    }
    assert_not_installed(&installer, &spec);
}

#[test]
fn removes_an_install_over_the_unpacked_cap() {
    let f = fixture();
    let spec = spec("v1.0.0", f.remote.release("v1.0.0", &program("1.0.0")));
    let installer = f.installer.clone().with_limits(Limits {
        max_installed_bytes: 4,
        ..Limits::default()
    });

    assert!(matches!(
        install(&installer, &spec),
        Err(Error::TooLarge { limit: 4, .. })
    ));
    assert_not_installed(&installer, &spec);
}

#[test]
fn a_new_pin_updates_the_resolved_binary() {
    let f = fixture();
    let old = spec("v1.0.0", f.remote.release("v1.0.0", &program("1.0.0")));
    let new_bytes = program("1.1.0");
    let new = spec("v1.1.0", f.remote.release("v1.1.0", &new_bytes));

    install(&f.installer, &old).unwrap();
    let path = install(&f.installer, &new).unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), new_bytes);
    assert_eq!(f.installer.resolve(&new).unwrap(), path);
}

#[cfg(unix)]
#[test]
fn a_bin_link_leaving_the_root_is_refused() {
    let f = fixture();
    let spec = spec("v1.0.0", f.remote.release("v1.0.0", &program("1.0.0")));
    let bin = f.installer.root().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::os::unix::fs::symlink("/bin/sh", bin.join("demo-ls")).unwrap();

    assert!(matches!(
        f.installer.resolve(&spec),
        Err(Error::OutsideRoot { .. })
    ));
}
