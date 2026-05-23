//! Parser rejection tests — table-driven coverage of inputs the parser
//! MUST reject, paired with positive contrast cases.
//!
//! Three test surfaces:
//!
//!   - `parser_rejection_table` — inputs that MUST fail to parse. Each
//!     row carries a typed [`Cite`] (grammar section + lines, or
//!     `Policy` for non-spec rejection) and an `intent` string.
//!     Citations are deduped into named [`Cite`] consts so a single
//!     spec-line correction updates every row that pins the same rule.
//!     Failures are collected and reported in one panic so a regression
//!     affecting multiple rows surfaces the full blast radius.
//!
//!   - `valid_parse_table` — positive contrast cases. Documents what
//!     the rejection rules push against (`if(...)` is valid per §4.1
//!     even though other reserved words are not; trailing comma in
//!     object is valid per parser policy even though it's not in a
//!     function call).
//!
//!   - `parse_error_span_*` / `lexer_error_*` — bespoke standalone
//!     tests. They pin span byte-ranges on rejection, not the rejection
//!     itself. Different contract from the table; keeping them
//!     separate.

use fel_core::{Error, parse};

/// Provenance of a rejection or acceptance contract.
///
/// `Grammar { section, lines }` cites `specs/fel/fel-grammar.md`. `Policy`
/// covers non-spec parser policy or robustness guards (chained-comparison
/// rejection, depth limits, trailing-comma policy in function calls).
#[derive(Clone, Copy)]
enum Cite {
    Grammar {
        section: &'static str,
        lines: &'static str,
    },
    Policy,
}

impl Cite {
    fn render(&self) -> String {
        match self {
            Cite::Grammar { section, lines } => format!("fel-grammar.md {section} {lines}"),
            Cite::Policy => "parser policy".to_string(),
        }
    }
}

// Named citations — single source of truth per spec rule. Citing the same
// rule from N rows here means N rows update on one edit.
const G_RESERVED_WORDS: Cite = Cite::Grammar {
    section: "§3.3",
    lines: "L83-84",
};
const G_LEADING_DOT: Cite = Cite::Grammar {
    section: "§3.5",
    lines: "L132",
};
const G_TRAILING_DOT: Cite = Cite::Grammar {
    section: "§3.5",
    lines: "L133",
};
const G_OBJECT_DUP_KEYS: Cite = Cite::Grammar {
    section: "§4.2",
    lines: "L272-273",
};
const G_IF_FN_CALL: Cite = Cite::Grammar {
    section: "§4.1",
    lines: "L256-261",
};
const G_CONFORMANCE_REJECT: Cite = Cite::Grammar {
    section: "§7",
    lines: "L501-503", // "MUST reject all input strings that do not match the Expression production"
};
const G_CONFORMANCE_PIPE: Cite = Cite::Grammar {
    section: "§7",
    lines: "L510-512", // "MUST parse `|>` as a syntax error in v1.0"
};
const POLICY: Cite = Cite::Policy;

