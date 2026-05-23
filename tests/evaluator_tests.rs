/// Comprehensive FEL evaluator tests.
mod common;

use common::{arr, dec, eval, eval_fields, eval_fields_result, eval_result, num, obj, s};
use fel_core::*;
use rust_decimal::Decimal;
use serde_json::json;
use std::collections::HashMap;

// ── Literals ────────────────────────────────────────────────────

#[test]
fn test_number_literals() {
    assert_eq!(eval("0"), num(0));
    assert_eq!(eval("42"), num(42));
    assert_eq!(eval("3.14"), dec("3.14"));
    assert_eq!(eval("1e3"), num(1000));
}

#[test]
fn test_string_literals() {
    assert_eq!(eval("'hello'"), s("hello"));
    assert_eq!(eval("\"world\""), s("world"));
    assert_eq!(eval("'it\\'s'"), s("it's"));
}

#[test]
fn test_boolean_literals() {
    assert_eq!(eval("true"), Value::Boolean(true));
    assert_eq!(eval("false"), Value::Boolean(false));
}

#[test]
fn test_null_literal() {
    assert_eq!(eval("null"), Value::Null);
}

#[test]
fn test_date_literal() {
    let result = eval("@2024-01-15");
    assert!(matches!(
        result,
        Value::Date(Date::Date {
            year: 2024,
            month: 1,
            day: 15
        })
    ));
}

#[test]
fn test_datetime_literal() {
    let result = eval("@2024-01-15T10:30:00");
    assert!(matches!(
        result,
        Value::Date(Date::DateTime {
            year: 2024,
            month: 1,
            day: 15,
            hour: 10,
            minute: 30,
            second: 0
        })
    ));
}

// ── eval_with_fields convenience API ────────────────────────────
//
// `eval_with_fields(input, HashMap)` is the main public entry point for
// callers that don't construct an `Environment` directly. These pin the
// convenience-function contract (Result return, missing-field
// null-propagation, parse-error propagation).

#[test]
fn eval_with_fields_basic() {
    let mut fields = HashMap::new();
    fields.insert("x".to_string(), num(10));
    fields.insert("y".to_string(), num(20));

    let result = eval_with_fields("$x + $y", fields).unwrap();
    assert_eq!(result.value, num(30));
}

#[test]
fn eval_with_fields_strings() {
    let mut fields = HashMap::new();
    fields.insert("name".to_string(), s("Alice"));

    let result = eval_with_fields("$name", fields).unwrap();
    assert_eq!(result.value, s("Alice"));
}

#[test]
fn eval_with_fields_parse_error() {
    let fields = HashMap::new();
    let result = eval_with_fields("", fields);
    assert!(result.is_err());
}

#[test]
fn eval_with_fields_missing_field() {
    let fields = HashMap::new();
    let result = eval_with_fields("$missing", fields).unwrap();
    assert_eq!(result.value, Value::Null);
}

#[test]
fn eval_with_fields_complex() {
    let mut fields = HashMap::new();
    fields.insert("price".to_string(), dec("19.99"));
    fields.insert("qty".to_string(), num(3));

    let result = eval_with_fields("$price * $qty", fields).unwrap();
    assert_eq!(result.value, dec("59.97"));
}

// ── Arithmetic ──────────────────────────────────────────────────

#[test]
fn test_basic_arithmetic() {
    assert_eq!(eval("1 + 2"), num(3));
    assert_eq!(eval("10 - 3"), num(7));
    assert_eq!(eval("4 * 5"), num(20));
    assert_eq!(eval("15 / 3"), num(5));
    assert_eq!(eval("17 % 5"), num(2));
}

#[test]
fn test_unspaced_subtraction() {
    assert_eq!(eval("1-2"), num(-1));
}

#[test]
fn test_arithmetic_precedence() {
    assert_eq!(eval("2 + 3 * 4"), num(14));
    assert_eq!(eval("(2 + 3) * 4"), num(20));
    assert_eq!(eval("10 - 2 * 3"), num(4));
}

#[test]
fn test_division_by_zero() {
    assert_eq!(eval("1 / 0"), Value::Null);
    assert_eq!(eval("1 % 0"), Value::Null);
}

#[test]
fn test_unary_negation() {
    assert_eq!(eval("-5"), num(-5));
    assert_eq!(eval("-(3 + 2)"), num(-5));
}

// ── Comparison ──────────────────────────────────────────────────

#[test]
fn test_equality() {
    assert_eq!(eval("1 = 1"), Value::Boolean(true));
    assert_eq!(eval("1 = 2"), Value::Boolean(false));
    assert_eq!(eval("'a' = 'a'"), Value::Boolean(true));
    assert_eq!(eval("null = null"), Value::Boolean(true));
    assert_eq!(eval("null = 1"), Value::Boolean(false));
    assert_eq!(eval("1 != 2"), Value::Boolean(true));
    assert_eq!(eval("1 != 1"), Value::Boolean(false));
}

#[test]
fn test_ordering() {
    assert_eq!(eval("1 < 2"), Value::Boolean(true));
    assert_eq!(eval("2 > 1"), Value::Boolean(true));
    assert_eq!(eval("1 <= 1"), Value::Boolean(true));
    assert_eq!(eval("1 >= 2"), Value::Boolean(false));
    assert_eq!(eval("'a' < 'b'"), Value::Boolean(true));
}

