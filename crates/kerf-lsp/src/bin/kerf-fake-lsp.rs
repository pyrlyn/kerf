// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! A scripted language server for tests: `kerf-fake-lsp <script.json> [mode]`.
//! It answers a request with the script's entry for its method, and
//! `textDocument/references` with the entry whose `position` matches. Modes
//! misbehave on purpose: `silent` never answers, `oversize` claims a huge
//! message, `noisy` floods stderr, `flood` sends 1000 notifications first.

use std::io::{self, Write};
use std::process::ExitCode;

use kerf_lsp::{read_message, write_message};
use serde_json::{Value, json};

fn main() -> ExitCode {
    match serve() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kerf-fake-lsp: {e}");
            ExitCode::FAILURE
        }
    }
}

fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let script: Value =
        serde_json::from_str(&std::fs::read_to_string(args.next().ok_or("no script")?)?)?;
    let mode = args.next().unwrap_or_default();
    let mut out = io::stdout().lock();
    if mode == "noisy" {
        io::stderr().write_all(&[b'e'; 64 << 10])?;
    }
    if mode == "flood" {
        for i in 0..1000 {
            write_message(
                &mut out,
                &json!({"jsonrpc": "2.0", "method": "$/progress", "params": i}),
            )?;
        }
    }
    let mut input = io::stdin().lock();
    while let Some(message) = read_message(&mut input, 4 << 20)? {
        let method = message["method"].as_str().unwrap_or_default();
        if method == "exit" {
            return Ok(());
        }
        let Some(id) = message.get("id") else {
            continue;
        };
        if method.is_empty() || mode == "silent" {
            continue;
        }
        if mode == "oversize" {
            write!(out, "Content-Length: {}\r\n\r\n", usize::MAX)?;
            out.flush()?;
            continue;
        }
        let result = match method {
            "initialize" => json!({"capabilities": {"positionEncoding": "utf-16"}}),
            "textDocument/references" => script[method]
                .as_array()
                .and_then(|all| {
                    all.iter()
                        .find(|a| a["position"] == message["params"]["position"])
                })
                .map_or(json!([]), |a| a["locations"].clone()),
            other => script.get(other).cloned().unwrap_or(Value::Null),
        };
        // A log line first, as real servers do, so the client must skip it.
        write_message(
            &mut out,
            &json!({"jsonrpc": "2.0", "method": "window/logMessage", "params": {"type": 4, "message": method}}),
        )?;
        write_message(
            &mut out,
            &json!({"jsonrpc": "2.0", "id": id, "result": result}),
        )?;
    }
    Ok(())
}
