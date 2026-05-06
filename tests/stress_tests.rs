//! Stress and coarse performance sanity checks (bounded sizes so CI stays fast).
//!
//! These are not Criterion benchmarks; they guard regressions on large inputs.

use fel_core::{MapEnvironment, Value, evaluate, parse, tokenize};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Sum of `0..n` as closed form `n * (n - 1) / 2` for integer `n`.
fn triangular(n: usize) -> Decimal {
    Decimal::from(n * (n.saturating_sub(1)) / 2)
}

#[test]
fn large_literal_array_sum() {
    const N: usize = 1_000;
    let inner: String = (0..N).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let src = format!("sum([{inner}])");
    let expr = parse(&src).expect("parse large array literal");
    let env = MapEnvironment::new();
    let out = evaluate(&expr, &env).value;
    assert_eq!(out, Value::Number(triangular(N)));
}

#[test]
fn large_flat_field_map_lookup() {
    const N: usize = 500;
    let mut fields = HashMap::with_capacity(N);
    for i in 0..N {
        fields.insert(format!("f{i}"), Value::Number(Decimal::from(i as i64)));
    }
    let expr = parse("$f249").expect("parse");
    let env = MapEnvironment::with_fields(fields);
    let out = evaluate(&expr, &env).value;
    assert_eq!(out, Value::Number(Decimal::from(249)));
}

#[test]
fn deep_parentheses_still_parse_below_limit() {
    // Parser max nesting frame limit is 32; stay clearly under.
    const DEPTH: usize = 28;
    let mut src = String::with_capacity(DEPTH * 2 + 2);
    src.extend(std::iter::repeat('(').take(DEPTH));
    src.push_str("42");
    src.extend(std::iter::repeat(')').take(DEPTH));
    let expr = parse(&src).expect("parse deeply parenthesized literal");
    let env = MapEnvironment::new();
    let out = evaluate(&expr, &env).value;
    assert_eq!(out, Value::Number(Decimal::from(42)));
}

#[test]
fn long_flat_addition_chain() {
    // Left-associative `+` builds a deep BinaryOp chain; keep TERMs modest to avoid eval stack overflow.
    const TERMS: usize = 48;
    let mut src = String::with_capacity(TERMS * 4);
    src.push_str("0");
    for _ in 0..TERMS {
        src.push_str(" + 1");
    }
    let expr = parse(&src).expect("parse long addition chain");
    let env = MapEnvironment::new();
    let out = evaluate(&expr, &env).value;
    assert_eq!(out, Value::Number(Decimal::from(TERMS)));
}

#[test]
fn tokenize_long_expression() {
    const TERMS: usize = 400;
    let mut src = String::with_capacity(TERMS * 4);
    src.push('1');
    for _ in 0..TERMS {
        src.push_str(" + 1");
    }
    let toks = tokenize(&src).expect("tokenize");
    assert!(toks.len() > TERMS, "expected many tokens, got {}", toks.len());
}

#[test]
fn tokenize_long_expression_parse_eval_agrees_with_flat_add_chain() {
    // Same term count as `long_flat_addition_chain` — deeper left-deep AST risks eval stack overflow.
    const TERMS: usize = 48;
    let mut src = String::with_capacity(TERMS * 4);
    src.push_str("0");
    for _ in 0..TERMS {
        src.push_str(" + 1");
    }
    let toks = tokenize(&src).expect("tokenize");
    assert!(toks.len() > TERMS);
    let expr = parse(&src).expect("parse long addition");
    let env = MapEnvironment::new();
    let out = evaluate(&expr, &env).value;
    assert_eq!(out, Value::Number(Decimal::from(TERMS)));
}
