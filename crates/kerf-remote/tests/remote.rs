// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The host driven as a UI would drive it, over an in-memory byte stream.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{self, Read};
use std::path::Path;

use kerf_remote::protocol::{code, method};
use kerf_remote::{Connection, Host, Limits};
use serde_json::{Value, json};
use tempfile::TempDir;

/// Hands out a few bytes per read, so frames arrive split across reads the
/// way a pipe or an SSH channel delivers them.
struct Trickle<'a> {
    bytes: &'a [u8],
    step: usize,
}

impl Read for Trickle<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.step.min(buf.len()).min(self.bytes.len());
        buf[..n].copy_from_slice(&self.bytes[..n]);
        self.bytes = &self.bytes[n..];
        Ok(n)
    }
}

fn request(id: u64, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

fn hello(id: u64, major: u32, minor: u32) -> String {
    request(
        id,
        method::INITIALIZE,
        json!({ "protocol": { "major": major, "minor": minor } }),
    )
}

/// Serves `lines` through the in-memory stream and returns every answer.
fn run(root: &Path, limits: Limits, lines: &[String]) -> Vec<Value> {
    let input = lines.iter().map(|l| format!("{l}\n")).collect::<String>();
    let mut output = Vec::new();
    let host = Host::new(root, limits).expect("grammar loads");
    Connection::new(host)
        .serve(
            Trickle {
                bytes: input.as_bytes(),
                step: 7,
            },
            &mut output,
        )
        .expect("in-memory stream");
    String::from_utf8(output)
        .expect("utf-8 answers")
        .lines()
        .map(|l| serde_json::from_str(l).expect("one JSON object per line"))
        .collect()
}

fn workspace(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    for (name, text) in files {
        std::fs::write(dir.path().join(name), text).expect("write fixture");
    }
    dir
}

fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().expect("an error answer")
}

fn lookup(id: u64, name: &str) -> String {
    request(
        id,
        method::LOOKUP,
        json!({ "name": name, "role": "definition" }),
    )
}

#[test]
fn open_edit_undo_and_symbol_query_round_trip() {
    let dir = workspace(&[("lib.rs", "fn alpha() {}\n")]);
    let file = json!({ "path": "lib.rs" });
    let answers = run(
        dir.path(),
        Limits::default(),
        &[
            hello(1, 1, 0),
            request(2, method::OPEN, file.clone()),
            lookup(3, "alpha"),
            request(
                4,
                method::EDIT,
                json!({ "path": "lib.rs", "start": 14, "end": 14, "text": "fn beta() {}\n" }),
            ),
            request(5, method::READ, file.clone()),
            lookup(6, "beta"),
            request(7, method::UNDO, file.clone()),
            request(8, method::READ, file.clone()),
            lookup(9, "beta"),
            request(10, method::REDO, file.clone()),
            lookup(11, "beta"),
        ],
    );
    assert_eq!(answers.len(), 11);
    for (i, answer) in answers.iter().enumerate() {
        assert_eq!(answer["id"], json!(i + 1), "answers keep request order");
    }
    assert_eq!(
        answers[0]["result"]["protocol"],
        json!({ "major": 1, "minor": 0 })
    );
    assert_eq!(answers[1]["result"]["len_bytes"], 14);
    let alpha = &answers[2]["result"]["symbols"][0];
    assert_eq!(
        (&alpha["path"], &alpha["kind"], &alpha["start"]),
        (&json!("lib.rs"), &json!("function"), &json!(3))
    );
    assert_eq!(answers[3]["result"]["len_bytes"], 27);
    assert_eq!(
        answers[4]["result"]["text"],
        "fn alpha() {}\nfn beta() {}\n"
    );
    assert_eq!(answers[5]["result"]["symbols"][0]["line"], 1);
    assert_eq!(answers[6]["result"]["changed"], true);
    assert_eq!(answers[7]["result"]["text"], "fn alpha() {}\n");
    assert_eq!(
        answers[8]["result"]["symbols"],
        json!([]),
        "undo re-indexes"
    );
    assert_eq!(answers[9]["result"]["changed"], true);
    assert_eq!(
        answers[10]["result"]["symbols"].as_array().map(Vec::len),
        Some(1)
    );
}