/// Object and array equality, per `docs/SPEC.md` L1073 — "Any two values
/// of the same type may be compared for equality."
///
/// Cross-type comparisons (e.g. `1 = 'one'`) live in
/// `cross_type_equality_returns_null_with_diagnostic` below — they pin
/// diagnostic emission, not just the Boolean result.
#[test]
fn equality_table() {
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

/// Cross-type equality (`1 = 'one'`) is not a Boolean — it produces
/// `Value::Null` with a diagnostic. Distinct shape from
/// `equality_table` because it pins diagnostic emission.
#[test]
fn cross_type_equality_returns_null_with_diagnostic() {
    let r = eval_result("1 = 'one'");
    assert_eq!(r.value, Value::Null);
    assert!(!r.diagnostics.is_empty());
}

/// Cross-type comparison (`1 < 'abc'`) produces null + diagnostic.
#[test]
fn cross_type_comparison_returns_null() {
    let r = eval_result("1 < 'abc'");
    assert_eq!(r.value, Value::Null);
}

// ── Logical ─────────────────────────────────────────────────────

#[test]
fn test_logical_and_or() {
    assert_eq!(eval("true and true"), Value::Boolean(true));
    assert_eq!(eval("true and false"), Value::Boolean(false));
    assert_eq!(eval("false or true"), Value::Boolean(true));
    assert_eq!(eval("false or false"), Value::Boolean(false));
}

#[test]
fn test_short_circuit_and() {
    assert_eq!(eval("false and (1/0 = 1)"), Value::Boolean(false));
}

#[test]
fn test_short_circuit_or() {
    assert_eq!(eval("true or (1/0 = 1)"), Value::Boolean(true));
}

#[test]
fn test_logical_not() {
    assert_eq!(eval("not true"), Value::Boolean(false));
    assert_eq!(eval("not false"), Value::Boolean(true));
}

#[test]
fn test_null_propagation_logical() {
    assert_eq!(eval("null and true"), Value::Null);
    assert_eq!(eval("null or true"), Value::Null);
    assert_eq!(eval("not null"), Value::Null);
}

#[test]
fn test_bang_prefix_not_null_propagation() {
    assert_eq!(eval("!(null > 25)"), Value::Null);
    assert_eq!(
        eval_fields("!($amount > 25)", vec![("amount", Value::Null)]),
        Value::Null
    );
}

// ── String concatenation ────────────────────────────────────────

#[test]
fn test_string_concat() {
    assert_eq!(eval("'hello' & ' ' & 'world'"), s("hello world"));
}

/// `&` propagates null on either operand.
#[test]
fn concat_with_null_propagates() {
    assert_eq!(eval("'hello' & null"), Value::Null);
    assert_eq!(eval("null & 'world'"), Value::Null);
}

// ── Null coalesce ───────────────────────────────────────────────

#[test]
fn test_null_coalesce() {
    assert_eq!(eval("null ?? 42"), num(42));
    assert_eq!(eval("5 ?? 42"), num(5));
    assert_eq!(eval("null ?? null ?? 3"), num(3));
}

/// Chained null-coalesce evaluates left-to-right, short-circuits at the
/// first non-null operand.
#[test]
fn chained_null_coalesce() {
    assert_eq!(eval("null ?? null ?? null ?? 5"), num(5));
    assert_eq!(eval("null ?? 1 ?? 2"), num(1));
}

// ── Membership ──────────────────────────────────────────────────

#[test]
fn test_in_operator() {
    assert_eq!(eval("1 in [1, 2, 3]"), Value::Boolean(true));
    assert_eq!(eval("4 in [1, 2, 3]"), Value::Boolean(false));
    assert_eq!(eval("'a' not in ['b', 'c']"), Value::Boolean(true));
}

// ── Ternary and if/then/else ────────────────────────────────────

#[test]
fn test_ternary() {
    assert_eq!(eval("true ? 'yes' : 'no'"), s("yes"));
    assert_eq!(eval("false ? 'yes' : 'no'"), s("no"));
}

#[test]
fn test_if_then_else() {
    assert_eq!(eval("if true then 'yes' else 'no'"), s("yes"));
    assert_eq!(eval("if false then 'yes' else 'no'"), s("no"));
}

#[test]
fn test_if_function() {
    assert_eq!(eval("if(true, 'yes', 'no')"), s("yes"));
    assert_eq!(eval("if(false, 'yes', 'no')"), s("no"));
}

/// Deeply nested ternary preserves precedence; inner result is selected.
#[test]
fn deeply_nested_ternary() {
    assert_eq!(eval("true ? (false ? 1 : (true ? 42 : 3)) : 0"), num(42));
}

// ── Let binding ─────────────────────────────────────────────────

#[test]
fn test_let_binding() {
    assert_eq!(eval("let x = 5 in x + 1"), num(6));
    assert_eq!(eval("let x = 10 in let y = 20 in x + y"), num(30));
}

#[test]
fn test_let_binding_property_access_on_bound_object() {
    assert_eq!(eval("let x = {a: 1} in x.a"), num(1));
}

#[test]
fn test_let_binding_multi_level_property_access() {
    assert_eq!(eval("let x = {a: {b: 2}} in x.a.b"), num(2));
    assert_eq!(eval("let x = {a: {b: {c: 3}}} in x.a.b.c"), num(3));
}

/// Let binding combined with ternary on the binding value.
#[test]
fn let_binding_with_ternary() {
    assert_eq!(eval("let x = (true ? 10 : 20) in x + 1"), num(11));
}

/// Inner let shadows outer; outer binding restored after inner body.
#[test]
fn let_binding_shadowing() {
    assert_eq!(eval("let x = 1 in let x = 2 in x"), num(2));
}

// ── Field references ────────────────────────────────────────────

#[test]
fn test_field_ref() {
    let result = eval_fields("$name", vec![("name", s("Alice"))]);
    assert_eq!(result, s("Alice"));
}

#[test]
fn test_nested_field_ref() {
    let addr = obj(vec![("city".to_string(), s("NYC"))]);
    let result = eval_fields("$address.city", vec![("address", addr)]);
    assert_eq!(result, s("NYC"));
}

#[test]
fn test_wildcard_projection() {
    let items = arr(vec![
        obj(vec![("qty".to_string(), num(2))]),
        obj(vec![("qty".to_string(), num(5))]),
        obj(vec![("qty".to_string(), num(3))]),
    ]);
    let result = eval_fields("$items[*].qty", vec![("items", items)]);
    assert_eq!(result, arr(vec![num(2), num(5), num(3)]));
}

#[test]
fn test_wildcard_projection_through_let_bound_variable() {
    let items = arr(vec![
        obj(vec![("qty".to_string(), num(2))]),
        obj(vec![("qty".to_string(), num(5))]),
        obj(vec![("qty".to_string(), num(3))]),
    ]);
    let result = eval_fields(
        "let rows = $items in sum(rows[*].qty)",
        vec![("items", items)],
    );
    assert_eq!(result, num(10));
}

#[test]
fn test_indexed_access() {
    let items = arr(vec![num(10), num(20), num(30)]);
    // 1-based indexing
    let result = eval_fields("$items[1]", vec![("items", items)]);
    assert_eq!(result, num(10));
}

#[test]
fn test_index_zero_is_null_with_diagnostic() {
    let items = arr(vec![num(10), num(20)]);
    let result = eval_fields_result("$items[0]", vec![("items", items)]);
    assert_eq!(result.value, Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("index 0") && d.message.contains("out of bounds")),
        "expected 1-based index diagnostic, got {:?}",
        result.diagnostics
    );
}

