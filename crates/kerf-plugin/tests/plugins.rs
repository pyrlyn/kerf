// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The plugin lifecycle end to end, with WAT plugins written to temporary
//! user and project plugin roots: read without running code, activate on a
//! trigger, answer a call, and be disabled after a trap.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;

use kerf_plugin::{Config, HostError, Lane, MAX_STRIKES, PluginError, Plugins, Status, Trigger};
use serde_json::{Value, json};
use tempfile::TempDir;

// extism kernel imports; WAT wants imports before functions.
const IMPORTS: &str = r#"
  (import "extism:host/env" "input_length" (func $input_length (result i64)))
  (import "extism:host/env" "input_load_u8" (func $load (param i64) (result i32)))
  (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
  (import "extism:host/env" "store_u8" (func $store (param i64 i32)))
  (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))"#;

// `answer` copies its input to its output; `boom` traps.
const ANSWER_AND_BOOM: &str = r#"
  (func (export "answer") (result i32) (local $n i64) (local $off i64) (local $i i64)
    (local.set $n (call $input_length))
    (local.set $off (call $alloc (local.get $n)))
    (block $done (loop $copy
      (br_if $done (i64.ge_u (local.get $i) (local.get $n)))
      (call $store (i64.add (local.get $off) (local.get $i)) (call $load (local.get $i)))
      (local.set $i (i64.add (local.get $i) (i64.const 1)))
      (br $copy)))
    (call $output_set (local.get $off) (local.get $n))
    (i32.const 0))
  (func (export "boom") (result i32) unreachable)"#;

/// A module whose `cox_init` answers `init` (JSON) and that also exports
/// `answer` and `boom`.
fn module(init: &str) -> String {
    let len = init.len();
    let data = init.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        r#"(module {IMPORTS}
  (memory 1)
  (data (i32.const 0) "{data}")
  (func (export "cox_init") (result i32) (local $off i64) (local $i i64)
    (local.set $off (call $alloc (i64.const {len})))
    (block $done (loop $copy
      (br_if $done (i64.ge_u (local.get $i) (i64.const {len})))
      (call $store (i64.add (local.get $off) (local.get $i))
        (i32.load8_u (i32.wrap_i64 (local.get $i))))
      (local.set $i (i64.add (local.get $i) (i64.const 1)))
      (br $copy)))
    (call $output_set (local.get $off) (i64.const {len}))
    (i32.const 0))
  {ANSWER_AND_BOOM})"#
    )
}

const TRAPPING_INIT: &str = r#"(module
  (func (export "cox_init") (result i32) unreachable))"#;

fn manifest(id: &str, languages: &[&str]) -> String {
    let mut text = format!(
        "api = 1\nid = \"{id}\"\nversion = \"0.1.0\"\nname = \"{id}\"\nwasm = \"plugin.wat\"\n"
    );
    if !languages.is_empty() {
        let list: Vec<String> = languages.iter().map(|l| format!("{l:?}")).collect();
        text.push_str(&format!(
            "[[language_servers]]\nname = \"ls\"\ncommand = \"ls-bin\"\nlanguages = [{}]\n",
            list.join(", ")
        ));
    }
    text
}

fn write_package(dir: &Path, manifest: &str, wasm: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("plugin.toml"), manifest).unwrap();
    fs::write(dir.join("plugin.wat"), wasm).unwrap();
}

/// Installs a user plugin the way `wasm-plugin-host` lays it out: a version
/// directory and a `current` pointer to it.
fn user_plugin(home: &Path, id: &str, manifest: &str, wasm: &str) {
    let plugin = home.join("plugins").join(id);
    write_package(&plugin.join("versions/0123456789ab"), manifest, wasm);
    fs::write(plugin.join("current"), "0123456789ab").unwrap();
}

fn project_plugin(project: &Path, id: &str, manifest: &str, wasm: &str) {
    write_package(&project.join(".kerf/plugins").join(id), manifest, wasm);
}

struct Roots {
    home: TempDir,
    project: TempDir,
}

impl Roots {
    fn new() -> Self {
        Self {
            home: tempfile::tempdir().unwrap(),
            project: tempfile::tempdir().unwrap(),
        }
    }