#[test]
fn unknown_major_is_refused_and_the_connection_ends() {
    let dir = workspace(&[("lib.rs", "")]);
    let answers = run(
        dir.path(),
        Limits::default(),
        &[
            hello(1, 2, 0),
            request(2, method::OPEN, json!({ "path": "lib.rs" })),
        ],
    );
    assert_eq!(answers.len(), 1, "nothing is answered after a refusal");
    assert_eq!(answers[0]["id"], 1);
    assert_eq!(
        error_code(&answers[0]),
        i64::from(code::UNSUPPORTED_VERSION)
    );
    assert_eq!(
        answers[0]["error"]["data"],
        json!({ "major": 1, "minor": 0 })
    );
}

#[test]
fn newer_minor_is_accepted() {
    let dir = workspace(&[]);
    let answers = run(dir.path(), Limits::default(), &[hello(1, 1, 99)]);
    assert_eq!(answers[0]["result"]["protocol"]["major"], 1);
}

#[test]
fn requests_before_initialize_and_a_second_initialize_are_refused() {
    let dir = workspace(&[("lib.rs", "")]);
    let answers = run(
        dir.path(),
        Limits::default(),
        &[
            request(1, method::OPEN, json!({ "path": "lib.rs" })),
            hello(2, 1, 0),
            hello(3, 1, 0),
        ],
    );
    assert_eq!(error_code(&answers[0]), i64::from(code::HANDSHAKE));
    assert!(answers[1]["result"].is_object());
    assert_eq!(error_code(&answers[2]), i64::from(code::HANDSHAKE));
}

#[test]
fn oversized_message_is_refused_and_the_connection_goes_on() {
    let dir = workspace(&[]);
    let limits = Limits {
        max_message_bytes: 256,
        ..Limits::default()
    };
    let huge = request(1, method::READ, json!({ "path": "x".repeat(1024) }));
    let answers = run(dir.path(), limits, &[huge, hello(2, 1, 0)]);
    assert_eq!(answers.len(), 2);
    assert_eq!(
        answers[0]["id"],
        Value::Null,
        "the oversized line is never parsed"
    );
    assert_eq!(error_code(&answers[0]), i64::from(code::MESSAGE_TOO_LARGE));
    assert_eq!(answers[1]["id"], 2);
    assert!(answers[1]["result"].is_object());
}

#[test]
fn oversized_reply_becomes_an_error() {
    let dir = workspace(&[("big.txt", &"y".repeat(300))]);
    let limits = Limits {
        max_message_bytes: 256,
        ..Limits::default()
    };
    let answers = run(
        dir.path(),
        limits,
        &[
            hello(1, 1, 0),
            request(2, method::OPEN, json!({ "path": "big.txt" })),
            request(3, method::READ, json!({ "path": "big.txt" })),
        ],
    );
    assert_eq!(answers[1]["result"]["len_bytes"], 300);
    assert_eq!(answers[2]["id"], 3);
    assert_eq!(error_code(&answers[2]), i64::from(code::REPLY_TOO_LARGE));
}

#[test]
fn requests_past_the_queue_cap_are_answered_busy() {
    let dir = workspace(&[]);
    let limits = Limits {
        max_queued: 2,
        ..Limits::default()
    };
    let mut conn = Connection::new(Host::new(dir.path(), limits).expect("grammar loads"));
    let burst = [hello(1, 1, 0), lookup(2, "a"), lookup(3, "b")]
        .map(|l| format!("{l}\n"))
        .concat();
    conn.receive(burst.as_bytes());
    while conn.step() {}
    let out = String::from_utf8(conn.take_output()).expect("utf-8");
    let answers: Vec<Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect();
    assert_eq!(answers.len(), 3);
    assert_eq!(
        answers[0]["id"], 3,
        "refused at once, before the queue drains"
    );
    assert_eq!(error_code(&answers[0]), i64::from(code::BUSY));
    assert!(answers[1]["result"].is_object());
    assert!(answers[2]["result"].is_object());
}

