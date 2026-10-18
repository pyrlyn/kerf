// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later OR LicenseRef-kerf-Royalty-Free
// Licensed under GPL-3.0 or later, or under the royalty-free licence in LICENSE-ROYALTY-FREE.md

//! An in-process inspection over a real buffer and index, its fix applied and
//! undone, and replies the runner must refuse.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use kerf_core::TextBuffer;
use kerf_inspect::contract::{
    API, Diagnostic, Edit, Fix, InspectIn, InspectOut, Severity, SymbolIn, SymbolRole,
};
use kerf_inspect::{
    CallError, Error, Inspection, MAX_DIAGNOSTICS, MAX_FIXES, MAX_MESSAGE_BYTES, MAX_REPLY_BYTES,
    run,
};
use kerf_semantic::Semantic;
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("fixtures/deprecated.rs");
const PATH: &str = "src/deprecated.rs";

/// Flags every call of `old_api` and offers one fix that renames all of them,
/// so the fix has several edits. Stands in for a WASM plugin until T6.
struct DeprecatedCall;

impl Inspection for DeprecatedCall {
    fn call(&mut self, input: &[u8]) -> Result<Vec<u8>, CallError> {
        let input: InspectIn = serde_json::from_slice(input)?;
        if input.api != API {
            return Err(format!("unsupported api {}", input.api).into());
        }
        let calls: Vec<&SymbolIn> = input
            .symbols
            .iter()
            .filter(|s| s.role == SymbolRole::Reference && s.name == "old_api")
            .collect();
        let fix = Fix {
            title: "Replace every old_api call with new_api".into(),
            edits: calls
                .iter()
                .map(|s| Edit {
                    span: s.name_span,
                    text: "new_api".into(),
                })
                .collect(),
        };
        let diagnostics = calls
            .iter()
            .map(|s| Diagnostic {
                code: "deprecated-call".into(),
                severity: Severity::Warning,
                message: "old_api is deprecated; use new_api".into(),
                span: s.name_span,
                fixes: vec![fix.clone()],
            })
            .collect();
        Ok(serde_json::to_vec(&InspectOut { diagnostics })?)
    }
}

/// Replies with fixed bytes, whatever the input.
struct Canned(Vec<u8>);

impl Inspection for Canned {
    fn call(&mut self, _input: &[u8]) -> Result<Vec<u8>, CallError> {
        Ok(self.0.clone())
    }
}

fn canned(reply: &Value) -> Canned {
    Canned(serde_json::to_vec(reply).unwrap())
}

/// One diagnostic over `span` with one fix made of `edits`.
fn one_fix(span: [usize; 2], edits: &Value) -> Value {
    json!({ "diagnostics": [{
        "code": "test",
        "severity": "error",
        "message": "problem",
        "span": { "start": span[0], "end": span[1] },
        "fixes": [{ "title": "fix it", "edits": edits }],
    }]})
}

fn open() -> (Semantic, TextBuffer) {
    let mut semantic = Semantic::new().unwrap();
    semantic.open(PATH, FIXTURE.to_string()).unwrap();
    (semantic, TextBuffer::with_text(FIXTURE))
}

fn run_with(inspection: &mut dyn Inspection) -> Result<kerf_inspect::Report, Error> {
    let (semantic, buffer) = open();
    run(inspection, &semantic, Path::new(PATH), &buffer)
}

fn calls(semantic: &Semantic, name: &str) -> usize {
    semantic.references(name).len()
}

#[test]
fn inspection_reports_each_deprecated_call_on_the_fixture() {
    let report = run_with(&mut DeprecatedCall).unwrap();
    let found = report.diagnostics();
    assert_eq!(found.len(), 2);
    for d in found {
        assert_eq!(d.code, "deprecated-call");
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(&FIXTURE[d.span.start..d.span.end], "old_api");
        assert_eq!(d.fixes.len(), 1);
        assert_eq!(d.fixes[0].edits.len(), 2);
    }
    // The definition is not a call and is not reported.
    let def = FIXTURE.find("fn old_api").unwrap() + "fn ".len();
    assert!(found.iter().all(|d| d.span.start != def));
}

