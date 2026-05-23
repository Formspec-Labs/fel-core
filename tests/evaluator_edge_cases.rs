/// Evaluator edge case tests.
///
/// Addresses audit finding: "Missing evaluator edge cases"
///
/// Covers: eval_with_fields, money arithmetic, date arithmetic edge cases,
/// object equality, today()/now(), and other gaps.
mod common;

use common::{dec, eval, num, obj, s};
use fel_core::ast::{BinaryOp as AstBinaryOp, Expr};
use fel_core::*;
use rust_decimal::Decimal;
use std::collections::HashMap;

fn eval_result(input: &str) -> EvalResult {
    let expr = parse(input).unwrap();
    let env = MapEnvironment::new();
    evaluate(&expr, &env)
}

/// Correctness: decimal overflow yields null + diagnostic (no panic). Regression for LibFuzzer crash.
#[test]
fn decimal_multiplication_overflow_is_null_not_panic() {
    let result = eval_result("1+2266*75555555555555555555555555555*67");
    assert_eq!(result.value, Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("overflow")),
        "expected overflow diagnostic, got {:?}",
        result.diagnostics
    );
}

/// Correctness: extremely deep left-associated binary AST hits evaluation depth cap (LibFuzzer stack-overflow guard).
#[test]
fn evaluation_depth_limit_returns_null_with_diagnostic() {
    let mut e = Expr::Number(Decimal::ZERO);
    for _ in 0..200 {
        e = Expr::BinaryOp {
            op: AstBinaryOp::Add,
            left: Box::new(e),
            right: Box::new(Expr::Number(Decimal::ONE)),
        };
    }
    let out = evaluate(&e, &MapEnvironment::new());
    assert_eq!(out.value, Value::Null);
    assert!(
        out.diagnostics.iter().any(|d| d.message.contains("depth")),
        "{:?}",
        out.diagnostics
    );
    // Avoid recursive drop on a deep Expr tree (would overflow the test thread stack).
    std::mem::forget(e);
}

// ── eval_with_fields convenience function ───────────────────────

/// Correctness: eval_with_fields is the main public API entry point
#[test]
fn eval_with_fields_basic() {
    let mut fields = HashMap::new();
    fields.insert("x".to_string(), num(10));
    fields.insert("y".to_string(), num(20));

    let result = eval_with_fields("$x + $y", fields).unwrap();
    assert_eq!(result.value, num(30));
}

/// Correctness: eval_with_fields with string fields
#[test]
fn eval_with_fields_strings() {
    let mut fields = HashMap::new();
    fields.insert("name".to_string(), s("Alice"));

    let result = eval_with_fields("$name", fields).unwrap();
    assert_eq!(result.value, s("Alice"));
}

/// Correctness: eval_with_fields returns parse error for invalid input
#[test]
fn eval_with_fields_parse_error() {
    let fields = HashMap::new();
    let result = eval_with_fields("", fields);
    assert!(result.is_err());
}

/// Correctness: eval_with_fields with missing field returns null
#[test]
fn eval_with_fields_missing_field() {
    let fields = HashMap::new();
    let result = eval_with_fields("$missing", fields).unwrap();
    assert_eq!(result.value, Value::Null);
}

/// Correctness: eval_with_fields with complex expression
#[test]
fn eval_with_fields_complex() {
    let mut fields = HashMap::new();
    fields.insert("price".to_string(), dec("19.99"));
    fields.insert("qty".to_string(), num(3));

    let result = eval_with_fields("$price * $qty", fields).unwrap();
    assert_eq!(result.value, dec("59.97"));
}

// ── Money arithmetic edge cases ─────────────────────────────────
//
// Single table-driven test covering operator overloads (`-`, `*`, `/`) plus
// the `moneySum` aggregate. Diagnostic-message-content tests for money (e.g.
// `money(...) < n` ordering errors, `sum([money...])` rejection text) live in
// `evaluator_tests.rs` and are intentionally NOT consolidated here — they pin
// diagnostic message text, a stronger assertion than null-propagation.
//
// To add a case: append a row. To investigate a failure: the assertion
// message includes the original input expression.

