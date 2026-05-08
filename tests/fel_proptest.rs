//! Property tests for FEL parse/print fidelity, null propagation, and JSON conversion.
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{
    evaluate, fel_to_json, json_to_fel, parse, print_expr, Expr, MapEnvironment, Value,
    ast::BinaryOp,
    testing::strategies::arb_value,
};
use proptest::prelude::*;
use serde_json::Value as JsonValue;

fn value_to_expr(v: &Value) -> Expr {
    match v {
        Value::Null => Expr::Null,
        Value::Boolean(b) => Expr::Boolean(*b),
        Value::Number(n) => Expr::Number(*n),
        Value::String(s) => Expr::String(s.clone()),
        Value::Date(d) => Expr::DateLiteral(format!("@{}", d.format_iso())),
        Value::Array(items) => Expr::Array(items.iter().map(value_to_expr).collect()),
        Value::Object(entries) => Expr::Object(
            entries.iter().map(|(k, v)| (k.clone(), value_to_expr(v))).collect(),
        ),
        Value::Money(_) => Expr::Null,
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        ..Default::default()
    })]

    /// Integer literals survive parse → print → parse.
    #[test]
    fn parse_print_roundtrip_decimal_integer(n in any::<i32>()) {
        let src = n.to_string();
        let expr = parse(&src).expect("integer literal parses");
        let printed = print_expr(&expr);
        let expr2 = parse(&printed).expect("printed form re-parses");
        prop_assert_eq!(expr, expr2);
    }

    /// Arithmetic null propagation on numbers (spec §3).
    #[test]
    fn null_propagates_through_binary_numeric(v in arb_value(None)) {
        prop_assume!(!matches!(&v, Value::Null | Value::Array(_) | Value::Object(_) | Value::Money(_)));
        let expr = value_to_expr(&v);
        let ops = [
            BinaryOp::Add, BinaryOp::Sub, BinaryOp::Mul, BinaryOp::Div,
            BinaryOp::Lt, BinaryOp::Gt, BinaryOp::LtEq, BinaryOp::GtEq,
        ];
        let env = MapEnvironment::new();
        for op in &ops {
            let left_null = Expr::BinaryOp { op: *op, left: Box::new(Expr::Null), right: Box::new(expr.clone()) };
            prop_assert_eq!(evaluate(&left_null, &env).value, Value::Null);
            let right_null = Expr::BinaryOp { op: *op, left: Box::new(expr.clone()), right: Box::new(Expr::Null) };
            prop_assert_eq!(evaluate(&right_null, &env).value, Value::Null);
        }
    }

    /// Equality does not propagate null.
    #[test]
    fn equality_no_null_propagation(v in arb_value(None)) {
        prop_assume!(!matches!(&v, Value::Null | Value::Money(_)));
        let expr = value_to_expr(&v);
        let env = MapEnvironment::new();
        let null_eq_null = Expr::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expr::Null),
            right: Box::new(Expr::Null),
        };
        prop_assert_eq!(evaluate(&null_eq_null, &env).value, Value::Boolean(true));
        let null_eq_expr = Expr::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expr::Null),
            right: Box::new(expr.clone()),
        };
        prop_assert_eq!(evaluate(&null_eq_expr, &env).value, Value::Boolean(false));
        let expr_eq_null = Expr::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(expr),
            right: Box::new(Expr::Null),
        };
        prop_assert_eq!(evaluate(&expr_eq_null, &env).value, Value::Boolean(false));
    }

    /// Shallow JSON object preserves key order (IndexMap wire).
    #[test]
    fn json_object_order_roundtrip(v in arb_value(None)) {
        prop_assume!(matches!(&v, Value::Object(_)));
        let json = fel_to_json(&v);
        let back = json_to_fel(&json);
        let back_json = fel_to_json(&back);
        prop_assert_eq!(back_json, json);
    }

    /// JSON scalars round-trip where conversion is defined.
    #[test]
    fn json_scalar_roundtrip(
        v in prop_oneof![
            Just(JsonValue::Null),
            any::<bool>().prop_map(JsonValue::Bool),
            (-1000_i64..1000_i64).prop_map(|n| JsonValue::Number(n.into())),
            "[a-z]{1,12}".prop_map(JsonValue::String),
        ]
    ) {
        let fel = json_to_fel(&v);
        let back = fel_to_json(&fel);
        prop_assert_eq!(back, v);
    }
}