#[test]
fn fix_applies_as_one_undo_step_and_undoes() {
    let (mut semantic, mut buffer) = open();
    let path = Path::new(PATH);
    let report = run(&mut DeprecatedCall, &semantic, path, &buffer).unwrap();
    assert_eq!(calls(&semantic, "old_api"), 2);
    assert_eq!(calls(&semantic, "new_api"), 1);

    let event = report.apply_fix(0, 0, &mut buffer).unwrap();
    let fixed = FIXTURE
        .replace("old_api(1)", "new_api(1)")
        .replace("old_api(a)", "new_api(a)");
    assert_eq!(buffer.to_text(), fixed);
    assert_eq!(buffer.undo_depth(), 1);
    semantic.edit(path, &event, buffer.to_text()).unwrap();
    assert_eq!(calls(&semantic, "old_api"), 0);
    assert_eq!(calls(&semantic, "new_api"), 3);

    let undo = buffer.undo().unwrap();
    assert_eq!(buffer.to_text(), FIXTURE);
    assert_eq!(buffer.undo_depth(), 0);
    semantic.edit(path, &undo, buffer.to_text()).unwrap();
    assert_eq!(calls(&semantic, "old_api"), 2);
    assert_eq!(calls(&semantic, "new_api"), 1);

    buffer.redo().unwrap();
    assert_eq!(buffer.to_text(), fixed);
}

#[test]
fn fix_on_a_changed_buffer_is_refused() {
    let (semantic, mut buffer) = open();
    let report = run(&mut DeprecatedCall, &semantic, Path::new(PATH), &buffer).unwrap();
    buffer.apply(0..0, "// moved\n").unwrap();
    let before = buffer.to_text();
    assert!(matches!(
        report.apply_fix(0, 0, &mut buffer),
        Err(Error::Stale)
    ));
    assert_eq!(buffer.to_text(), before);
}

#[test]
fn fix_under_a_same_length_edit_is_refused_and_elsewhere_applies() {
    let (semantic, mut buffer) = open();
    let report = run(&mut DeprecatedCall, &semantic, Path::new(PATH), &buffer).unwrap();
    let call = FIXTURE.find("old_api(1)").unwrap();
    buffer.apply(call..call + 1, "O").unwrap();
    assert!(matches!(
        report.apply_fix(0, 0, &mut buffer),
        Err(Error::Stale)
    ));
    buffer.undo().unwrap();

    // Same length, outside the fix's span: every offset still holds.
    buffer.apply(0..2, "##").unwrap();
    report.apply_fix(0, 0, &mut buffer).unwrap();
    let text = buffer.to_text();
    assert!(text.starts_with("##") && text.contains("new_api(1)") && text.contains("new_api(a)"));
}

#[test]
fn edits_out_of_order_are_sorted_before_applying() {
    let first = FIXTURE.find("old_api(1)").unwrap();
    let second = FIXTURE.find("old_api(a)").unwrap();
    let reply = one_fix(
        [first, first + 7],
        &json!([
            { "span": { "start": second, "end": second + 7 }, "text": "b" },
            { "span": { "start": first, "end": first + 7 }, "text": "a" },
        ]),
    );
    let (semantic, mut buffer) = open();
    let report = run(&mut canned(&reply), &semantic, Path::new(PATH), &buffer).unwrap();
    report.apply_fix(0, 0, &mut buffer).unwrap();
    let text = buffer.to_text();
    assert!(text.contains("let a = a(1);"));
    assert!(text.contains("let b = b(a);"));
}

#[test]
fn unknown_fields_are_ignored_and_omitted_ones_default() {
    let reply = json!({
        "from_a_later_minor": true,
        "diagnostics": [{
            "code": "x",
            "severity": "info",
            "message": "note",
            "span": { "start": 0, "end": 2 },
            "tags": ["unnecessary"],
        }],
    });
    let report = run_with(&mut canned(&reply)).unwrap();
    assert_eq!(report.diagnostics().len(), 1);
    assert!(report.diagnostics()[0].fixes.is_empty());
    assert!(
        run_with(&mut canned(&json!({})))
            .unwrap()
            .diagnostics()
            .is_empty()
    );
}

#[test]
fn oversized_reply_is_refused_before_parsing() {
    let mut huge = Canned(vec![b' '; MAX_REPLY_BYTES + 1]);
    assert!(matches!(
        run_with(&mut huge),
        Err(Error::ReplyTooLarge { len, max: MAX_REPLY_BYTES }) if len == MAX_REPLY_BYTES + 1
    ));
}

#[test]
fn malformed_reply_is_refused() {
    for bad in [
        &b"not json"[..],
        br#"{"diagnostics": [{"code": "x"}]}"#,
        br#"{"diagnostics": [{"code": "x", "severity": "fatal", "message": "m", "span": {"start": 0, "end": 0}}]}"#,
        br#"{"diagnostics": [{"code": "x", "severity": "info", "message": "m", "span": {"start": -1, "end": 0}}]}"#,
        br#"{"diagnostics": "none"}"#,
    ] {
        assert!(
            matches!(run_with(&mut Canned(bad.to_vec())), Err(Error::Malformed(_))),
            "{}",
            String::from_utf8_lossy(bad)
        );
    }
}