#[test]
fn parser_rejection_table() {
    let cases: &[(&str, Cite, &str)] = &[
        // ── Duplicate object keys ──
        (
            "{a: 1, a: 2}",
            G_OBJECT_DUP_KEYS,
            "duplicate keys in object literal",
        ),
        // ── Pipe operator (reserved for v2) ──
        ("1 |> 2", G_CONFORMANCE_PIPE, "pipe operator (middle)"),
        ("|> 2", G_CONFORMANCE_PIPE, "pipe operator (start)"),
        // ── Reserved words as function names ── (`if(` is special-cased VALID below)
        ("true()", G_RESERVED_WORDS, "reserved word `true` as fn"),
        ("false()", G_RESERVED_WORDS, "reserved word `false` as fn"),
        ("null()", G_RESERVED_WORDS, "reserved word `null` as fn"),
        ("and()", G_RESERVED_WORDS, "reserved word `and` as fn"),
        ("or()", G_RESERVED_WORDS, "reserved word `or` as fn"),
        (
            "not()",
            G_RESERVED_WORDS,
            "`not` is unary op, empty parens fail",
        ),
        // ── Leading/trailing dot numbers ──
        (".5", G_LEADING_DOT, "leading dot number"),
        ("5.", G_TRAILING_DOT, "trailing dot number"),
        // ── Unterminated / mismatched grouping ──
        ("(1 + 2", G_CONFORMANCE_REJECT, "unterminated parenthesis"),
        ("[1, 2", G_CONFORMANCE_REJECT, "unterminated bracket"),
        ("{a: 1", G_CONFORMANCE_REJECT, "unterminated brace"),
        ("(1 + 2]", G_CONFORMANCE_REJECT, "mismatched delimiters"),
        // ── Empty / whitespace-only input ──
        ("", G_CONFORMANCE_REJECT, "empty input"),
        ("   ", POLICY, "whitespace-only input"),
        // ── Trailing tokens / extra delimiters ──
        ("1 2", G_CONFORMANCE_REJECT, "two atoms without operator"),
        ("(1 + 2))", POLICY, "extra closing parenthesis"),
        // ── Chained comparisons (non-associative) ──
        ("1 < 2 < 3", POLICY, "chained `<` comparison"),
        ("1 <= 2 <= 3", POLICY, "chained `<=` comparison"),
        ("$a = $b = $c", POLICY, "chained `=` equality"),
        ("1 == 2 == 3", POLICY, "chained `==` equality"),
        ("$a != $b != $c", POLICY, "chained `!=` inequality"),
        // ── Invalid operator / control-flow shapes ──
        ("+", POLICY, "bare operator, no operands"),
        ("1 + + 2", POLICY, "consecutive operators"),
        ("if true else false", POLICY, "if-then-else missing `then`"),
        ("if true then 1", POLICY, "if-then-else missing `else`"),
        ("let x = 5", POLICY, "let binding missing `in`"),
        ("{a 1}", POLICY, "object literal missing colon"),
        ("{a:}", POLICY, "object literal missing value"),
        ("sum(1, 2", POLICY, "function call missing close paren"),
        (
            "sum(1,)",
            POLICY,
            "trailing comma in function call (no arg)",
        ),
    ];

    // Failure-collection: with 33 rows a parser regression can flip
    // several at once. First-fail panic would mask the blast radius.
    let mut failures: Vec<String> = Vec::new();
    for (input, cite, intent) in cases {
        if let Ok(ast) = parse(input) {
            failures.push(format!(
                "  parse({input:?}) should fail [{intent}; {}]; got AST: {ast:?}",
                cite.render()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} rejection rows parsed successfully (should have failed):\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

#[test]
fn valid_parse_table() {
    // Positive contrast: inputs that look like they might be rejected but
    // are explicitly valid per spec or parser policy. The `cite` column
    // names the contract that protects each case from rejection.
    let cases: &[(&str, Cite, &str)] = &[
        (
            "if(true, 1, 2)",
            G_IF_FN_CALL,
            "`if(...)` dispatched to FunctionCall",
        ),
        (
            "0 <= $age and $age <= 120",
            POLICY,
            "explicit range via `and` is valid (chained comparison is the bug, not this)",
        ),
        (
            "$a = $b",
            POLICY,
            "single equality parses (only chained is rejected)",
        ),
        ("[]", POLICY, "empty array literal is valid"),
        ("{}", POLICY, "empty object literal is valid"),
        (
            "{a: 1, b: 2,}",
            POLICY,
            "trailing comma in OBJECT is valid (vs `sum(1,)` rejected in fn call)",
        ),
    ];

    let mut failures: Vec<String> = Vec::new();
    for (input, cite, intent) in cases {
        if let Err(e) = parse(input) {
            failures.push(format!(
                "  parse({input:?}) should succeed [{intent}; {}]; got error: {e:?}",
                cite.render()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} valid-parse rows failed (should have parsed):\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

#[test]
fn parser_rejection_overly_deep_nesting() {
    // Bespoke setup (depth-100 paren pair) — awkward as a row; kept here.
    // The parser MUST reject before stack exhaustion. Robustness guard.
    let depth = 100usize;
    let mut input = "(".repeat(depth);
    input.push('1');
    input.push_str(&")".repeat(depth));
    assert!(
        parse(&input).is_err(),
        "depth-{depth} paren nesting should be rejected"
    );
}

// ── Parse-error span correctness ────────────────────────────────
//
// These pin span byte-ranges on rejection, not just "input is rejected".
// Different contract from `parser_rejection_table`; kept as bespoke tests.

#[test]
fn parse_error_span_points_at_trailing_token() {
    let src = "1 2";
    let err = parse(src).expect_err("two atoms without operator");
    let Error::Parse(pe) = err;
    let sp = pe.span.expect("parser should attach span");
    assert_eq!(&src[sp.start..sp.end], "2");
}

#[test]
fn parse_error_span_points_at_bad_postfix_identifier() {
    let src = "$foo.123.bar";
    let err = parse(src).expect_err("numeric postfix member");
    let Error::Parse(pe) = err;
    let sp = pe.span.expect("parser should attach span");
    assert_eq!(&src[sp.start..sp.end], "123");
}

#[test]
fn parse_error_span_points_at_bad_bracket_index() {
    let src = "$foo[1.5]";
    let err = parse(src).expect_err("fractional bracket index");
    let Error::Parse(pe) = err;
    let sp = pe.span.expect("parser should attach span");
    assert_eq!(&src[sp.start..sp.end], "1.5");
}

#[test]
fn lexer_error_includes_span_on_unterminated_string() {
    let src = "\"hello";
    let err = parse(src).expect_err("unterminated string");
    let Error::Parse(pe) = err;
    let sp = pe.span.expect("lexer should attach span");
    assert!(sp.start < sp.end || sp.start == 0);
    assert!(sp.end <= src.chars().count());
}
