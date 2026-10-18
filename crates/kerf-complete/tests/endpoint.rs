// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! The client and the completer against a fake `/v1/infill`: a good answer,
//! a slow one, an oversized one, malformed ones, and requests overtaken by
//! edits or by newer requests.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant};

use httpmock::prelude::*;
use kerf_complete::{Client, Completer, Config, Context, Error};
use kerf_core::TextBuffer;
use serde_json::json;

const ROUTE: &str = "/v1/infill";

fn config(server: &MockServer) -> Config {
    Config::new(server.url(ROUTE))
}

fn context(prefix: &str, suffix: &str) -> Context {
    Context {
        prefix: prefix.into(),
        suffix: suffix.into(),
    }
}

/// Poll until an answer arrives or `within` runs out.
fn wait(completer: &mut Completer, within: Duration) -> Option<Result<String, Error>> {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if let Some(answer) = completer.poll() {
            return Some(answer);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

#[test]
fn the_middle_comes_back_for_the_text_around_the_cursor() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path(ROUTE)
            .header("authorization", "Bearer k")
            .json_body_includes(
                json!({"input_prefix": "fn main() {\n    ", "input_suffix": "\n}\n"}).to_string(),
            );
        then.status(200).json_body(json!({
            "content": "println!(\"hi\");",
            "stop_type": "eos",
            "tokens_predicted": 6,
        }));
    });
    let mut cfg = config(&server);
    cfg.api_key = Some("k".into());
    let mut completer = Completer::new(Client::new(cfg).unwrap()).unwrap();
    let buf = TextBuffer::with_text("fn main() {\n    \n}\n");
    completer.request(&buf, "fn main() {\n    ".len()).unwrap();
    assert!(completer.is_pending());

    let answer = wait(&mut completer, Duration::from_secs(5)).expect("an answer");
    assert_eq!(answer.unwrap(), "println!(\"hi\");");
    assert!(!completer.is_pending());
    mock.assert();
}

#[test]
fn a_slow_endpoint_times_out() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path(ROUTE);
        then.status(200)
            .json_body(json!({"content": "late"}))
            .delay(Duration::from_secs(3));
    });
    let mut cfg = config(&server);
    cfg.timeout = Duration::from_millis(200);
    let client = Client::new(cfg).unwrap();
    let started = Instant::now();
    let err = client.complete(&context("a", "b")).unwrap_err();
    assert!(matches!(err, Error::Timeout), "{err}");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn an_oversized_reply_is_refused() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path(ROUTE);
        then.status(200)
            .json_body(json!({"content": "x".repeat(8 * 1024)}));
    });
    let mut cfg = config(&server);
    cfg.max_reply_bytes = 1024;
    let err = Client::new(cfg)
        .unwrap()
        .complete(&context("a", "b"))
        .unwrap_err();
    assert!(matches!(err, Error::ReplyTooLarge { limit: 1024 }), "{err}");
}

#[test]
fn a_completion_over_its_cap_is_refused() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path(ROUTE);
        then.status(200)
            .json_body(json!({"content": "x".repeat(100)}));
    });
    let mut cfg = config(&server);
    cfg.max_completion_bytes = 10;
    let err = Client::new(cfg)
        .unwrap()
        .complete(&context("a", "b"))
        .unwrap_err();
    assert!(
        matches!(
            err,
            Error::TooLong {
                len: 100,
                limit: 10
            }
        ),
        "{err}"
    );
}

#[test]
fn malformed_replies_are_refused() {
    for body in ["not json", r#"{"text":"x"}"#, r#"{"content":7}"#, "[]"] {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path(ROUTE);
            then.status(200).body(body);
        });
        let err = Client::new(config(&server))
            .unwrap()
            .complete(&context("a", "b"))
            .unwrap_err();
        assert!(matches!(err, Error::BadReply(_)), "{body}: {err}");
    }
}

#[test]
fn an_error_status_keeps_the_servers_reason() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path(ROUTE);
        then.status(400)
            .body("infill_unsupported: the model has no fill-in-the-middle tokens");
    });
    let err = Client::new(config(&server))
        .unwrap()
        .complete(&context("a", "b"))
        .unwrap_err();
    match err {
        Error::Status { status, message } => {
            assert_eq!(status, 400);
            assert!(message.starts_with("infill_unsupported"), "{message}");
        }
        other => panic!("unexpected: {other}"),
    }
}

#[test]
fn an_unreachable_endpoint_is_a_transport_error() {
    // Bind and drop to get a port nothing listens on.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let client = Client::new(Config::new(format!("http://127.0.0.1:{port}{ROUTE}"))).unwrap();
    let err = client.complete(&context("a", "b")).unwrap_err();
    assert!(matches!(err, Error::Transport(_)), "{err}");
}

#[test]
fn an_edit_cancels_the_request_in_flight() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path(ROUTE);
        then.status(200)
            .json_body(json!({"content": "stale"}))
            .delay(Duration::from_millis(300));
    });
    let mut completer = Completer::new(Client::new(config(&server)).unwrap()).unwrap();
    let mut buf = TextBuffer::with_text("let x = ");
    completer.request(&buf, buf.len_bytes()).unwrap();
    // Let the worker put it on the wire: an edit before that skips the
    // request entirely, which the newest-wins test covers.
    std::thread::sleep(Duration::from_millis(100));

    let edit = buf.apply(8..8, "1").unwrap();
    completer.on_edit(&edit);
    assert!(!completer.is_pending());

    // Long enough for the slow answer to arrive and be dropped.
    assert!(wait(&mut completer, Duration::from_millis(800)).is_none());
    assert_eq!(mock.calls(), 1);
}

#[test]
fn only_the_newest_queued_request_is_sent() {
    let server = MockServer::start();
    let first = server.mock(|when, then| {
        when.method(POST)
            .path(ROUTE)
            .json_body_includes(json!({"input_prefix": "one"}).to_string());
        then.status(200)
            .json_body(json!({"content": "1"}))
            .delay(Duration::from_millis(300));
    });
    let second = server.mock(|when, then| {
        when.method(POST)
            .path(ROUTE)
            .json_body_includes(json!({"input_prefix": "two"}).to_string());
        then.status(200).json_body(json!({"content": "2"}));
    });
    let third = server.mock(|when, then| {
        when.method(POST)
            .path(ROUTE)
            .json_body_includes(json!({"input_prefix": "three"}).to_string());
        then.status(200).json_body(json!({"content": "3"}));
    });
    let mut completer = Completer::new(Client::new(config(&server)).unwrap()).unwrap();
    for text in ["one", "two", "three"] {
        let buf = TextBuffer::with_text(text);
        completer.request(&buf, buf.len_bytes()).unwrap();
        if text == "one" {
            // Let the worker put the first request on the wire before the
            // others queue behind it.
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    let answer = wait(&mut completer, Duration::from_secs(5)).expect("an answer");
    assert_eq!(answer.unwrap(), "3");
    assert_eq!(first.calls(), 1);
    assert_eq!(second.calls(), 0, "an overtaken request went out");
    assert_eq!(third.calls(), 1);
}
