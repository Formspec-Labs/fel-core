//! Property tests for FEL parse/print fidelity, null propagation, and JSON conversion.
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{
    evaluate, fel_to_json, json_to_fel, parse, print_expr, MapEnvironment, Value,
};
use proptest::prelude::*;
use serde_json::{json, Value as JsonValue};

fn eval_plain(src: &str) -> Value {
    let expr = parse(src).unwrap();
    let env = MapEnvironment::new();
    evaluate(&expr, &env).value
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
    fn null_propagates_through_binary_numeric(_ in any::<u8>()) {
        prop_assert_eq!(eval_plain("null + 1"), Value::Null);
        prop_assert_eq!(eval_plain("1 + null"), Value::Null);
        prop_assert_eq!(eval_plain("null * 5"), Value::Null);
        prop_assert_eq!(eval_plain("null < 1"), Value::Null);
    }

    /// Equality does not propagate null.
    #[test]
    fn equality_no_null_propagation(_ in any::<u8>()) {
        prop_assert_eq!(eval_plain("null = null"), Value::Boolean(true));
        prop_assert_eq!(eval_plain("null = 1"), Value::Boolean(false));
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

    /// Shallow JSON object preserves key order (IndexMap wire).
    #[test]
    fn json_object_order_roundtrip(_ in any::<u8>()) {
        let v = json!({ "a": 1, "b": 2, "c": 3 });
        let fel = json_to_fel(&v);
        let back = fel_to_json(&fel);
        prop_assert_eq!(back, v);
    }
}
