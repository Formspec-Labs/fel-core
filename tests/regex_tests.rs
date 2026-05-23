//! Comprehensive tests for `matches()` via the regex crate backend.
//!
//! Three test surfaces:
//!
//!   - `matches_table` — uniform-shape `matches(text, pattern) → Boolean`
//!     coverage. Each row pins one regex feature. Adding a feature is one
//!     row, not one function.
//!
//!   - `matches_null_propagation_table` — null text or null pattern
//!     produces `Value::Null`. Different result type from the main table.
//!
//!   - `matches_invalid_regex_emits_diagnostic` — bespoke shape: asserts
//!     `Value::Null` + diagnostic *content* containing "invalid regex".
//!     Stays standalone because it pins message text, not the boolean
//!     outcome.
//!
//! Per the test triage plan (Cluster D in `thoughts/2026-05-23-test-suite-triage.md`),
//! the regex contract surface lives in this one file. Tests previously
//! split between this file and `evaluator_tests.rs:1085-1188` were
//! consolidated here.

use fel_core::*;

fn eval(input: &str) -> Value {
    let expr = parse(input).unwrap();
    let env = MapEnvironment::new();
    evaluate(&expr, &env).value
}

#[test]
fn matches_table() {
    // (text, pattern, expected) — `matches(text, pattern) → Boolean(expected)`.
    //
    // Patterns containing `\\` are FEL-source backslash-escapes:
    // `'\\d'` in source becomes the regex `\d`.
    //
    // Spec anchor: core/spec.llm.md §"matches (regex)".
    let cases: &[(&str, &str, bool)] = &[
        // ── Literal matching ──
        ("hello world", "world", true),
        ("hello world", "xyz", false),
        ("hello", "hello", true),
        // ── Dot (any char) ──
        ("abc", "a.c", true),
        ("aXc", "a.c", true),
        ("ac", "a.c", false), // dot requires one char
        ("ac", "^a.c$", false),
        // ── Star quantifier (zero or more) ──
        ("abc", "a.*c", true),
        ("ac", "a.*c", true), // zero between a and c
        ("aaa", "^a*$", true),
        ("", "^a*$", true), // zero a's
        ("anything", ".*", true),
        ("", "^.*$", true),
        // ── Plus quantifier (one or more) ──
        ("aaa", "^a+$", true),
        ("", "^a+$", false), // requires ≥1
        ("abc", "^.+$", true),
        ("", "^.+$", false),
        // ── Question quantifier (zero or one) ──
        ("ac", "^ab?c$", true),
        ("abc", "^ab?c$", true),
        ("abbc", "^ab?c$", false), // two b's reject
        ("a", "^.?$", true),
        ("", "^.?$", true),
        ("ab", "^.?$", false), // two chars reject
        // ── Anchors ──
        ("abc", "^abc", true),
        ("xabc", "^abc", false),
        ("abc", "abc$", true),
        ("abcx", "abc$", false),
        ("abc", "^abc$", true),
        ("abcd", "^abc$", false),
        ("xabc", "^abc$", false),
        ("", "^$", true),
        ("a", "^$", false),
        // ── \d (digit) — single and quantified ──
        ("a1b", r"\\d", true), // unanchored: contains a digit
        ("a", r"^\\d$", false),
        ("5", r"^\\d$", true),
        ("abc123", r"\\d+", true),
        ("abc", r"\\d+", false),
        ("", r"^\\d*$", true),
        ("123", r"^\\d*$", true),
        ("12345", r"^\\d+$", true),
        ("123abc", r"^\\d+$", false),
        // ── \D (non-digit) ──
        ("abc", r"^\\D+$", true),
        // ── \w (word char) ──
        ("hello_123", r"^\\w+$", true),
        ("hello_world", r"\\w+", true),
        ("abc", r"^\\w*$", true),
        // ── \W (non-word char) ──
        ("!@#", r"^\\W+$", true),
        // ── \s (whitespace) and \S (non-whitespace) ──
        (" ", r"^\\s$", true),
        ("hello world", r"\\s+", true),
        ("abc", r"\\s+", false),
        ("abc", r"^\\S+$", true),
        // ── \d? (optional digit) ──
        ("a", r"\\d?a", true),
        ("1a", r"\\d?a", true),
        // ── Escaped literal dot ──
        ("a.b", r"a\\.b", true),
        ("axb", r"^a\\.b$", false),
        // ── Alternation ──
        ("cat", "cat|dog", true),
        ("dog", "cat|dog", true),
        ("fish", "cat|dog", false),
        // ── Grouping ──
        ("abcabc", "(abc)+", true),
        // ── Character set ──
        ("a", "[abc]", true),
        ("d", "[abc]", false),
        // ── Empty pattern / empty text ──
        ("hello", "", true), // empty pattern matches anywhere
        ("", "", true),
        ("", "abc", false),
        // ── Combined patterns ──
        ("abab", ".*b", true), // greedy + backtrack
        ("aaabbb", "^a+b+$", true),
        ("aaa", "^a+b+$", false),
        ("color", "^colou?r$", true),
        ("colour", "^colou?r$", true),
        ("user@example.com", r"\\w+@\\w+", true),
    ];

    // Collect failures so one bad row doesn't hide the rest. With 60+
    // rows, a parser/regex regression could flip several at once; first-
    // fail panic would mask the blast radius.
    let mut failures: Vec<String> = Vec::new();
    for (text, pattern, expected) in cases {
        let expr = format!("matches('{text}', '{pattern}')");
        let actual = eval(&expr);
        if actual != Value::Boolean(*expected) {
            failures.push(format!(
                "  matches({text:?}, {pattern:?}): expected {expected}, got {actual:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} matches_table rows failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

#[test]
fn matches_null_propagation_table() {
    let cases: &[&str] = &["matches(null, 'abc')", "matches('abc', null)"];
    for input in cases {
        assert_eq!(eval(input), Value::Null, "input={input:?}");
    }
}

#[test]
fn matches_invalid_regex_emits_diagnostic() {
    // Bespoke shape: pins both `Value::Null` AND diagnostic message content.
    // Diagnostic-message-content tests stay standalone per the test triage
    // plan (Cluster D scope contract) — merging would weaken the assertion
    // from "contains 'invalid regex'" to just "returns Null".
    let expr = parse("matches('abc', '[invalid')").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    assert_eq!(result.value, Value::Null);
    assert!(!result.diagnostics.is_empty());
    assert!(
        result.diagnostics[0].message.contains("invalid regex"),
        "expected diagnostic to mention 'invalid regex', got: {}",
        result.diagnostics[0].message
    );
}