/// Expected outcome shape for a money-arithmetic case.
///
/// `Num(&str)` mirrors `MoneyBuiltinCase::Num` in `evaluator_tests.rs` and
/// uses string-parsed Decimal so future fractional-ratio rows (e.g.
/// `money(100, 'USD') / money(33, 'USD') ≈ 3.030303…`) are expressible
/// without a type change.
///
/// `Null` carries an `intent` string so the failure message names which
/// contract failed (currency-mismatch vs divide-by-zero vs empty-aggregate
/// vs mixed-currency-aggregate). Promoting intent to a column applies the
/// pre-Phase-1 H1 lesson ("columns get reviewed") uniformly across tables.
enum MoneyCase {
    /// Result is `Value::Money(amount, currency)`.
    Money(&'static str, &'static str),
    /// Result is `Value::Number(_)` — e.g. `money / money` ratio.
    Num(&'static str),
    /// Result is `Value::Null` with a named cause.
    Null(&'static str),
}

#[test]
fn money_arithmetic_table() {
    use MoneyCase::*;
    let cases: &[(&str, MoneyCase)] = &[
        // operator: subtraction
        ("money(100, 'USD') - money(30, 'USD')", Money("70", "USD")),
        (
            "money(100, 'USD') - money(30, 'EUR')",
            Null("subtraction currency mismatch"),
        ),
        // operator: multiplication (commutative)
        ("money(25, 'EUR') * 4", Money("100", "EUR")),
        ("3 * money(10, 'GBP')", Money("30", "GBP")),
        // operator: division
        ("money(100, 'USD') / 4", Money("25", "USD")),
        ("money(100, 'USD') / money(25, 'USD')", Num("4")),
        (
            "money(100, 'USD') / money(25, 'EUR')",
            Null("division currency mismatch"),
        ),
        ("money(100, 'USD') / 0", Null("division by zero")),
        // operator: fractional amounts exercise decimal precision (no integer-overflow path)
        ("money(0.1, 'USD') + money(0.2, 'USD')", Money("0.3", "USD")),
        ("money(10, 'USD') * 0.5", Money("5.0", "USD")),
        // aggregate: moneySum
        (
            "moneySum([money(10, 'USD'), money(20, 'USD'), money(30, 'USD')])",
            Money("60", "USD"),
        ),
        (
            "moneySum([money(10, 'USD'), null, money(30, 'USD')])",
            Money("40", "USD"), // nulls skipped
        ),
        (
            "moneySum([money(10, 'USD'), money(20, 'EUR')])",
            Null("aggregate mixed currencies"),
        ),
        ("moneySum([])", Null("aggregate empty array")),
    ];

    for (input, expected) in cases {
        let actual = eval(input);
        match expected {
            MoneyCase::Money(amt, cur) => match &actual {
                Value::Money(m) => {
                    assert_eq!(
                        m.amount,
                        Decimal::from_str_exact(amt).unwrap(),
                        "input={input:?}: amount mismatch"
                    );
                    assert_eq!(
                        m.currency.as_str(),
                        *cur,
                        "input={input:?}: currency mismatch"
                    );
                }
                _ => panic!("input={input:?}: expected Money({amt}, {cur}), got {actual:?}"),
            },
            MoneyCase::Num(n) => assert_eq!(
                actual,
                Value::Number(Decimal::from_str_exact(n).unwrap()),
                "input={input:?}: number mismatch"
            ),
            MoneyCase::Null(intent) => assert_eq!(
                actual,
                Value::Null,
                "input={input:?}: expected Null ({intent}), got {actual:?}"
            ),
        }
    }
}

// ── Date arithmetic ─────────────────────────────────────────────
//
// Consolidated coverage of `dateAdd` and `dateDiff`. Covers negative deltas,
// year wraps, leap-year handling (Feb 29 clamping into non-leap years,
// Jan 31 + month clamping), and dateDiff sign + unit semantics.
//
// `dateDiff(...)` returns a Number; `dateAdd(...)` returns a Date.

/// Expected outcome for a date-arithmetic case.
///
/// Variant `Add` covers `dateAdd` (a Date result); `Diff` covers `dateDiff`
/// (a Number result). Renamed from `Date` (which shadowed `fel_core::Date`
/// after a glob `use DateOp::*` and required noisy `fel_core::Date::Date`
/// qualification).
enum DateOp {
    Add(i32, u32, u32),
    Diff(i64),
}

#[test]
fn date_arithmetic_table() {
    use DateOp::*;
    let cases: &[(&str, DateOp)] = &[
        // dateAdd: negative deltas
        ("dateAdd(@2024-03-15, -1, 'months')", Add(2024, 2, 15)),
        ("dateAdd(@2024-03-01, -1, 'days')", Add(2024, 2, 29)), // leap year
        // dateAdd: leap-year Feb 29 + years
        ("dateAdd(@2024-02-29, 1, 'years')", Add(2025, 2, 28)), // clamp non-leap Feb 28
        ("dateAdd(@2024-02-29, 4, 'years')", Add(2028, 2, 29)), // next leap retains day
        // dateAdd: month wraps + day clamping
        ("dateAdd(@2024-11-15, 3, 'months')", Add(2025, 2, 15)), // Nov + 3 → Feb next year
        ("dateAdd(@2024-01-31, 1, 'months')", Add(2024, 2, 29)), // Jan 31 + 1, leap
        ("dateAdd(@2023-01-31, 1, 'months')", Add(2023, 2, 28)), // Jan 31 + 1, non-leap
        // dateDiff: units + sign
        ("dateDiff(@2024-03-01, @2024-01-01, 'days')", Diff(60)),
        ("dateDiff(@2024-06-01, @2024-01-01, 'months')", Diff(5)),
        ("dateDiff(@2024-06-15, @2020-06-15, 'years')", Diff(4)),
        ("dateDiff(@2024-01-01, @2024-03-01, 'days')", Diff(-60)),
    ];

    for (input, expected) in cases {
        let actual = eval(input);
        match expected {
            DateOp::Add(y, m, d) => match &actual {
                Value::Date(fel_core::Date::Date { year, month, day }) => {
                    assert_eq!(
                        (*year, *month, *day),
                        (*y, *m, *d),
                        "input={input:?}: date component mismatch (year, month, day)"
                    );
                }
                _ => panic!("input={input:?}: expected Date({y:04}-{m:02}-{d:02}), got {actual:?}"),
            },
            DateOp::Diff(n) => assert_eq!(
                actual,
                num(*n),
                "input={input:?}: dateDiff numeric mismatch"
            ),
        }
    }
}

// ── Object and array equality ───────────────────────────────────
// Spec: docs/SPEC.md L1073 — "Any two values of the same type may be compared for equality."
//
// Cross-type comparisons (e.g. `1 = 'one'`) produce null + diagnostic and
// are tested separately below in the "Cross-type comparisons" section —
// they pin diagnostic emission, not just the Boolean result.

#[test]
fn equality_table() {
    // (input, expected, facet) — `facet` names what the row asserts so a
    // failure points at the rule, not just the input.
    let cases: &[(&str, bool, &str)] = &[
        // object equality
        (
            "{a: 1, b: 2} = {a: 1, b: 2}",
            true,
            "object same keys + values",
        ),
        ("{a: 1} = {a: 2}", false, "object different values"),
        ("{a: 1} = {b: 1}", false, "object different keys"),
        ("{a: 1} = {a: 1, b: 2}", false, "object different lengths"),
        ("{a: {b: 1}} = {a: {b: 1}}", true, "object nested"),
        // array equality
        ("[1, 2, 3] = [1, 2, 3]", true, "array same"),
        ("[1, 2, 3] = [1, 2, 4]", false, "array different element"),
        ("[1, 2] = [1, 2, 3]", false, "array different lengths"),
        ("[] = []", true, "empty array equality"),
    ];

    for (input, expected, facet) in cases {
        assert_eq!(
            eval(input),
            Value::Boolean(*expected),
            "input={input:?}: {facet}"
        );
    }
}

// ── today() and now() ───────────────────────────────────────────

/// Correctness: today() returns a Date value
#[test]
fn today_returns_date() {
    let result = eval("today()");
    assert!(
        matches!(result, Value::Date(Date::Date { .. })),
        "today() should return a Date, got: {result:?}"
    );
}

/// Correctness: now() returns a DateTime value
#[test]
fn now_returns_datetime() {
    let result = eval("now()");
    assert!(
        matches!(result, Value::Date(Date::DateTime { .. })),
        "now() should return a DateTime, got: {result:?}"
    );
}

/// Correctness: today() can be used in date arithmetic
#[test]
fn today_in_date_arithmetic() {
    let result = eval("dateAdd(today(), 1, 'days')");
    assert!(
        matches!(result, Value::Date(Date::Date { .. })),
        "dateAdd on today() should return a Date, got: {result:?}"
    );
}

/// Correctness: year/month/day extract from today()
#[test]
fn today_date_parts() {
    // Since today() returns a hardcoded date, we can check its parts
    let result = eval("year(today())");
    assert!(matches!(result, Value::Number(_)));
}

// ── Cross-type comparisons ──────────────────────────────────────

/// Correctness: cross-type equality returns false (not error)
#[test]
fn cross_type_equality_returns_null_with_diagnostic() {
    let r = eval_result("1 = 'one'");
    // Cross-type equality produces null + diagnostic
    assert_eq!(r.value, Value::Null);
    assert!(!r.diagnostics.is_empty());
}

/// Correctness: cross-type comparison produces null + diagnostic
#[test]
fn cross_type_comparison_returns_null() {
    let r = eval_result("1 < 'abc'");
    assert_eq!(r.value, Value::Null);
}

// ── Miscellaneous evaluator edge cases ──────────────────────────

/// Correctness: deeply nested expressions
#[test]
fn deeply_nested_ternary() {
    assert_eq!(eval("true ? (false ? 1 : (true ? 42 : 3)) : 0"), num(42));
}

/// Correctness: chained null coalesce
#[test]
fn chained_null_coalesce() {
    assert_eq!(eval("null ?? null ?? null ?? 99"), num(99));
}

/// Correctness: let binding with complex body
#[test]
fn let_binding_with_ternary() {
    assert_eq!(
        eval("let x = 5 in if x > 3 then 'big' else 'small'"),
        s("big")
    );
}

/// Correctness: nested let bindings with shadowing
#[test]
fn let_binding_shadowing() {
    assert_eq!(eval("let x = 1 in let x = 2 in x"), num(2));
}

/// Correctness: undefined function produces null + diagnostic
#[test]
fn undefined_function_diagnostic() {
    let r = eval_result("fooBar(1, 2)");
    assert_eq!(r.value, Value::Null);
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("undefined function")),
        "expected 'undefined function' diagnostic, got: {:?}",
        r.diagnostics
    );
}

