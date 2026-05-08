//! Emits deterministic conformance fixtures for cross-runtime differential testing.
//!
//! Usage:
//!   cargo run --features proptest-strategies --bin emit-conformance-fixtures -- [N] > fel-conformance.jsonl
//!
//! Without `[N]` (or `[N]` ≧ base corpus size), emits the full semantic-rules corpus.
//! When `[N]` > corpus size, pads with proptest-generated random expressions.
//!
//! Each line is a JSON object:
//!   {"expression": "<FEL source>", "environment": {"field": <value>}, "expectedValue": <JSON>, "expectedDiagnosticKinds": ["UndefinedFunction", ...]}

use std::collections::BTreeMap;
use std::io::Write;

use fel_core::{builtin_function_catalog, evaluate, fel_to_json, json_object_to_field_map, MapEnvironment, parse};
use fel_core::error::DiagnosticKind;
use fel_core::extensions::{BuiltinFunctionCatalogEntry, Example};
use proptest::strategy::Strategy;
use serde::Serialize;

// ── Output structure ─────────────────────────────────────────────────────

#[derive(Serialize)]
struct ConformanceFixture {
    expression: String,
    environment: BTreeMap<String, serde_json::Value>,
    #[serde(rename = "expectedValue")]
    expected_value: serde_json::Value,
    #[serde(rename = "expectedDiagnosticKinds")]
    expected_diagnostic_kinds: Vec<String>,
}

fn diagnostic_kind_name(kind: &DiagnosticKind) -> &'static str {
    match kind {
        DiagnosticKind::UndefinedFunction { .. } => "UndefinedFunction",
        DiagnosticKind::TypeMismatch { .. } => "TypeMismatch",
    }
}

// ── Helper: evaluate a FEL expression, capture result + diagnostic kinds ─

fn eval(expression: &str) -> ConformanceFixture {
    eval_with_env(expression, serde_json::Value::Object(Default::default()))
}

fn eval_with_env(expression: &str, env_json: serde_json::Value) -> ConformanceFixture {
    let expr = parse(expression)
        .unwrap_or_else(|e| panic!("parse failed: {expression}: {e}"));
    let fields = json_object_to_field_map(&env_json);
    let env = MapEnvironment::with_fields(fields);
    let result = evaluate(&expr, &env);
    let env_map: BTreeMap<String, serde_json::Value> = match env_json {
        serde_json::Value::Object(m) => m.into_iter().collect(),
        _ => BTreeMap::new(),
    };
    ConformanceFixture {
        expression: expression.to_string(),
        environment: env_map,
        expected_value: fel_to_json(&result.value),
        expected_diagnostic_kinds: result
            .diagnostics
            .iter()
            .filter_map(|d| d.kind.as_ref().map(|k| diagnostic_kind_name(k).to_string()))
            .collect(),
    }
}

fn sort_keys(val: serde_json::Value) -> serde_json::Value {
    match val {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().map(|(k, v)| (k, sort_keys(v))).collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            serde_json::Value::Object(entries.into_iter().collect())
        }
        other => other,
    }
}

fn emit(w: &mut impl Write, fixture: &ConformanceFixture) {
    let val = serde_json::to_value(fixture).unwrap();
    let json = serde_json::to_string(&sort_keys(val)).unwrap();
    writeln!(w, "{json}").unwrap();
}

// ── Fixture generators ───────────────────────────────────────────────────

fn catalog_example_fixtures() -> Vec<ConformanceFixture> {
    let catalog = builtin_function_catalog();
    let mut out = Vec::new();
    for entry in catalog {
        for ex in entry.examples {
            if let Some(f) = catalog_example_to_fixture(entry, ex) {
                out.push(f);
            }
        }
    }
    out
}

fn catalog_example_to_fixture(entry: &BuiltinFunctionCatalogEntry, ex: &Example) -> Option<ConformanceFixture> {
    if ex.expression.contains('$') {
        return None;
    }
    let expr = parse(&ex.expression).ok()?;
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    let kinds: Vec<String> = result
        .diagnostics
        .iter()
        .filter_map(|d| d.kind.as_ref().map(|k| diagnostic_kind_name(k).to_string()))
        .collect();
    Some(ConformanceFixture {
        expression: ex.expression.to_string(),
        environment: BTreeMap::new(),
        expected_value: fel_to_json(&result.value),
        expected_diagnostic_kinds: kinds,
    })
}

fn null_propagation_fixtures() -> Vec<ConformanceFixture> {
    vec![
        eval("null + 1"),
        eval("1 + null"),
        eval("null - 1"),
        eval("1 - null"),
        eval("null * 5"),
        eval("5 * null"),
        eval("null / 2"),
        eval("2 / null"),
        eval("null % 3"),
        eval("not null"),
        eval("-null"),
        eval("null < 1"),
        eval("1 < null"),
        eval("null > 1"),
        eval("null <= 1"),
        eval("null >= 1"),
        eval("null and true"),
        eval("false and null"),
        eval("null or false"),
        eval("true or null"),
        // null-coalesce propagates null
        eval("null ?? null"),
        // let-binding with null value
        eval("let x = null in x + 1"),
    ]
}