#[test]
fn malformed_lines_get_typed_errors() {
    let dir = workspace(&[]);
    let answers = run(
        dir.path(),
        Limits::default(),
        &[
            "{not json".to_owned(),
            json!({ "id": 7, "method": "x" }).to_string(),
            json!({ "jsonrpc": "2.0", "method": "note" }).to_string(),
            hello(8, 1, 0),
            request(9, "buffer/rename", json!({})),
            request(10, method::OPEN, json!({ "file": "lib.rs" })),
        ],
    );
    assert_eq!(answers.len(), 5, "the notification gets no answer");
    assert_eq!(error_code(&answers[0]), i64::from(code::PARSE));
    assert_eq!(
        (answers[1]["id"].clone(), error_code(&answers[1])),
        (json!(7), i64::from(code::INVALID_REQUEST))
    );
    assert_eq!(error_code(&answers[3]), i64::from(code::METHOD_NOT_FOUND));
    assert_eq!(error_code(&answers[4]), i64::from(code::INVALID_PARAMS));
}

#[test]
fn paths_leaving_the_root_and_buffer_caps_are_refused() {
    let dir = workspace(&[
        ("a.rs", "fn a() {}\n"),
        ("b.rs", ""),
        ("big.rs", &"/".repeat(64)),
    ]);
    let limits = Limits {
        max_buffers: 1,
        max_buffer_bytes: 32,
        ..Limits::default()
    };
    let answers = run(
        dir.path(),
        limits,
        &[
            hello(1, 1, 0),
            request(2, method::OPEN, json!({ "path": "../a.rs" })),
            request(3, method::OPEN, json!({ "path": dir.path().join("a.rs") })),
            request(4, method::OPEN, json!({ "path": "big.rs" })),
            request(5, method::OPEN, json!({ "path": "a.rs" })),
            request(6, method::OPEN, json!({ "path": "b.rs" })),
            request(
                7,
                method::EDIT,
                json!({ "path": "a.rs", "start": 0, "end": 0, "text": "x".repeat(32) }),
            ),
            request(
                8,
                method::EDIT,
                json!({ "path": "a.rs", "start": 1, "end": 99, "text": "" }),
            ),
            request(9, method::READ, json!({ "path": "b.rs" })),
        ],
    );
    for i in [1, 2, 3, 5, 6, 7, 8] {
        assert_eq!(
            error_code(&answers[i]),
            i64::from(code::BUFFER),
            "answer {i}"
        );
    }
    assert_eq!(answers[4]["result"]["len_bytes"], 10);
    let messages: Vec<&str> = answers
        .iter()
        .filter_map(|a| a["error"]["message"].as_str())
        .collect();
    assert!(messages[0].contains("relative path"), "{messages:?}");
    assert!(messages[2].contains("32-byte cap"), "{messages:?}");
    assert!(messages[3].contains("1 buffers"), "{messages:?}");
    assert!(messages[6].contains("not open"), "{messages:?}");
}

#[test]
fn kerf_host_binary_serves_stdio() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let dir = workspace(&[]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_kerf-host"))
        .arg(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn kerf-host");
    let mut stdin = child.stdin.take().expect("stdin");
    writeln!(stdin, "{}", hello(1, 1, 0)).expect("write");
    drop(stdin);
    let out = child
        .wait_with_output()
        .expect("kerf-host exits at end of input");
    assert!(out.status.success());
    let answer: Value = serde_json::from_slice(&out.stdout).expect("one JSON line");
    assert_eq!(answer["result"]["protocol"]["major"], 1);
}