#[test]
fn spans_outside_the_file_or_inside_a_character_are_refused() {
    let len = FIXTURE.len();
    let past = one_fix([0, len + 1], &json!([]));
    assert!(matches!(
        run_with(&mut canned(&past)),
        Err(Error::OutOfBounds { .. })
    ));
    let reversed = one_fix([3, 1], &json!([]));
    assert!(matches!(
        run_with(&mut canned(&reversed)),
        Err(Error::OutOfBounds { .. })
    ));
    let dash = FIXTURE.find('—').unwrap() + 1;
    let edit = json!([{ "span": { "start": dash, "end": dash }, "text": "x" }]);
    assert!(matches!(
        run_with(&mut canned(&one_fix([0, 1], &edit))),
        Err(Error::NotCharBoundary(at)) if at == dash
    ));
}

#[test]
fn overlapping_and_empty_fixes_are_refused() {
    let overlap = json!([
        { "span": { "start": 0, "end": 5 }, "text": "a" },
        { "span": { "start": 4, "end": 6 }, "text": "b" },
    ]);
    assert!(matches!(
        run_with(&mut canned(&one_fix([0, 1], &overlap))),
        Err(Error::Overlap {
            first: 0,
            second: 4,
            ..
        })
    ));
    assert!(matches!(
        run_with(&mut canned(&one_fix([0, 1], &json!([])))),
        Err(Error::EmptyFix)
    ));
}

#[test]
fn lists_and_strings_over_their_caps_are_refused() {
    let diagnostic = json!({
        "code": "x", "severity": "info", "message": "m", "span": { "start": 0, "end": 0 },
    });
    let many = json!({ "diagnostics": vec![diagnostic; MAX_DIAGNOSTICS + 1] });
    assert!(matches!(
        run_with(&mut canned(&many)),
        Err(Error::TooMany {
            what: "diagnostics",
            ..
        })
    ));

    let mut reply = one_fix([0, 0], &json!([{ "span": { "start": 0, "end": 0 } }]));
    let fix = reply["diagnostics"][0]["fixes"][0].clone();
    reply["diagnostics"][0]["fixes"] = json!(vec![fix; MAX_FIXES + 1]);
    assert!(matches!(
        run_with(&mut canned(&reply)),
        Err(Error::TooMany { what: "fixes", .. })
    ));

    let mut long = one_fix([0, 0], &json!([{ "span": { "start": 0, "end": 0 } }]));
    long["diagnostics"][0]["message"] = json!("m".repeat(MAX_MESSAGE_BYTES + 1));
    assert!(matches!(
        run_with(&mut canned(&long)),
        Err(Error::TooLong {
            what: "diagnostic message",
            ..
        })
    ));
}

#[test]
fn control_characters_in_shown_text_are_refused() {
    let mut reply = one_fix([0, 0], &json!([{ "span": { "start": 0, "end": 0 } }]));
    reply["diagnostics"][0]["message"] = json!("line one\nline\u{1b}[2J two");
    assert!(matches!(
        run_with(&mut canned(&reply)),
        Err(Error::BadText {
            what: "diagnostic message"
        })
    ));
    reply["diagnostics"][0]["message"] = json!("ok\n\tindented");
    reply["diagnostics"][0]["fixes"][0]["title"] = json!(" ");
    assert!(matches!(
        run_with(&mut canned(&reply)),
        Err(Error::BadText { what: "fix title" })
    ));
}

#[test]
fn failed_call_and_unknown_file_are_errors() {
    struct Trap;
    impl Inspection for Trap {
        fn call(&mut self, _input: &[u8]) -> Result<Vec<u8>, CallError> {
            Err("wasm trap".into())
        }
    }
    assert!(matches!(run_with(&mut Trap), Err(Error::Call(_))));

    let (semantic, buffer) = open();
    assert!(matches!(
        run(
            &mut DeprecatedCall,
            &semantic,
            Path::new("other.rs"),
            &buffer
        ),
        Err(Error::NotOpen { .. })
    ));
}

#[test]
fn missing_fix_is_an_error_and_leaves_the_buffer() {
    let (semantic, mut buffer) = open();
    let report = run(&mut DeprecatedCall, &semantic, Path::new(PATH), &buffer).unwrap();
    assert!(matches!(
        report.apply_fix(0, 1, &mut buffer),
        Err(Error::NoSuchFix {
            diagnostic: 0,
            fix: 1
        })
    ));
    assert!(matches!(
        report.apply_fix(9, 0, &mut buffer),
        Err(Error::NoSuchFix {
            diagnostic: 9,
            fix: 0
        })
    ));
    assert_eq!(buffer.to_text(), FIXTURE);
    assert_eq!(buffer.undo_depth(), 0);
}