fn equality_fixtures() -> Vec<ConformanceFixture> {
    vec![
        eval("null == null"),
        eval("null != null"),
        eval("null == 0"),
        eval("null != 0"),
        eval("null == ''"),
        eval("null == false"),
        eval("0 == 0"),
        eval("0 != 0"),
        eval("0 != 1"),
        eval("'abc' == 'abc'"),
        eval("'abc' != 'ABC'"),
        eval("true == true"),
        eval("true == false"),
        eval("false != true"),
        eval("@2025-01-15 == @2025-01-15"),
        eval("@2025-01-15 != @2025-01-16"),
        eval("@2025-06-15T10:30:00 == @2025-06-15T10:30:00"),
        eval("[] == []"),
        eval("[1, 2] == [1, 2]"),
        eval("[1, 2] != [2, 1]"),
        eval("[null] == [null]"),
        eval("{} == {}"),
        eval("{'a': 1} == {'a': 1}"),
        eval("{'a': 1} != {'a': 2}"),
    ]
}

fn short_circuit_fixtures() -> Vec<ConformanceFixture> {
    vec![
        eval("true or (1 / 0)"),
        eval("false and (1 / 0)"),
        // if short-circuits
        eval("if true then 42 else (1 / 0)"),
        eval("if false then (1 / 0) else 99"),
    ]
}

fn date_arithmetic_fixtures() -> Vec<ConformanceFixture> {
    vec![
        eval("dateAdd(@2025-01-15, 30, 'days')"),
        eval("dateAdd(@2025-01-15, 1, 'months')"),
        eval("dateAdd(@2025-01-15, 1, 'years')"),
        eval("dateAdd(@2025-01-15, -7, 'days')"),
        eval("dateDiff(@2025-07-10, @2025-01-01, 'days')"),
        eval("dateDiff(@2025-12-31, @2025-01-01, 'months')"),
        eval("dateDiff(@2026-01-01, @2020-01-01, 'years')"),
        eval("dateDiff(@2025-01-01, @2025-07-10, 'days')"),
        eval("timeDiff('14:30:00', '13:00:00')"),
        eval("timeDiff('13:00:00', '14:30:00')"),
        eval("duration('PT1H')"),
        eval("duration('P1D')"),
        eval("duration('PT30M')"),
    ]
}

fn money_fixtures() -> Vec<ConformanceFixture> {
    vec![
        eval("money(50000, 'USD')"),
        eval("money(12500, 'EUR')"),
        eval("moneyAmount(money(50000, 'USD'))"),
        eval("moneyCurrency(money(50000, 'USD'))"),
        eval("moneyAdd(money(100, 'USD'), money(250, 'USD'))"),
        eval("money(100, 'USD') + money(250, 'USD')"),
        eval("money(100, 'USD') - money(30, 'USD')"),
        eval("money(100, 'USD') * 2"),
        eval("2 * money(100, 'USD')"),
        eval("money(100, 'USD') / 4"),
        eval("money(100, 'USD') / money(2, 'USD')"),
        eval("money(100, 'USD') % 3"),
        eval("moneySum([money(100, 'USD'), money(200, 'USD'), money(50, 'USD')])"),
        eval("moneySumWhere([money(10, 'USD'), money(100, 'USD'), money(20, 'USD')], moneyAmount($) > 50)"),
        // currency mismatch diagnostics
        eval("money(100, 'USD') + money(100, 'EUR')"),
        eval("moneyAdd(money(100, 'USD'), money(100, 'EUR'))"),
    ]
}

fn type_coercion_fixtures() -> Vec<ConformanceFixture> {
    vec![
        eval("number('42')"),
        eval("number('3.14')"),
        eval("number('abc')"),
        eval("string(42)"),
        eval("string(true)"),
        eval("string(null)"),
        eval("boolean(1)"),
        eval("boolean(0)"),
        eval("boolean('true')"),
        eval("boolean(null)"),
        eval("date('2025-07-10')"),
        eval("date('not-a-date')"),
        eval("typeOf(42)"),
        eval("typeOf([1, 2])"),
        eval("typeOf(null)"),
        eval("isNumber(42)"),
        eval("isNumber('42')"),
        eval("isString('hello')"),
        eval("isString(42)"),
        eval("isNull(null)"),
        eval("isNull('')"),
        eval("isNull(0)"),
        eval("empty(null)"),
        eval("empty('')"),
        eval("empty([])"),
        eval("empty('hello')"),
        eval("empty(0)"),
        eval("present('hello')"),
        eval("present(null)"),
    ]
}

fn comparison_fixtures() -> Vec<ConformanceFixture> {
    vec![
        eval("1 < 2"),
        eval("2 < 1"),
        eval("1 > 2"),
        eval("2 > 1"),
        eval("1 <= 1"),
        eval("1 >= 1"),
        eval("1 <= 2"),
        eval("2 >= 1"),
        eval("'a' < 'b'"),
        eval("'b' > 'a'"),
        eval("@2025-01-01 < @2025-12-31"),
        eval("@2025-06-15T10:30:00 > @2025-06-15T09:00:00"),
    ]
}

