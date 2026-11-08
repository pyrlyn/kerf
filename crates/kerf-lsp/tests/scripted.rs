// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The client against `kerf-fake-lsp`, a real process: answers merged into
//! `kerf-semantic` over the wire, and each cap met by a misbehaving server.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../../kerf-semantic/tests/common/mod.rs"]
mod common;

use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use gen_lsp_types::{DocumentSymbol, DocumentSymbolResponse, Location, PositionEncodingKind};
use kerf_lsp::{Client, Error, Limits, Spawn};
use kerf_semantic::Semantic;
use serde_json::{Value, json};

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../kerf-semantic/tests/fixtures/"
);
const URI: &str = "file:///recall.rs";

/// Spawns as asked; the trust check (T7) is another implementation.
struct Direct;

impl Spawn for Direct {
    fn spawn(
        &mut self,
        command: &mut Command,
    ) -> Result<Child, Box<dyn std::error::Error + Send + Sync>> {
        Ok(command.spawn()?)
    }
}

fn start(mode: &str, limits: Limits) -> Client {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kerf-fake-lsp"));
    command.arg(format!("{FIXTURES}recall.lsp.json")).arg(mode);
    Client::start(&mut Direct, command, limits).unwrap()
}

fn initialize(client: &mut Client) -> Result<Value, Error> {
    let capabilities = json!({"general": {"positionEncodings": ["utf-16"]}});
    client.request(
        "initialize",
        json!({"processId": null, "rootUri": null, "capabilities": capabilities}),
    )
}

/// Waits for a reader thread to catch up.
fn eventually(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn rust_recall_improves_over_the_wire() {
    let path = Path::new("recall.rs");
    let text = std::fs::read_to_string(format!("{FIXTURES}recall.rs")).unwrap();
    let expected =
        common::labels(&std::fs::read_to_string(format!("{FIXTURES}recall.labels")).unwrap());
    let mut semantic = Semantic::new().unwrap();
    semantic.open(path, text.clone()).unwrap();
    let version = semantic.version(path).unwrap();

    let mut client = start("", Limits::default());
    let init = initialize(&mut client).unwrap();
    let encoding: PositionEncodingKind =
        serde_json::from_value(init["capabilities"]["positionEncoding"].clone())
            .unwrap_or(PositionEncodingKind::UTF16);
    let doc = json!({"uri": URI, "languageId": "rust", "version": version, "text": text});
    client
        .notify("textDocument/didOpen", json!({"textDocument": doc}))
        .unwrap();
    let answer = client
        .request(
            "textDocument/documentSymbol",
            json!({"textDocument": {"uri": URI}}),
        )
        .unwrap();
    let DocumentSymbolResponse::DocumentSymbolList(symbols) =
        serde_json::from_value(answer).unwrap()
    else {
        panic!("flat symbol list");
    };

    // References of every definition the server named, as an IDE asks them.
    let mut references = Vec::new();
    let mut stack: Vec<&DocumentSymbol> = symbols.iter().collect();
    while let Some(symbol) = stack.pop() {
        stack.extend(symbol.children.iter().flatten());
        let params = json!({
            "textDocument": {"uri": URI},
            "position": symbol.selection_range.start,
            "context": {"includeDeclaration": true},
        });
        let answer = client.request("textDocument/references", params).unwrap();
        let locations: Vec<Location> = serde_json::from_value(answer).unwrap();
        references.extend(
            locations
                .into_iter()
                .filter(|l| l.uri.0 == URI)
                .map(|l| l.range),
        );
    }
    let report = semantic
        .merge_lsp(path, version, &encoding, &symbols, &references)
        .unwrap();
    println!("merge over the wire {report:?}");
    assert_eq!(report.added, 2);
    common::check(
        "rust over LSP",
        &expected,
        &common::found(&semantic, path),
        (1.0, 1.0),
    );
    assert!(client.shutdown(), "the server exits on shutdown and exit");
}

#[test]
fn a_refused_spawn_starts_nothing() {
    struct Untrusted;
    impl Spawn for Untrusted {
        fn spawn(
            &mut self,
            _: &mut Command,
        ) -> Result<Child, Box<dyn std::error::Error + Send + Sync>> {
            Err("folder is not trusted".into())
        }
    }
    let command = Command::new(env!("CARGO_BIN_EXE_kerf-fake-lsp"));
    let err = Client::start(&mut Untrusted, command, Limits::default()).unwrap_err();
    assert!(matches!(err, Error::Spawn(_)), "{err}");
}

#[test]
fn an_oversized_message_ends_the_server() {
    let mut client = start("oversize", Limits::default());
    assert!(matches!(
        initialize(&mut client),
        Err(Error::TooLarge { .. })
    ));
    assert!(initialize(&mut client).is_err(), "the server was killed");
}

#[test]
fn a_silent_server_times_out_and_is_killed_on_shutdown() {
    let limits = Limits {
        timeout: Duration::from_millis(200),
        shutdown: Duration::from_millis(200),
        ..Limits::default()
    };
    let mut client = start("silent", limits);
    assert!(matches!(
        initialize(&mut client),
        Err(Error::Timeout { .. })
    ));
    assert!(!client.shutdown(), "killed, not exited");
}

#[test]
fn stderr_keeps_only_its_tail() {
    let client = start("noisy", Limits::default());
    eventually("8 KiB of stderr", || client.stderr_tail().len() == 8 << 10);
    assert!(client.stderr_tail().bytes().all(|b| b == b'e'));
}

#[test]
fn notifications_over_the_queue_cap_are_dropped_and_answers_still_arrive() {
    let limits = Limits {
        queued_messages: 8,
        ..Limits::default()
    };
    let mut client = start("flood", limits);
    eventually("992 of 1000 dropped", || {
        client.dropped_notifications() == 992
    });
    assert!(initialize(&mut client).is_ok());
}
