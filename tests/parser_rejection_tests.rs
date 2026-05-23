//! Parser rejection tests — verifying invalid inputs are rejected.
//!
//! Addresses audit finding: "Zero parser rejection tests".
//!
//! Two table-driven tests + four span-correctness tests:
//!
//!   - `parser_rejection_table` — each row carries `(input, spec_ref,
//!     intent)`. Spec citations are structured per row (not freeform
//!     comments) so they survive review and don't drift.
//!
//!   - `valid_parse_table` — positive contrast cases: inputs that look like
//!     they might be rejected but are explicitly valid per spec. Documents
//!     the boundary the rejection table pushes against.
//!
//!   - `parse_error_span_*` / `lexer_error_*` — kept as standalone tests:
//!     they assert on span byte-ranges, a different shape from "input is
//!     rejected".

use fel_core::{Error, parse};

/// Spec section / rationale for a parser-rejection case.
///
/// Concrete strings keep grep-discoverability ("§3.3 L83-84") while making
/// the citation a structured column rather than a freeform comment.
type SpecRef = &'static str;

/// Short prose describing what contract the rejection pins.
type Intent = &'static str;

#[test]
fn parser_rejection_table() {
    let cases: &[(&str, SpecRef, Intent)] = &[
        // ── Duplicate object keys ──
        (
            "{a: 1, a: 2}",
            "fel-grammar.md §4.2 L272-273",
            "duplicate keys in object literal",
        ),
        // ── Pipe operator (reserved for future use) ──
        (
            "1 |> 2",
            "fel-grammar.md §7 L510-512",
            "pipe operator in middle",
        ),
        (
            "|> 2",
            "fel-grammar.md §7 L510-512",
            "pipe operator at start",
        ),
        // ── Reserved words as function names ──
        //
        // §3.3: reserved words MUST NOT be used as function names. `if(` is
        // explicitly special-cased and tested as VALID in `valid_parse_table`.
        (
            "true()",
            "fel-grammar.md §3.3 L83-84",
            "reserved word `true` as fn",
        ),
        (
            "false()",
            "fel-grammar.md §3.3 L83-84",
            "reserved word `false` as fn",
        ),
        (
            "null()",
            "fel-grammar.md §3.3 L83-84",
            "reserved word `null` as fn",
        ),
        (
            "and()",
            "fel-grammar.md §3.3 L83-84",
            "reserved word `and` as fn",
        ),
        (
            "or()",
            "fel-grammar.md §3.3 L83-84",
            "reserved word `or` as fn",
        ),
        (
            "not()",
            "fel-grammar.md §3.3 L83-84",
            "`not` is unary operator, empty parens fail",
        ),
        // ── Leading/trailing dot numbers ──
        (".5", "fel-grammar.md §3.5 L132", "leading dot number"),
        ("5.", "fel-grammar.md §3.5 L133", "trailing dot number"),
        // ── Unterminated grouping ──
        (
            "(1 + 2",
            "fel-grammar.md §7 L501-503",
            "unterminated parenthesis",
        ),
        (
            "[1, 2",
            "fel-grammar.md §7 L501-503",
            "unterminated bracket",
        ),
        ("{a: 1", "fel-grammar.md §7 L501-503", "unterminated brace"),
        (
            "(1 + 2]",
            "fel-grammar.md §7 L501-503",
            "mismatched delimiters",
        ),
        // ── Empty / whitespace-only input ──
        ("", "fel-grammar.md §7 L501-503", "empty input"),
        ("   ", "non-spec (correctness)", "whitespace-only input"),
        // ── Trailing tokens ──
        (
            "1 2",
            "fel-grammar.md §7 L501-503",
            "two atoms without operator",
        ),
        (
            "(1 + 2))",
            "non-spec (correctness)",
            "extra closing parenthesis",
        ),
        // ── Chained comparisons (non-associative) ──
        (
            "1 < 2 < 3",
            "non-spec (correctness)",
            "chained `<` comparison",
        ),
        (
            "1 <= 2 <= 3",
            "non-spec (correctness)",
            "chained `<=` comparison",
        ),
        (
            "$a = $b = $c",
            "non-spec (correctness)",
            "chained `=` equality",
        ),
        (
            "1 == 2 == 3",
            "non-spec (correctness)",
            "chained `==` equality",
        ),
        (
            "$a != $b != $c",
            "non-spec (correctness)",
            "chained `!=` inequality",
        ),
        // ── Invalid operator / control-flow shapes ──
        ("+", "non-spec (correctness)", "bare operator, no operands"),
        ("1 + + 2", "non-spec (correctness)", "consecutive operators"),
        (
            "if true else false",
            "non-spec (correctness)",
            "if-then-else missing `then`",
        ),
        (
            "if true then 1",
            "non-spec (correctness)",
            "if-then-else missing `else`",
        ),
        (
            "let x = 5",
            "non-spec (correctness)",
            "let binding missing `in`",
        ),
        (
            "{a 1}",
            "non-spec (correctness)",
            "object literal missing colon",
        ),
        (
            "{a:}",
            "non-spec (correctness)",
            "object literal missing value",
        ),
        (
            "sum(1, 2",
            "non-spec (correctness)",
            "function call missing close paren",
        ),
        (
            "sum(1,)",
            "non-spec (correctness)",
            "trailing comma in function call (no arg)",
        ),
    ];

    for (input, spec_ref, intent) in cases {
        match parse(input) {
            Err(_) => {} // expected
            Ok(ast) => panic!(
                "expected parse error for {input:?} ({intent}, {spec_ref}); got AST: {ast:?}"
            ),
        }
    }
}

#[test]
fn parser_rejection_overly_deep_nesting() {
    // Robustness: excessively deep nesting is rejected before stack
    // exhaustion. Bespoke setup (depth-100 paren pair) makes a table row
    // awkward — kept as its own #[test].
    let depth = 100usize;
    let mut input = "(".repeat(depth);
    input.push('1');
    input.push_str(&")".repeat(depth));
    assert!(
        parse(&input).is_err(),
        "depth-{depth} paren nesting should be rejected"
    );
}

#[test]
fn valid_parse_table() {
    // Positive contrast cases for `parser_rejection_table` — inputs that
    // look near-rejected but are explicitly valid per spec / parser policy.
    let cases: &[(&str, &str)] = &[
        (
            "if(true, 1, 2)",
            "fel-grammar.md §4.1 L256-261: `if(...)` dispatched to FunctionCall",
        ),
        (
            "0 <= $age and $age <= 120",
            "explicit range expression with `and` is valid",
        ),
        (
            "$a = $b",
            "single equality parses (only chained equality rejected)",
        ),
        ("[]", "empty array literal is valid"),
        ("{}", "empty object literal is valid"),
        (
            "{a: 1, b: 2,}",
            "trailing comma in object literal is valid (parser policy)",
        ),
    ];

    for (input, intent) in cases {
        assert!(
            parse(input).is_ok(),
            "expected parse OK for {input:?} ({intent})"
        );
    }
}

// ── Parse-error span correctness ────────────────────────────────
//
// These pin span byte-ranges on rejection, not just "was rejected". A
// different contract from `parser_rejection_table`, kept as bespoke tests.

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