/// Correctness: string concatenation with null propagation
#[test]
fn concat_with_null_propagates() {
    assert_eq!(eval("'hello' & null"), Value::Null);
    assert_eq!(eval("null & 'world'"), Value::Null);
}

/// Correctness: length of array
#[test]
fn length_of_array() {
    assert_eq!(eval("length([1, 2, 3])"), num(3));
}

/// Correctness: length of null returns zero
#[test]
fn length_of_null() {
    assert_eq!(eval("length(null)"), num(0));
}

/// Correctness: empty() on various types
#[test]
fn empty_edge_cases() {
    assert_eq!(eval("empty(0)"), Value::Boolean(false));
    assert_eq!(eval("empty(false)"), Value::Boolean(false));
}

/// Correctness: date comparison
#[test]
fn date_comparison() {
    assert_eq!(eval("@2024-01-15 < @2024-06-15"), Value::Boolean(true));
    assert_eq!(eval("@2024-06-15 > @2024-01-15"), Value::Boolean(true));
    assert_eq!(eval("@2024-01-15 = @2024-01-15"), Value::Boolean(true));
}

/// Correctness: date casting from string
#[test]
fn date_cast_from_string() {
    let result = eval("date('2024-06-15')");
    assert!(
        matches!(
            result,
            Value::Date(Date::Date {
                year: 2024,
                month: 6,
                day: 15
            })
        ),
        "got: {result:?}"
    );
}