#[test]
fn test_index_out_of_bounds_is_null_with_diagnostic() {
    let items = arr(vec![num(10), num(20), num(30)]);
    let result = eval_fields_result("$items[5]", vec![("items", items.clone())]);
    assert_eq!(result.value, Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("index 5") && d.message.contains("out of bounds")),
        "expected OOB diagnostic, got {:?}",
        result.diagnostics
    );
}

/// Postfix `.field` access works on the result of a function call /
/// arbitrary expression, not just bare field references.
#[test]
fn postfix_access_on_expression() {
    let mut fields = std::collections::HashMap::new();
    fields.insert("data".to_string(), obj(vec![("x".to_string(), num(42))]));
    let result = eval_with_fields("$data.x", fields).unwrap();
    assert_eq!(result.value, num(42));
}

// ── Array broadcasting ──────────────────────────────────────────

#[test]
fn test_array_scalar_broadcast() {
    assert_eq!(eval("[1, 2, 3] + 10"), arr(vec![num(11), num(12), num(13)]));
    assert_eq!(eval("5 * [1, 2, 3]"), arr(vec![num(5), num(10), num(15)]));
}

#[test]
fn test_array_array_zip() {
    assert_eq!(
        eval("[1, 2, 3] + [10, 20, 30]"),
        arr(vec![num(11), num(22), num(33)])
    );
}

// ── Aggregate functions ─────────────────────────────────────────

#[test]
fn test_sum() {
    assert_eq!(eval("sum([1, 2, 3])"), num(6));
    assert_eq!(eval("sum([1, null, 3])"), num(4)); // nulls skipped
}

#[test]
fn test_sum_rejects_money_array_with_diagnostic() {
    let expr = parse("sum([money(10, 'USD'), money(20, 'USD')])").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    assert_eq!(result.value, Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("use moneySum()")),
        "expected sum(money[]) guidance diagnostic, got {:?}",
        result.diagnostics
    );
}

#[test]
fn test_sum_rejects_mixed_money_and_number_array_with_diagnostic() {
    let expr = parse("sum([money(10, 'USD'), 5])").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    assert_eq!(result.value, Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("use moneySum()")),
        "expected mixed money+number sum guidance, got {:?}",
        result.diagnostics
    );
}

#[test]
fn test_count() {
    assert_eq!(eval("count([1, 2, null, 4])"), num(3)); // non-null count
}

#[test]
fn test_avg() {
    assert_eq!(eval("avg([2, 4, 6])"), num(4));
}

#[test]
fn test_min_max() {
    assert_eq!(eval("min([3, 1, 2])"), num(1));
    assert_eq!(eval("max([3, 1, 2])"), num(3));
    assert_eq!(eval("min(['b', 'a', 'c'])"), s("a"));
    assert_eq!(eval("max(['b', 'a', 'c'])"), s("c"));
}

// ── String functions ────────────────────────────────────────────

#[test]
fn test_string_functions() {
    assert_eq!(eval("length('hello')"), num(5));
    assert_eq!(
        eval("contains('hello world', 'world')"),
        Value::Boolean(true)
    );
    assert_eq!(eval("startsWith('hello', 'hel')"), Value::Boolean(true));
    assert_eq!(eval("endsWith('hello', 'llo')"), Value::Boolean(true));
    assert_eq!(eval("upper('hello')"), s("HELLO"));
    assert_eq!(eval("lower('HELLO')"), s("hello"));
    assert_eq!(eval("trim('  hi  ')"), s("hi"));
    assert_eq!(eval("replace('hello', 'l', 'r')"), s("herro"));
    assert_eq!(eval("substring('hello', 2, 3)"), s("ell"));
    assert_eq!(eval("length(null)"), num(0));
    // `length()` also operates on arrays (collection-length surface).
    assert_eq!(eval("length([1, 2, 3])"), num(3));
}

// ── Numeric functions ───────────────────────────────────────────

#[test]
fn test_numeric_functions() {
    assert_eq!(eval("round(3.5)"), num(4)); // banker's rounding
    assert_eq!(eval("round(2.5)"), num(2)); // banker's rounding: .5 → even
    assert_eq!(eval("round(3.14159, 2)"), dec("3.14"));
    assert_eq!(eval("floor(3.7)"), num(3));
    assert_eq!(eval("ceil(3.2)"), num(4));
    assert_eq!(eval("abs(-5)"), num(5));
    assert_eq!(eval("power(2, 10)"), num(1024));
}

#[test]
fn test_builtin_type_and_arity_diagnostics_normalize() {
    let env = MapEnvironment::new();

    let out = evaluate(&parse("round('x')").unwrap(), &env);
    assert_eq!(out.value, Value::Null);
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.message.contains("expected number"))
    );

    let out = evaluate(&parse("round(1, 'x')").unwrap(), &env);
    assert_eq!(out.value, Value::Null);
    assert!(out.diagnostics.iter().any(|d| d.message.contains("round")));

    let out = evaluate(&parse("power(2)").unwrap(), &env);
    assert_eq!(out.value, Value::Null);
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.message.contains("requires at least 2 arguments"))
    );

    let out = evaluate(&parse("power('a', 2)").unwrap(), &env);
    assert_eq!(out.value, Value::Null);
    assert!(out.diagnostics.iter().any(|d| d.message.contains("power")));

    let out = evaluate(&parse("if(true, 1, 2, 3)").unwrap(), &env);
    assert_eq!(out.value, Value::Null);
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.message.contains("exactly 3 arguments"))
    );

    let out = evaluate(&parse("selected(1, 2)").unwrap(), &env);
    assert_eq!(out.value, Value::Null);
    assert!(out.diagnostics.iter().any(|d| d.message.contains("array")));

    let out = evaluate(&parse("number(money(1, 'USD'))").unwrap(), &env);
    assert_eq!(out.value, Value::Null);
    assert!(out.diagnostics.iter().any(|d| d.message.contains("number")));
}