fn arithmetic_fixtures() -> Vec<ConformanceFixture> {
    vec![
        eval("1 + 2"),
        eval("5 - 3"),
        eval("4 * 7"),
        eval("10 / 3"),
        eval("10 / 0"),
        eval("10 % 3"),
        eval("-5 + 3"),
        eval("'hello' & ' world'"),
    ]
}

fn membership_fixtures() -> Vec<ConformanceFixture> {
    vec![
        eval("1 in [1, 2, 3]"),
        eval("4 in [1, 2, 3]"),
        eval("4 not in [1, 2, 3]"),
        eval("'a' in ['a', 'b']"),
        eval("null in [1, 2, 3]"),
        eval("1 in []"),
    ]
}

fn undefined_function_fixtures() -> Vec<ConformanceFixture> {
    vec![
        eval("nonexistent(1, 2)"),
        eval("notARealFunc()"),
    ]
}

fn field_reference_fixtures() -> Vec<ConformanceFixture> {
    vec![
        eval_with_env("$x + $y", serde_json::json!({"x": 3, "y": 4})),
        eval_with_env("$name", serde_json::json!({"name": "Alice"})),
        eval_with_env("$a > $b", serde_json::json!({"a": 10, "b": 5})),
        eval_with_env("$nested.a", serde_json::json!({"nested": {"a": 42}})),
        eval_with_env("$arr[1]", serde_json::json!({"arr": [10, 20, 30]})),
    ]
}

fn aggregate_fixtures() -> Vec<ConformanceFixture> {
    vec![
        eval("sum([10, 20, 30])"),
        eval("sum([10, null, 20])"),
        eval("sum([])"),
        eval("count([1, null, 3])"),
        eval("count([])"),
        eval("avg([10, 20, 30])"),
        eval("avg([10, null, 30])"),
        eval("min([5, 2, 8])"),
        eval("max([5, 2, 8])"),
        eval("countWhere([1, 2, 3, 4, 5], $ > 2)"),
        eval("sumWhere([1, 2, 3, 4], $ > 2)"),
        eval("avgWhere([40, 50, 60], $ >= 50)"),
        eval("minWhere([3, 1, 4], $ > 0)"),
        eval("maxWhere([3, 1, 4], $ < 4)"),
        eval("every([1, 2, 3], $ > 0)"),
        eval("every([], $ > 0)"),
        eval("some([0, 2, 4], $ > 3)"),
        eval("some([], $ > 0)"),
    ]
}

fn operator_type_mismatch_fixtures() -> Vec<ConformanceFixture> {
    vec![
        eval("not 42"),
        eval("-'hello'"),
        eval("'hello' + 1"),
        eval("true and 'x'"),
        eval("false or 1"),
    ]
}

// ── Main ─────────────────────────────────────────────────────────────────

fn all_semantic_fixtures() -> Vec<ConformanceFixture> {
    let mut out = Vec::new();

    out.extend(catalog_example_fixtures());
    out.extend(null_propagation_fixtures());
    out.extend(equality_fixtures());
    out.extend(short_circuit_fixtures());
    out.extend(date_arithmetic_fixtures());
    out.extend(money_fixtures());
    out.extend(type_coercion_fixtures());
    out.extend(comparison_fixtures());
    out.extend(arithmetic_fixtures());
    out.extend(membership_fixtures());
    out.extend(undefined_function_fixtures());
    out.extend(field_reference_fixtures());
    out.extend(aggregate_fixtures());
    out.extend(operator_type_mismatch_fixtures());

    out
}

fn main() {
    let target: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let semantic = all_semantic_fixtures();
    let base_len = semantic.len();

    let stdout = std::io::stdout();
    let mut writer = stdout.lock();

    for fixture in &semantic {
        emit(&mut writer, fixture);
    }

    if target > base_len {
        #[cfg(feature = "proptest-strategies")]
        {
            let surplus = target - base_len;
            let catalog = builtin_function_catalog();
            let strategy = fel_core::testing::strategies::arb_expr(3, catalog);
            let mut runner = proptest::test_runner::TestRunner::deterministic();
            let mut count = 0usize;
            while count < surplus {
                if let Ok(tree) = strategy.new_tree(&mut runner) {
                    let expr_tree = tree.current();
                    let source = fel_core::print_expr(&expr_tree);
                    let fixture = eval(&source);
                    emit(&mut writer, &fixture);
                    count += 1;
                    if count % 64 == 0 {
                        eprintln!(
                            "emitted {}/{} random ({} total)",
                            count,
                            surplus,
                            base_len + count
                        );
                    }
                }
            }
        }
    }

    eprintln!(
        "emitted {} semantic + {} random = {} total fixtures",
        base_len,
        if target > base_len { target - base_len } else { 0 },
        if target > base_len { target } else { base_len }
    );
}