    fn discover(&self) -> Plugins {
        Plugins::discover(Config {
            home: self.home.path().to_path_buf(),
            project_root: Some(self.project.path().to_path_buf()),
            session_id: "session-1".into(),
        })
    }
}

#[test]
fn manifest_is_read_without_running_code() {
    let roots = Roots::new();
    // Not a module at all: reading the manifest must not touch it.
    user_plugin(
        roots.home.path(),
        "lazy",
        &manifest("lazy", &["rust"]),
        "not wasm",
    );

    let mut plugins = roots.discover();

    assert_eq!(plugins.ids().collect::<Vec<_>>(), ["lazy"]);
    assert_eq!(plugins.status("lazy"), Some(Status::Dormant));
    assert_eq!(plugins.triggers("lazy"), [Trigger::Language("rust".into())]);
    assert_eq!(plugins.manifest("lazy").unwrap().name, "lazy");

    // Only activation reads the module, and its failure is a disable.
    let outcomes = plugins.fire(&Trigger::Language("rust".into()));
    assert!(
        matches!(&outcomes[..], [(id, Err(HostError::Disabled { .. }))] if id == "lazy"),
        "{outcomes:?}"
    );
    assert!(matches!(plugins.status("lazy"), Some(Status::Disabled(_))));
}

#[test]
fn plugin_activates_on_its_trigger_and_answers_a_call() {
    let roots = Roots::new();
    let init = r#"{"subscribe":["buffer_opened"]}"#;
    user_plugin(
        roots.home.path(),
        "rusty",
        &manifest("rusty", &["rust"]),
        &module(init),
    );

    let mut plugins = roots.discover();

    assert!(plugins.fire(&Trigger::Language("python".into())).is_empty());
    assert!(plugins.fire(&Trigger::Debug("rust".into())).is_empty());
    assert_eq!(plugins.status("rusty"), Some(Status::Dormant));

    let outcomes = plugins.fire(&Trigger::Language("rust".into()));
    assert!(
        matches!(&outcomes[..], [(id, Ok(()))] if id == "rusty"),
        "{outcomes:?}"
    );
    assert_eq!(plugins.status("rusty"), Some(Status::Active));
    assert_eq!(
        plugins.init_out("rusty").unwrap().subscribe,
        ["buffer_opened"]
    );
    // Already active: the same trigger does not load it again.
    assert!(plugins.fire(&Trigger::Language("rust".into())).is_empty());

    let reply: Option<Value> = plugins
        .call("rusty", Lane::Control, "answer", &json!({ "q": 42 }))
        .unwrap();
    assert_eq!(reply, Some(json!({ "q": 42 })));

    let absent: Option<Value> = plugins
        .call("rusty", Lane::Event, "not_exported", &json!({}))
        .unwrap();
    assert_eq!(absent, None);
}

#[test]
fn first_call_activates_a_plugin_without_triggers() {
    let roots = Roots::new();
    user_plugin(
        roots.home.path(),
        "tool",
        &manifest("tool", &[]),
        &module("{}"),
    );

    let mut plugins = roots.discover();
    assert!(plugins.triggers("tool").is_empty());
    assert_eq!(plugins.status("tool"), Some(Status::Dormant));

    let reply: Option<Value> = plugins
        .call("tool", Lane::Control, "answer", &json!("hi"))
        .unwrap();
    assert_eq!(reply, Some(json!("hi")));
    assert_eq!(plugins.status("tool"), Some(Status::Active));
}