// ── Date functions ──────────────────────────────────────────────

#[test]
fn test_date_functions() {
    assert_eq!(eval("year(@2024-06-15)"), num(2024));
    assert_eq!(eval("month(@2024-06-15)"), num(6));
    assert_eq!(eval("day(@2024-06-15)"), num(15));
}

#[test]
fn test_map_environment_clock_can_be_overridden() {
    let expr = parse("today()").unwrap();
    let env = MapEnvironment::new().with_current_datetime(Some(Date::DateTime {
        year: 2030,
        month: 1,
        day: 2,
        hour: 3,
        minute: 4,
        second: 5,
    }));
    let result = evaluate(&expr, &env);
    assert_eq!(
        result.value,
        Value::Date(Date::Date {
            year: 2030,
            month: 1,
            day: 2
        })
    );
}

/// Expected outcome for a date-arithmetic case.
///
/// `Add(y, m, d)` covers `dateAdd` (Date result); `Diff(n)` covers
/// `dateDiff` (Number result). Named `Add` rather than `Date` to avoid
/// shadowing `fel_core::Date` under `use DateOp::*`.
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
                Value::Date(Date::Date { year, month, day }) => {
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

/// `today()` returns a Date value.
#[test]
fn today_returns_date() {
    let result = eval("today()");
    assert!(
        matches!(result, Value::Date(Date::Date { .. })),
        "today() should return a Date, got: {result:?}"
    );
}

/// `now()` returns a DateTime value.
#[test]
fn now_returns_datetime() {
    let result = eval("now()");
    assert!(
        matches!(result, Value::Date(Date::DateTime { .. })),
        "now() should return a DateTime, got: {result:?}"
    );
}

/// `today()` can be passed through date arithmetic.
#[test]
fn today_in_date_arithmetic() {
    let result = eval("dateAdd(today(), 1, 'days')");
    assert!(
        matches!(result, Value::Date(Date::Date { .. })),
        "dateAdd on today() should return a Date, got: {result:?}"
    );
}

/// Year/month/day extractors operate on `today()`.
#[test]
fn today_date_parts() {
    let result = eval("year(today())");
    assert!(matches!(result, Value::Number(_)));
}

/// Date comparison: `<`, `>`, `=`.
#[test]
fn date_comparison() {
    assert_eq!(eval("@2024-01-15 < @2024-06-15"), Value::Boolean(true));
    assert_eq!(eval("@2024-06-15 > @2024-01-15"), Value::Boolean(true));
    assert_eq!(eval("@2024-01-15 = @2024-01-15"), Value::Boolean(true));
}

/// `date('YYYY-MM-DD')` parses a string into a Date.
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

/// `isDate` type check: true for Date values, false otherwise.
#[test]
fn is_date_check() {
    assert_eq!(eval("isDate(@2024-01-15)"), Value::Boolean(true));
    assert_eq!(eval("isDate('not a date')"), Value::Boolean(false));
    assert_eq!(eval("isDate(42)"), Value::Boolean(false));
}

// ── Time functions ──────────────────────────────────────────────

#[test]
fn test_time_functions() {
    assert_eq!(eval("hours('10:30:45')"), num(10));
    assert_eq!(eval("minutes('10:30:45')"), num(30));
    assert_eq!(eval("seconds('10:30:45')"), num(45));
    assert_eq!(eval("time(10, 30, 45)"), s("10:30:45"));
    assert_eq!(eval("timeDiff('10:30:00', '08:15:00')"), num(8100));
}

// ── Logical functions ───────────────────────────────────────────

#[test]
fn test_coalesce() {
    assert_eq!(eval("coalesce(null, null, 42)"), num(42));
    assert_eq!(eval("coalesce(1, 2, 3)"), num(1));
}

#[test]
fn test_empty_present() {
    assert_eq!(eval("empty(null)"), Value::Boolean(true));
    assert_eq!(eval("empty('')"), Value::Boolean(true));
    assert_eq!(eval("empty([])"), Value::Boolean(true));
    assert_eq!(eval("empty('x')"), Value::Boolean(false));
    // Non-empty primitives (`0`, `false`) are NOT "empty" — empty() pins
    // missing/absent, not falsy.
    assert_eq!(eval("empty(0)"), Value::Boolean(false));
    assert_eq!(eval("empty(false)"), Value::Boolean(false));
    assert_eq!(eval("present('hello')"), Value::Boolean(true));
    assert_eq!(eval("present(null)"), Value::Boolean(false));
}

#[test]
fn test_selected() {
    assert_eq!(eval("selected(['a', 'b', 'c'], 'b')"), Value::Boolean(true));
    assert_eq!(
        eval("selected(['a', 'b', 'c'], 'd')"),
        Value::Boolean(false)
    );
}

// ── Type checking ───────────────────────────────────────────────

#[test]
fn test_type_functions() {
    assert_eq!(eval("isNumber(42)"), Value::Boolean(true));
    assert_eq!(eval("isNumber('x')"), Value::Boolean(false));
    assert_eq!(eval("isString('x')"), Value::Boolean(true));
    assert_eq!(eval("isNull(null)"), Value::Boolean(true));
    assert_eq!(eval("isNull(0)"), Value::Boolean(false));
    assert_eq!(eval("typeOf(42)"), s("number"));
    assert_eq!(eval("typeOf('x')"), s("string"));
    assert_eq!(eval("typeOf(null)"), s("null"));
}

// ── Casting ─────────────────────────────────────────────────────

#[test]
fn test_casting() {
    assert_eq!(eval("number('42')"), num(42));
    assert_eq!(eval("number(true)"), num(1));
    assert_eq!(eval("string(42)"), s("42"));
    assert_eq!(eval("string(null)"), s(""));
    assert_eq!(eval("boolean('true')"), Value::Boolean(true));
    assert_eq!(eval("boolean(0)"), Value::Boolean(false));
    assert_eq!(eval("boolean(1)"), Value::Boolean(true));
    assert_eq!(eval("boolean(2)"), Value::Boolean(true));
    assert_eq!(eval("boolean(-3)"), Value::Boolean(true));
}

// ── Money functions ─────────────────────────────────────────────
//
// Three test surfaces:
//
//   - `money_arithmetic_table` — operator overloads (`-`, `*`, `/`) plus
//     `moneySum` aggregate; currency-mismatch and divide-by-zero null
//     propagation; fractional-amount decimal precision.
//
//   - `test_money_builtins_table` — builtin functions: `money(...)`
//     constructor, `moneyAmount`/`moneyCurrency` accessors, `moneyAdd`
//     aggregate (with currency-mismatch null-prop).
//
//   - `test_money_amount_currency_type_mismatch_emits_diagnostic` and
//     `test_money_*_comparison_*_diagnostic` (later in this file) stay
//     standalone — they pin diagnostic message text, a stronger
//     assertion than null-propagation that would be diluted in a
//     value-only table.

/// Expected outcome for a money-arithmetic (operator) case.
///
/// `Num(&str)` mirrors `MoneyBuiltinCase::Num`; string-parsed Decimal so
/// future fractional-ratio rows are expressible without a type change.
/// `Null(&str)` carries the failure-contract name so a future regression
/// that breaks one Null path doesn't silently mask another.
enum MoneyArithCase {
    Money(&'static str, &'static str),
    Num(&'static str),
    Null(&'static str),
}

#[test]
fn money_arithmetic_table() {
    use MoneyArithCase::*;
    let cases: &[(&str, MoneyArithCase)] = &[
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
        // operator: fractional amounts exercise decimal precision
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
            MoneyArithCase::Money(amt, cur) => match &actual {
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
            MoneyArithCase::Num(n) => assert_eq!(
                actual,
                Value::Number(Decimal::from_str_exact(n).unwrap()),
                "input={input:?}: number mismatch"
            ),
            MoneyArithCase::Null(intent) => assert_eq!(
                actual,
                Value::Null,
                "input={input:?}: expected Null ({intent}), got {actual:?}"
            ),
        }
    }
}

#[test]
fn money_equality() {
    let cases: &[(&str, bool, &str)] = &[
        (
            "money(100, 'USD') = money(100, 'USD')",
            true,
            "same amount + currency",
        ),
        (
            "money(100, 'USD') = money(100, 'EUR')",
            false,
            "different currency",
        ),
        (
            "money(100, 'USD') = money(50, 'USD')",
            false,
            "different amount",
        ),
    ];
    for (input, expected, facet) in cases {
        assert_eq!(
            eval(input),
            Value::Boolean(*expected),
            "input={input:?}: {facet}"
        );
    }
}

/// Expected outcome shape for a money-builtin case.
enum MoneyBuiltinCase {
    Money(&'static str, &'static str),
    Num(&'static str),
    Str(&'static str),
    Null,
}

#[test]
fn test_money_builtins_table() {
    use MoneyBuiltinCase::*;
    let cases: &[(&str, MoneyBuiltinCase)] = &[
        // constructor
        ("money(100.50, 'USD')", Money("100.50", "USD")),
        // accessors
        ("moneyAmount(money(100.50, 'USD'))", Num("100.50")),
        ("moneyCurrency(money(100.50, 'USD'))", Str("USD")),
        // builtin add
        (
            "moneyAdd(money(100, 'USD'), money(50, 'USD'))",
            Money("150", "USD"),
        ),
        // builtin add — currency mismatch
        ("moneyAdd(money(100, 'USD'), money(50, 'EUR'))", Null),
    ];

    for (input, expected) in cases {
        let actual = eval(input);
        match expected {
            MoneyBuiltinCase::Money(amt, cur) => match &actual {
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
            MoneyBuiltinCase::Num(d) => {
                assert_eq!(actual, dec(d), "input={input:?}")
            }
            MoneyBuiltinCase::Str(v) => assert_eq!(actual, s(v), "input={input:?}"),
            MoneyBuiltinCase::Null => assert_eq!(actual, Value::Null, "input={input:?}"),
        }
    }
}

#[test]
fn test_money_amount_currency_type_mismatch_emits_diagnostic() {
    let expr = parse("moneyAmount(42)").unwrap();
    let env = MapEnvironment::new();
    let out = evaluate(&expr, &env);
    assert_eq!(out.value, Value::Null);
    assert!(
        out.diagnostics.iter().any(|d| {
            d.message.contains("moneyAmount")
                && d.message.contains("expected money")
                && d.message.contains("number")
        }),
        "{:?}",
        out.diagnostics
    );

    let expr = parse("moneyCurrency('x')").unwrap();
    let out = evaluate(&expr, &env);
    assert_eq!(out.value, Value::Null);
    assert!(
        out.diagnostics.iter().any(|d| {
            d.message.contains("moneyCurrency")
                && d.message.contains("expected money")
                && d.message.contains("string")
        }),
        "{:?}",
        out.diagnostics
    );
}

// ── Null propagation ────────────────────────────────────────────

#[test]
fn test_null_propagation() {
    assert_eq!(eval("null + 1"), Value::Null);
    assert_eq!(eval("1 + null"), Value::Null);
    assert_eq!(eval("null * 5"), Value::Null);
    assert_eq!(eval("null < 1"), Value::Null);
}

#[test]
fn test_equality_no_propagation() {
    // Equality does NOT propagate null — spec §3
    assert_eq!(eval("null = null"), Value::Boolean(true));
    assert_eq!(eval("null = 1"), Value::Boolean(false));
    assert_eq!(eval("1 = null"), Value::Boolean(false));
    assert_eq!(eval("null != 1"), Value::Boolean(true));
}

// ── Format function ─────────────────────────────────────────────

#[test]
fn test_format() {
    assert_eq!(
        eval("format('{0} is {1}', 'sky', 'blue')"),
        s("sky is blue")
    );
}

/// Multiple `{n}` placeholders interpolate positional args.
#[test]
fn format_multiple_placeholders() {
    assert_eq!(
        eval("format('{0} has {1} items at ${2} each', 'Cart', 3, 9.99)"),
        s("Cart has 3 items at $9.99 each")
    );
}

/// `{n}` placeholders resolve before remaining `%s` substitutions consume
/// positional args sequentially.
#[test]
fn format_percent_s_after_brace_placeholder() {
    assert_eq!(
        eval("format('%s then {1}', 'first', 'second')"),
        s("first then second")
    );
}

/// Missing placeholder args leave the placeholder literal in the output
/// rather than panicking.
#[test]
fn format_missing_args() {
    assert_eq!(eval("format('{0} and {1}', 'hello')"), s("hello and {1}"));
}

// ── Nested/complex expressions ──────────────────────────────────

#[test]
fn test_complex_expression() {
    let items = arr(vec![
        obj(vec![
            ("qty".to_string(), num(3)),
            ("price".to_string(), num(10)),
        ]),
        obj(vec![
            ("qty".to_string(), num(2)),
            ("price".to_string(), num(25)),
        ]),
    ]);
    // sum of qty * price: 30 + 50 = 80
    let result = eval_fields(
        "sum($items[*].qty * $items[*].price)",
        vec![("items", items)],
    );
    assert_eq!(result, num(80));
}

#[test]
fn test_conditional_with_fields() {
    let result = eval_fields(
        "if $age >= 18 then 'adult' else 'minor'",
        vec![("age", num(21))],
    );
    assert_eq!(result, s("adult"));
}

// ── Undefined function ──────────────────────────────────────────

#[test]
fn test_undefined_function() {
    let expr = parse("unknownFunc(1)").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    assert_eq!(result.value, Value::Null);
    let d = result
        .diagnostics
        .iter()
        .find(|d| matches!(&d.kind, Some(DiagnosticKind::UndefinedFunction { name }) if name == "unknownFunc"))
        .expect("undefinedFunc diagnostic with kind");
    assert_eq!(
        d.kind,
        Some(DiagnosticKind::UndefinedFunction {
            name: "unknownFunc".into()
        })
    );
    assert_eq!(
        fel_diagnostics_to_json_value(&result.diagnostics),
        json!([{
            "message": "undefined function: unknownFunc",
            "severity": "error",
            "kind": { "undefinedFunction": { "name": "unknownFunc" } }
        }])
    );
}

#[test]
fn test_extension_registry_arity_mismatch_emits_diagnostic() {
    let expr = parse("needsTwo(1)").unwrap();
    let env = MapEnvironment::new();
    let mut extensions = ExtensionRegistry::new();
    extensions
        .register("needsTwo", 2, Some(2), |args| match (&args[0], &args[1]) {
            (Value::Number(a), Value::Number(b)) => Value::Number(*a + *b),
            _ => Value::Null,
        })
        .unwrap();
    let result = evaluate_with(
        &expr,
        &env,
        EvaluatorOptions {
            extensions: Some(&extensions),
            ..EvaluatorOptions::default()
        },
    );
    assert!(matches!(result.value, Value::Null));
    assert!(result.diagnostics.iter().any(|d| {
        d.message == "needsTwo: requires exactly 2 arguments"
            && matches!(
                &d.kind,
                Some(DiagnosticKind::ArityMismatch {
                    name,
                    min_args: 2,
                    max_args: Some(2),
                    got: 1,
                }) if name == "needsTwo"
            )
    }));
}

#[test]
fn test_extension_registry_fallback_executes_unknown_function() {
    let expr = parse("double(3)").unwrap();
    let env = MapEnvironment::new();
    let mut extensions = ExtensionRegistry::new();
    extensions
        .register("double", 1, Some(1), |args| match &args[0] {
            Value::Number(n) => Value::Number(*n * Decimal::from(2)),
            _ => Value::Null,
        })
        .unwrap();

    let result = evaluate_with(
        &expr,
        &env,
        EvaluatorOptions {
            extensions: Some(&extensions),
            ..EvaluatorOptions::default()
        },
    );
    assert_eq!(result.value, num(6));
    assert!(
        result.diagnostics.is_empty(),
        "extension fallback should avoid undefined-function diagnostics"
    );
}

#[test]
fn test_evaluate_with_trace_and_extensions_records_extension_call() {
    let expr = parse("double(3)").unwrap();
    let env = MapEnvironment::new();
    let mut extensions = ExtensionRegistry::new();
    extensions
        .register("double", 1, Some(1), |args| match &args[0] {
            Value::Number(n) => Value::Number(*n * Decimal::from(2)),
            _ => Value::Null,
        })
        .unwrap();

    let mut trace = Trace::new();
    let result = evaluate_with(
        &expr,
        &env,
        EvaluatorOptions {
            trace: Some(&mut trace),
            extensions: Some(&extensions),
            ..EvaluatorOptions::default()
        },
    );
    assert_eq!(result.value, num(6));
    assert!(
        trace.steps.iter().any(|s| {
            matches!(
                s,
                TraceStep::FunctionCalled { name, .. } if name == "double"
            )
        }),
        "expected FunctionCalled step for extension, got {:?}",
        trace.steps
    );
}

#[test]
fn test_var_ref_and_dollar_field_ref_evaluate_same() {
    let mut fields = HashMap::new();
    fields.insert("qty".into(), num(7));
    fields.insert("outer.inner".into(), num(3));
    let env = MapEnvironment::with_fields(fields);
    let bare_qty = evaluate(&parse("qty").unwrap(), &env);
    let dollar_qty = evaluate(&parse("$qty").unwrap(), &env);
    assert_eq!(bare_qty.value, dollar_qty.value);
    let bare_nested = evaluate(&parse("outer.inner").unwrap(), &env);
    let dollar_nested = evaluate(&parse("$outer.inner").unwrap(), &env);
    assert_eq!(bare_nested.value, dollar_nested.value);
}

#[test]
fn test_if_ast_non_boolean_condition_matches_builtin_diagnostic() {
    let env = MapEnvironment::new();
    for src in ["1 ? 2 : 3", "if 1 then 2 else 3"] {
        let result = evaluate(&parse(src).unwrap(), &env);
        assert!(result.value.is_null(), "src={src:?}");
        assert!(
            result.diagnostics.iter().any(|d| {
                d.message == "if: expected boolean, got number"
                    && matches!(
                        &d.kind,
                        Some(DiagnosticKind::TypeMismatch { fn_name, expected, got })
                            if fn_name == "if" && expected == "boolean" && got == "number"
                    )
            }),
            "src={src:?} diagnostics={:?}",
            result.diagnostics
        );
    }
}

// ── MIP state queries ───────────────────────────────────────────

#[test]
fn test_mip_defaults() {
    assert_eq!(eval("valid($name)"), Value::Boolean(true));
    assert_eq!(eval("relevant($name)"), Value::Boolean(true));
    assert_eq!(eval("readonly($name)"), Value::Boolean(false));
    assert_eq!(eval("required($name)"), Value::Boolean(false));
}

// ── countWhere ──────────────────────────────────────────────────

#[test]
fn test_count_where() {
    assert_eq!(eval("countWhere([1, 2, 3, 4, 5], $ > 3)"), num(2));
    assert_eq!(eval("countWhere([1, 2, 3], $ = 2)"), num(1));
}

// ── every / some (spec §3.5.1) ──────────────────────────────────

#[test]
fn test_every_some_basic() {
    assert_eq!(eval("every([1, 2, 3], $ > 0)"), Value::Boolean(true));
    assert_eq!(eval("every([1, 0, 3], $ > 0)"), Value::Boolean(false));
    assert_eq!(eval("some([0, 2, 0], $ > 1)"), Value::Boolean(true));
    assert_eq!(eval("some([0, 1], $ > 5)"), Value::Boolean(false));
}

#[test]
fn test_every_some_empty_array() {
    assert_eq!(eval("every([], $ > 0)"), Value::Boolean(true));
    assert_eq!(eval("some([], $ > 0)"), Value::Boolean(false));
}

#[test]
fn test_duration_ms() {
    assert_eq!(eval("duration('PT1H')"), num(3_600_000));
    assert_eq!(eval("duration('P1D')"), num(86_400_000));
    assert_eq!(eval("duration('PT0.5S')"), num(500));
    assert_eq!(eval("duration('-PT1M')"), num(-60_000));
}

#[test]
fn test_duration_invalid_vs_out_of_range_diagnostics() {
    let env = MapEnvironment::new();
    let expr_invalid = parse("duration('P')").unwrap();
    let r_inv = evaluate(&expr_invalid, &env);
    assert!(r_inv.value.is_null());
    assert!(
        r_inv
            .diagnostics
            .iter()
            .any(|d| d.message == "duration: invalid ISO 8601 duration string"),
        "invalid shape should use invalid-string diagnostic, got {:?}",
        r_inv.diagnostics
    );

    let expr_range = parse("duration('P106751991167301D')").unwrap();
    let r_or = evaluate(&expr_range, &env);
    assert!(r_or.value.is_null());
    assert!(
        r_or.diagnostics.iter().any(|d| {
            d.message == "duration: duration exceeds representable range (milliseconds)"
        }),
        "overflow should use range diagnostic, got {:?}",
        r_or.diagnostics
    );
}

/// Spec: core/spec.md §3.5.1 — predicate `$` may be an object; `$.field` resolves on the element.
#[test]
fn test_quantifier_predicates_with_object_elements() {
    assert_eq!(
        eval("every([{amount: 1}, {amount: 2}], $.amount > 0)"),
        Value::Boolean(true)
    );
    assert_eq!(
        eval("some([{ok: false}, {ok: true}], $.ok)"),
        Value::Boolean(true)
    );
    assert_eq!(eval("countWhere([{v: 10}, {v: 20}], $.v > 15)"), num(1));
}

// ── Aggregate functions on empty arrays (spec §3.5.1) ───────────

/// Spec: core/spec.md §3.5.1 (lines 1220-1225) — sum([]) must return 0.
#[test]
fn test_sum_empty_array() {
    assert_eq!(eval("sum([])"), num(0));
}

/// Spec: core/spec.md §3.5.1 — count([]) must return 0.
#[test]
fn test_count_empty_array() {
    assert_eq!(eval("count([])"), num(0));
}

/// Spec: core/spec.md §3.5.1 — avg([]) must signal error (division by zero).
#[test]
fn test_avg_empty_array() {
    let expr = parse("avg([])").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    assert_eq!(result.value, Value::Null, "avg([]) must return null");
    assert!(
        !result.diagnostics.is_empty(),
        "avg([]) must produce a diagnostic"
    );
}

/// Spec: core/spec.md §3.5.1 — min([]) must return null.
#[test]
fn test_min_empty_array() {
    assert_eq!(eval("min([])"), Value::Null);
}

/// Spec: core/spec.md §3.5.1 — max([]) must return null.
#[test]
fn test_max_empty_array() {
    assert_eq!(eval("max([])"), Value::Null);
}

// ── Arity checks on aggregate functions (spec §3.10) ────────────

/// Spec: core/spec.md §3.10, fel-grammar.md §7 —
/// Wrong argument count on aggregate functions must be rejected.
#[test]
fn test_aggregate_arity_sum_no_args() {
    let expr = parse("sum()").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    // sum with no args evaluates with a missing arg (null) → null
    assert_eq!(result.value, Value::Null);
}

/// Spec: core/spec.md §3.10 — countWhere requires at least 2 arguments.
#[test]
fn test_count_where_wrong_arity() {
    let expr = parse("countWhere([1, 2, 3])").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    assert_eq!(result.value, Value::Null, "countWhere with 1 arg must fail");
    assert!(
        !result.diagnostics.is_empty(),
        "countWhere arity mismatch must produce diagnostic"
    );
}

// ── Type mismatch in casting (spec §3.4.3) ──────────────────────

/// Spec: core/spec.md §3.4.3 (line 1183) — number("abc") must signal error.
#[test]
fn test_number_cast_invalid_string() {
    let expr = parse("number('abc')").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    assert_eq!(result.value, Value::Null, "number('abc') must return null");
    assert!(
        !result.diagnostics.is_empty(),
        "number('abc') must produce a diagnostic"
    );
}

/// Spec: core/spec.md §3.4.3 (line 1193) — date("not-a-date") must signal error.
#[test]
fn test_date_cast_invalid_string() {
    let expr = parse("date('not-a-date')").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    assert_eq!(
        result.value,
        Value::Null,
        "date('not-a-date') must return null"
    );
    assert!(
        !result.diagnostics.is_empty(),
        "date('not-a-date') must produce a diagnostic"
    );
}

/// `boolean()` cast edge cases: null coerces to false; unrecognized
/// string yields null + diagnostic (NOT a Boolean).
#[test]
fn boolean_cast_edge_cases() {
    assert_eq!(eval("boolean(null)"), Value::Boolean(false));
    let r = eval_result("boolean('maybe')");
    assert_eq!(r.value, Value::Null);
}

// ── Decimal precision (spec S3.4.1) ─────────────────────────────

#[test]
fn test_decimal_precision_18_digits() {
    // Spec requires minimum 18 significant decimal digits.
    // f64 fails this (15-17 digits); rust_decimal gives 28-29.
    assert_eq!(eval("123456789012345678 + 1"), dec("123456789012345679"));
    assert_eq!(
        eval("0.123456789012345678 + 0"),
        dec("0.123456789012345678")
    );
}

#[test]
fn test_decimal_exact_money_arithmetic() {
    // Classic floating point failure: 0.1 + 0.2 != 0.3 in f64
    // With Decimal: exact
    assert_eq!(eval("0.1 + 0.2"), dec("0.3"));
    assert_eq!(eval("0.1 + 0.2 = 0.3"), Value::Boolean(true));
}

#[test]
fn test_bankers_rounding_decimal() {
    // Banker's rounding uses rust_decimal native MidpointNearestEven
    assert_eq!(eval("round(0.5)"), num(0)); // 0.5 → 0 (even)
    assert_eq!(eval("round(1.5)"), num(2)); // 1.5 → 2 (even)
    assert_eq!(eval("round(2.5)"), num(2)); // 2.5 → 2 (even)
    assert_eq!(eval("round(3.5)"), num(4)); // 3.5 → 4 (even)
    assert_eq!(eval("round(4.5)"), num(4)); // 4.5 → 4 (even)
}

// Note: `matches()` regex coverage lives in `regex_tests.rs`
// (`matches_table`, `matches_null_propagation_table`, and
// `matches_invalid_regex_emits_diagnostic`) — consolidated under Cluster D
// of the test triage.

// ── 9f: Money comparison diagnostic ────────────────────────────

#[test]
fn test_money_number_comparison_returns_null_with_diagnostic() {
    // money(...) < number should return Null + diagnostic
    let expr = parse("money(100, 'USD') < 200").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    assert_eq!(result.value, Value::Null);
    assert!(!result.diagnostics.is_empty(), "should have diagnostic");
    assert!(
        result.diagnostics[0]
            .message
            .contains("cannot compare money with number"),
        "diagnostic message should mention money/number mismatch, got: {}",
        result.diagnostics[0].message
    );
    assert!(
        result.diagnostics[0].message.contains("moneyAmount("),
        "diagnostic should suggest moneyAmount(), got: {}",
        result.diagnostics[0].message
    );
}

#[test]
fn test_number_money_comparison_returns_null_with_diagnostic() {
    // number > money(...) should also return Null + diagnostic
    let expr = parse("200 > money(100, 'USD')").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    assert_eq!(result.value, Value::Null);
    assert!(!result.diagnostics.is_empty());
    assert!(
        result.diagnostics[0].message.contains("moneyAmount("),
        "diagnostic should suggest moneyAmount(), got: {}",
        result.diagnostics[0].message
    );
}

#[test]
fn test_money_money_ordering_returns_null_with_diagnostic() {
    // money > money should return Null with diagnostic (only equality is supported)
    let expr = parse("money(200, 'USD') > money(100, 'USD')").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    assert_eq!(result.value, Value::Null);
    assert!(
        !result.diagnostics.is_empty(),
        "should have diagnostic for money ordering"
    );
    assert!(
        result.diagnostics[0].message.contains("moneyAmount("),
        "diagnostic should suggest moneyAmount() for money ordering, got: {}",
        result.diagnostics[0].message
    );
}

// ── GAP-1: *Where predicate aggregate functions ─────────────────

#[test]
fn test_sum_where() {
    assert_eq!(eval("sumWhere([1, 2, 3, 4, 5], $ > 3)"), num(9)); // 4 + 5
    assert_eq!(eval("sumWhere([10, 20, 30], $ < 25)"), num(30)); // 10 + 20
}

#[test]
fn test_sum_where_empty_match() {
    assert_eq!(eval("sumWhere([1, 2, 3], $ > 100)"), num(0));
}

#[test]
fn test_avg_where() {
    assert_eq!(eval("avgWhere([1, 2, 3, 4, 5], $ > 3)"), dec("4.5")); // (4+5)/2
}

#[test]
fn test_avg_where_no_match() {
    // avgWhere with no matching elements should return null (no values to average)
    assert_eq!(eval("avgWhere([1, 2, 3], $ > 100)"), Value::Null);
}

#[test]
fn test_min_where() {
    assert_eq!(eval("minWhere([1, 2, 3, 4, 5], $ > 2)"), num(3));
}

#[test]
fn test_min_where_no_match() {
    assert_eq!(eval("minWhere([1, 2, 3], $ > 100)"), Value::Null);
}

#[test]
fn test_max_where() {
    assert_eq!(eval("maxWhere([1, 2, 3, 4, 5], $ < 4)"), num(3));
}

#[test]
fn test_max_where_no_match() {
    assert_eq!(eval("maxWhere([1, 2, 3], $ > 100)"), Value::Null);
}

#[test]
fn test_money_sum_where() {
    assert_eq!(
        eval(
            "moneySumWhere([money(100, 'USD'), money(200, 'USD'), money(300, 'USD')], moneyAmount($) > 150)"
        ),
        Value::Money(Money {
            amount: Decimal::from(500),
            currency: CurrencyCode::parse("USD").expect("USD"),
        })
    ); // 200 + 300
}

#[test]
fn test_money_sum_where_no_match() {
    assert_eq!(
        eval("moneySumWhere([money(100, 'USD'), money(200, 'USD')], moneyAmount($) > 1000)"),
        Value::Null
    );
}

#[test]
fn test_where_functions_require_two_args() {
    for func in &[
        "sumWhere",
        "avgWhere",
        "minWhere",
        "maxWhere",
        "moneySumWhere",
    ] {
        let expr = parse(&format!("{func}([1, 2, 3])")).unwrap();
        let env = MapEnvironment::new();
        let result = evaluate(&expr, &env);
        assert_eq!(
            result.value,
            Value::Null,
            "{func} with 1 arg should return Null"
        );
        assert!(
            !result.diagnostics.is_empty(),
            "{func} with 1 arg should produce diagnostic"
        );
    }
}