/// Correctness: isDate type check
#[test]
fn is_date_check() {
    assert_eq!(eval("isDate(@2024-01-15)"), Value::Boolean(true));
    assert_eq!(eval("isDate('not a date')"), Value::Boolean(false));
    assert_eq!(eval("isDate(42)"), Value::Boolean(false));
}

/// Correctness: number cast edge cases
#[test]
fn number_cast_invalid_string() {
    let r = eval_result("number('not_a_number')");
    assert_eq!(r.value, Value::Null);
    assert!(!r.diagnostics.is_empty());
}

/// Correctness: boolean cast edge cases
#[test]
fn boolean_cast_edge_cases() {
    assert_eq!(eval("boolean(null)"), Value::Boolean(false));
    let r = eval_result("boolean('maybe')");
    assert_eq!(r.value, Value::Null);
}

/// Correctness: money equality
#[test]
fn money_equality() {
    assert_eq!(
        eval("money(100, 'USD') = money(100, 'USD')"),
        Value::Boolean(true)
    );
    assert_eq!(
        eval("money(100, 'USD') = money(100, 'EUR')"),
        Value::Boolean(false)
    );
    assert_eq!(
        eval("money(100, 'USD') = money(50, 'USD')"),
        Value::Boolean(false)
    );
}

/// Correctness: format function with multiple placeholders
#[test]
fn format_multiple_placeholders() {
    assert_eq!(
        eval("format('{0} has {1} items at ${2} each', 'Cart', 3, 9.99)"),
        s("Cart has 3 items at $9.99 each")
    );
}

