// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The Welcome screen and Workspace Trust path through the C functions
//! only: open untrusted, read the prompt, stay restricted, trust, reopen in
//! a new session, revoke and read what to stop; and a broken host's inputs.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Every call here plays the host within the header's contract; a SAFETY
// note per call would only repeat the header.
#![allow(clippy::undocumented_unsafe_blocks)]

use std::fs;
use std::path::Path;
use std::ptr::{null, null_mut};

use kerf_ffi::KerfStatus;
use kerf_ffi::workspace::*;
use serde_json::Value;

type PathCall = unsafe extern "C" fn(*mut KerfWorkspace, *const u8, usize) -> KerfStatus;
type CopyCall = unsafe extern "C" fn(*mut KerfWorkspace, *mut u8, usize, *mut usize) -> KerfStatus;

fn new(config: &Path, home: &Path) -> *mut KerfWorkspace {
    let (config, home) = (config.to_str().unwrap(), home.to_str().unwrap());
    let mut ws = null_mut();
    let status = unsafe {
        kerf_workspace_new(
            config.as_ptr(),
            config.len(),
            home.as_ptr(),
            home.len(),
            &mut ws,
        )
    };
    assert_eq!(status, KerfStatus::Ok);
    ws
}

fn call(f: PathCall, ws: *mut KerfWorkspace, path: &Path) -> KerfStatus {
    let path = path.to_str().unwrap();
    unsafe { f(ws, path.as_ptr(), path.len()) }
}

fn open(ws: *mut KerfWorkspace, path: &Path) -> bool {
    let path = path.to_str().unwrap();
    let mut trusted = 9;
    let status = unsafe { kerf_workspace_open(ws, path.as_ptr(), path.len(), &mut trusted) };
    assert_eq!(status, KerfStatus::Ok);
    trusted == 1
}

fn read(f: CopyCall, ws: *mut KerfWorkspace) -> Value {
    let mut len = 0;
    let first = unsafe { f(ws, null_mut(), 0, &mut len) };
    if len == 0 {
        assert_eq!(first, KerfStatus::Ok);
        return Value::Null;
    }
    assert_eq!(first, KerfStatus::TooLarge);
    let mut buf = vec![0u8; len];
    assert_eq!(
        unsafe { f(ws, buf.as_mut_ptr(), buf.len(), &mut len) },
        KerfStatus::Ok
    );
    serde_json::from_slice(&buf).unwrap()
}

#[test]
fn trust_prompt_restricted_mode_and_revoke_through_the_c_abi() {
    let dir = tempfile::tempdir().unwrap();
    let (config, home) = (dir.path().join("config"), dir.path().join("home"));
    let project = dir.path().join("project");
    let plugin = project.join(".kerf/plugins/rust-tools");
    fs::create_dir_all(&plugin).unwrap();
    fs::write(
        plugin.join("plugin.toml"),
        "api = 1\nid = \"rust-tools\"\nversion = \"0.1.0\"\nname = \"Rust tools\"\n\
         [[language_servers]]\nname = \"ra\"\nlanguages = [\"rust\"]\ncommand = \"rust-analyzer\"\nargs = [\"--stdio\"]\n",
    )
    .unwrap();

    let ws = new(&config, &home);
    assert_eq!(read(kerf_workspace_stop_json, ws), Value::Null);
    assert!(!open(ws, &project));
    let state = read(kerf_workspace_state_json, ws);
    let folder = &state["folders"][0];
    assert_eq!(folder["mode"], "restricted");
    assert_eq!(folder["asking"], true);
    let ls = &folder["prompt"]["items"][1];
    assert_eq!(ls["kind"], "language_server");
    assert_eq!(ls["command"], "rust-analyzer");
    assert_eq!(ls["args"], serde_json::json!(["--stdio"]));
    assert_eq!(state["recent"][0]["chip"], "restricted");

    assert_eq!(
        call(kerf_workspace_stay_restricted, ws, &project),
        KerfStatus::Ok
    );
    assert_eq!(
        read(kerf_workspace_state_json, ws)["folders"][0]["asking"],
        false
    );
    assert_eq!(call(kerf_workspace_trust, ws, &project), KerfStatus::Ok);
    unsafe { kerf_workspace_free(ws) };

    // A new session remembers the folder and its trust.
    let ws = new(&config, &home);
    assert_eq!(
        read(kerf_workspace_state_json, ws)["recent"][0]["chip"],
        "trusted"
    );
    assert!(open(ws, &project));

    let path = project.to_str().unwrap();
    let mut stop = 0;
    let status = unsafe { kerf_workspace_revoke(ws, path.as_ptr(), path.len(), &mut stop) };
    assert_eq!((status, stop), (KerfStatus::Ok, 1));
    let report = read(kerf_workspace_stop_json, ws);
    assert_eq!(report["stop"][0]["items"][1]["command"], "rust-analyzer");
    assert_eq!(
        read(kerf_workspace_state_json, ws)["folders"][0]["mode"],
        "restricted"
    );

    assert_eq!(call(kerf_workspace_close, ws, &project), KerfStatus::Ok);
    assert_eq!(
        call(kerf_workspace_close, ws, &project),
        KerfStatus::Nothing
    );
    assert_eq!(call(kerf_workspace_forget, ws, &project), KerfStatus::Ok);
    assert_eq!(
        call(kerf_workspace_forget, ws, &project),
        KerfStatus::Nothing
    );
    unsafe { kerf_workspace_free(ws) };
}

#[test]
fn a_broken_host_gets_statuses() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap();
    let mut ws = null_mut();
    let relative = "config";
    let status = unsafe {
        kerf_workspace_new(
            relative.as_ptr(),
            relative.len(),
            home.as_ptr(),
            home.len(),
            &mut ws,
        )
    };
    assert_eq!(status, KerfStatus::Io);
    let status = unsafe { kerf_workspace_new(home.as_ptr(), home.len(), null(), 4, &mut ws) };
    assert_eq!(status, KerfStatus::NullPointer);

    let ws = new(dir.path(), dir.path());
    let mut trusted = 0;
    let missing = dir.path().join("missing");
    let missing = missing.to_str().unwrap();
    let status = unsafe { kerf_workspace_open(ws, missing.as_ptr(), missing.len(), &mut trusted) };
    assert_eq!(status, KerfStatus::Io);
    let bad = [0xff_u8];
    let status = unsafe { kerf_workspace_open(ws, bad.as_ptr(), bad.len(), &mut trusted) };
    assert_eq!(status, KerfStatus::InvalidUtf8);
    assert_eq!(
        call(kerf_workspace_stay_restricted, ws, dir.path()),
        KerfStatus::Nothing
    );
    assert_eq!(
        unsafe { kerf_workspace_state_json(null_mut(), null_mut(), 0, &mut 0) },
        KerfStatus::NullPointer
    );
    unsafe { kerf_workspace_free(ws) };
    unsafe { kerf_workspace_free(null_mut()) };
}
