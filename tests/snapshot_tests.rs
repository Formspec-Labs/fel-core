//! Snapshot tests for error messages produced by the parser and evaluator.
//!
//! Uses `insta` to pin exact wording; PR diffs surface message changes for review.
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{MapEnvironment, evaluate, parse};
use insta::assert_snapshot;

fn diag_first(src: &str) -> String {
    let expr = match parse(src) {
        Ok(e) => e,
        Err(e) => return format!("parse error: {e}"),
    };
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    result
        .diagnostics
        .first()
        .map(|d| d.message.clone())
        .unwrap_or_else(|| "no diagnostics".to_string())
}

#[test]
fn snapshot_parse_errors() {
    assert_snapshot!(diag_first(""), @"parse error: parse error: unexpected token Eof");
    assert_snapshot!(diag_first("$a +"), @"parse error: parse error: unexpected token Eof");
    assert_snapshot!(diag_first("("), @"parse error: parse error: unexpected token Eof");
    assert_snapshot!(diag_first("$a + + $b"), @"parse error: parse error: unexpected token Plus");
}

#[test]
fn snapshot_eval_errors() {
    assert_snapshot!(diag_first("undefined_fn()"), @"undefined function: undefined_fn");
    assert_snapshot!(diag_first("if(42, 1, 2)"), @"if: expected boolean, got number");
    assert_snapshot!(diag_first("1 / 0"), @"division by zero");
    assert_snapshot!(diag_first("sum('hello')"), @"sum: expected array, got string");
}

#[test]
fn snapshot_type_mismatch_errors() {
    assert_snapshot!(diag_first("'a' + 'b'"), @"cannot apply '+' to string and string");
    assert_snapshot!(diag_first("true < false"), @"cannot compare boolean with boolean");
    assert_snapshot!(diag_first("not 5"), @"cannot apply 'not' to number");
    assert_snapshot!(diag_first("1 and 2"), @"cannot apply 'and' to number");
}

#[test]
fn snapshot_builtin_arity_errors() {
    assert_snapshot!(diag_first("countWhere([1])"), @"countWhere: requires at least 2 arguments");
    assert_snapshot!(diag_first("moneySumWhere([money(1, 'USD')])"), @"moneySumWhere: requires at least 2 arguments");
    assert_snapshot!(diag_first("money(10, 'XXXx')"), @"money: currency must be a three-letter ISO code");
    assert_snapshot!(diag_first("money(10, 'USD', 3)"), @"no diagnostics");
}