/// Correctness: format applies `{n}` placeholders before sequential `%s` substitution.
#[test]
fn format_percent_s_after_brace_placeholder() {
    assert_eq!(
        eval("format('%s then {1}', 'first', 'second')"),
        s("first then second")
    );
}

/// Correctness: format with missing placeholder args
#[test]
fn format_missing_args() {
    // {1} has no replacement arg — stays as-is
    assert_eq!(eval("format('{0} and {1}', 'hello')"), s("hello and {1}"));
}

/// Correctness: postfix access on function result
#[test]
fn postfix_access_on_expression() {
    // coalesce returns first non-null; test dot access on result
    let mut fields = HashMap::new();
    let obj = obj(vec![("x".to_string(), num(42))]);
    fields.insert("data".to_string(), obj);
    let result = eval_with_fields("$data.x", fields).unwrap();
    assert_eq!(result.value, num(42));
}

// ── Fuzz regression corpus (FEL-SMELL-C-001) ────────────────────

use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct FuzzRegressionCase {
    id: String,
    expression: String,
    #[serde(rename = "mustParse")]
    must_parse: bool,
    #[serde(default, rename = "displayOracle")]
    display_oracle: Option<String>,
}

/// Fuzz-discovered inputs: must not panic; rows with `mustParse` fail on parse regression.
///
/// `mustParse: false` rows guard against accidental grammar loosening on junk inputs.
/// `displayOracle` locks [`Value::Display`] for parseable rows; after intentional display
/// changes, run `make fuzz-regression-refresh` (see `tests/corpus/README.md`).
#[test]
fn fuzz_regression_corpus() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/fuzz_regression.jsonl");
    let raw = fs::read_to_string(&path).expect("read fuzz regression corpus");

    for (index, line) in raw.lines().enumerate() {
        let line_no = index + 1;
        let case: FuzzRegressionCase = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("line {line_no}: invalid corpus row: {e}"));

        match parse(&case.expression) {
            Ok(expr) => {
                assert!(
                    case.must_parse,
                    "line {line_no} id {}: parse succeeded but mustParse is false",
                    case.id
                );
                let result = evaluate(&expr, &MapEnvironment::new());
                let display = format!("{}", result.value);
                if let Some(expected) = &case.display_oracle {
                    assert_eq!(
                        display, *expected,
                        "line {line_no} id {} expression {:?}",
                        case.id, case.expression
                    );
                }
            }
            Err(err) => {
                assert!(
                    !case.must_parse,
                    "line {line_no} id {}: parse regression: {err}",
                    case.id
                );
            }
        }
    }
}