#[test]
fn trapping_plugin_is_disabled_while_the_host_keeps_running() {
    let roots = Roots::new();
    user_plugin(
        roots.home.path(),
        "bad",
        &manifest("bad", &["rust"]),
        &module("{}"),
    );
    user_plugin(
        roots.home.path(),
        "good",
        &manifest("good", &["rust"]),
        &module("{}"),
    );

    let mut plugins = roots.discover();
    let outcomes = plugins.fire(&Trigger::Language("rust".into()));
    assert!(outcomes.iter().all(|(_, o)| o.is_ok()), "{outcomes:?}");

    for strike in 1..=MAX_STRIKES {
        let err = plugins
            .call::<_, Value>("bad", Lane::Control, "boom", &json!({}))
            .unwrap_err();
        assert!(
            matches!(
                err,
                HostError::Plugin {
                    source: PluginError::Trap { .. },
                    ..
                }
            ),
            "strike {strike}: {err:?}"
        );
    }

    assert!(matches!(plugins.status("bad"), Some(Status::Disabled(_))));
    let err = plugins
        .call::<_, Value>("bad", Lane::Control, "answer", &json!({}))
        .unwrap_err();
    assert!(matches!(err, HostError::Disabled { .. }), "{err:?}");
    assert!(
        plugins.notices().any(|n| n.contains("\"bad\" disabled")),
        "{:?}",
        plugins.notices().collect::<Vec<_>>()
    );

    // The other plugin, and the host, are untouched.
    let reply: Option<Value> = plugins
        .call("good", Lane::Control, "answer", &json!(1))
        .unwrap();
    assert_eq!(reply, Some(json!(1)));
}

#[test]
fn a_successful_call_resets_the_strikes() {
    let roots = Roots::new();
    user_plugin(
        roots.home.path(),
        "flaky",
        &manifest("flaky", &[]),
        &module("{}"),
    );
    let mut plugins = roots.discover();

    for _ in 0..MAX_STRIKES * 2 {
        for _ in 1..MAX_STRIKES {
            let _ = plugins.call::<_, Value>("flaky", Lane::Control, "boom", &json!({}));
        }
        plugins
            .call::<_, Value>("flaky", Lane::Control, "answer", &json!({}))
            .unwrap();
    }
    assert_eq!(plugins.status("flaky"), Some(Status::Active));
}

#[test]
fn trapping_init_disables_at_activation() {
    let roots = Roots::new();
    user_plugin(
        roots.home.path(),
        "broken",
        &manifest("broken", &["rust"]),
        TRAPPING_INIT,
    );

    let mut plugins = roots.discover();
    let outcomes = plugins.fire(&Trigger::Language("rust".into()));

    assert!(
        matches!(&outcomes[..], [(_, Err(HostError::Disabled { .. }))]),
        "{outcomes:?}"
    );
    assert!(matches!(
        plugins.status("broken"),
        Some(Status::Disabled(_))
    ));
    // Disabled stays disabled: a later trigger does not retry it.
    assert!(plugins.fire(&Trigger::Language("rust".into())).is_empty());
}

#[test]
fn project_plugin_waits_for_the_gate() {
    let roots = Roots::new();
    project_plugin(
        roots.project.path(),
        "repo-tool",
        &manifest("repo-tool", &["rust"]),
        &module("{}"),
    );

    let mut plugins = roots.discover();
    let outcomes = plugins.fire(&Trigger::Language("rust".into()));
    assert!(
        matches!(&outcomes[..], [(_, Err(HostError::Refused { .. }))]),
        "{outcomes:?}"
    );
    // Refused is not disabled: the plugin can load once the gate allows it.
    assert_eq!(plugins.status("repo-tool"), Some(Status::Dormant));

    let mut plugins = plugins.with_gate(Box::new(|info| {
        if info.id == "repo-tool" && !info.digest.is_empty() {
            Ok(())
        } else {
            Err("not enabled".into())
        }
    }));
    let outcomes = plugins.fire(&Trigger::Language("rust".into()));
    assert!(matches!(&outcomes[..], [(_, Ok(()))]), "{outcomes:?}");
    assert_eq!(plugins.status("repo-tool"), Some(Status::Active));
}

#[test]
fn invalid_manifest_is_listed_disabled_with_its_reason() {
    let roots = Roots::new();
    user_plugin(
        roots.home.path(),
        "typo",
        &format!("{}capabilitiez = {{}}\n", manifest("typo", &[])),
        &module("{}"),
    );

    let mut plugins = roots.discover();

    let Some(Status::Disabled(reason)) = plugins.status("typo") else {
        panic!("{:?}", plugins.status("typo"));
    };
    assert!(reason.contains("capabilitiez"), "{reason}");
    assert!(plugins.manifest("typo").is_none());
    assert!(matches!(
        plugins.activate("typo"),
        Err(HostError::Disabled { .. })
    ));
}
