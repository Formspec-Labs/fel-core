//! Property tests for FEL parse/print fidelity, null propagation, and JSON conversion.
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{
    Expr, MapEnvironment, Value, ast::BinaryOp, evaluate, fel_to_json, fel_to_ui_json,
    fel_to_wire_json, json_to_fel, parse, print_expr, testing::strategies::arb_value,
};
use proptest::prelude::*;
use serde_json::Value as JsonValue;

/// Closed set of `TypeValue → serde_json::Value` encoders exposed from `fel_core`.
///
/// Note: this enum is the value-encoding axis, not the FFI key-casing axis (which
/// is `JsonWireStyle::{JsCamel, PythonSnake}` and only applies to diagnostic /
/// dependency / lexer envelopes — not value conversion). The Phase 3 P0 design
/// (`thoughts/2026-05-23-phase-3-ci-gate-design.md:237`) names "JsonWireStyle"
/// but the actual coverage gap it cites — `fel_to_ui_json` + `fel_to_wire_json`
/// — is the value-encoder axis below.
///
/// Each variant has a distinct invariant; see [`assert_value_encoder_invariant`].
#[derive(Debug, Clone, Copy)]
enum ValueEncoder {
    /// `fel_to_wire_json` — typed envelopes (`$type: number|date|money`); FULL
    /// round-trip through `json_to_fel` (reverse-mappable wire format).
    Wire,
    /// `fel_to_ui_json` — bare scalars (Date → ISO string, Money → flat object);
    /// IDEMPOTENT under re-encode but lossy (Date / Money type identity not
    /// recoverable through `json_to_fel`).
    Ui,
    /// `fel_to_json` — alias for `fel_to_ui_json` (back-compat name); must be
    /// byte-equal to `Ui`.
    Alias,
}

/// All encoder variants. Iterating this list keeps the per-variant invariants
/// in lock-step with the closed enum: adding a new encoder requires extending
/// both `ValueEncoder` and `ALL_ENCODERS`, and the exhaustive `match` in
/// `assert_value_encoder_invariant` forces an invariant decision.
const ALL_ENCODERS: &[ValueEncoder] = &[ValueEncoder::Wire, ValueEncoder::Ui, ValueEncoder::Alias];

/// Asserts the round-trip / idempotency invariant appropriate to `encoder`.
///
/// - `Wire`: full reverse-mappable round-trip — `v == json_to_fel(encode(v))`.
/// - `Ui`:   re-encode idempotency — `encode(json_to_fel(encode(v))) == encode(v)`
///           (Date / Money lose type identity through `json_to_fel`, so the
///           round-trip is weaker than `Wire`).
/// - `Alias`: byte-equal to `Ui` AND inherits the `Ui` idempotency invariant.
fn assert_value_encoder_invariant(
    encoder: ValueEncoder,
    v: &Value,
) -> Result<(), proptest::test_runner::TestCaseError> {
    match encoder {
        ValueEncoder::Wire => {
            let wire = fel_to_wire_json(v);
            let back = json_to_fel(&wire);
            prop_assert_eq!(&back, v, "fel_to_wire_json must round-trip via json_to_fel");
        }
        ValueEncoder::Ui => {
            let ui = fel_to_ui_json(v);
            let back = json_to_fel(&ui);
            let ui_again = fel_to_ui_json(&back);
            prop_assert_eq!(
                ui_again,
                ui,
                "fel_to_ui_json must be idempotent under json_to_fel re-encode"
            );
        }
        ValueEncoder::Alias => {
            // fel_to_json is documented as a back-compat alias for fel_to_ui_json
            // (src/convert.rs:271). Both invariants must hold:
            //  1. Output is byte-equal to fel_to_ui_json (alias contract).
            //  2. Same idempotency as Ui (inherited from the aliased function).
            let via_alias = fel_to_json(v);
            let via_ui = fel_to_ui_json(v);
            prop_assert_eq!(
                &via_alias,
                &via_ui,
                "fel_to_json must be byte-equal to fel_to_ui_json (alias contract)"
            );
            let back = json_to_fel(&via_alias);
            let alias_again = fel_to_json(&back);
            prop_assert_eq!(
                alias_again,
                via_alias,
                "fel_to_json (alias) must be idempotent under json_to_fel re-encode"
            );
        }
    }
    Ok(())
}

fn value_to_expr(v: &Value) -> Expr {
    match v {
        Value::Null => Expr::Null,
        Value::Boolean(b) => Expr::Boolean(*b),
        Value::Number(n) => Expr::Number(*n),
        Value::String(s) => Expr::String(s.clone()),
        Value::Date(d) => Expr::DateLiteral(format!("@{}", d.format_iso())),
        Value::Array(items) => Expr::Array(items.iter().map(value_to_expr).collect()),
        Value::Object(entries) => Expr::Object(
            entries
                .iter()
                .map(|(k, v)| (k.clone(), value_to_expr(v)))
                .collect(),
        ),
        // Money values lift to `money(amount, 'CCY')` constructor calls.
        // Previously this branch returned `Expr::Null`, which silently
        // dropped Money values into the null-propagation path — making
        // proptest rows with Money operands tautological. Now the
        // generated Expr actually evaluates back to the original Money,
        // so properties that pass through Money see real Money behavior.
        Value::Money(m) => Expr::FunctionCall {
            name: "money".to_string(),
            args: vec![
                Expr::Number(m.amount),
                Expr::String(m.currency.as_str().to_string()),
            ],
        },
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

    /// Per-encoder round-trip / idempotency for the three `TypeValue → serde_json`
    /// surfaces (`fel_to_wire_json`, `fel_to_ui_json`, `fel_to_json`).
    ///
    /// Phase 3 P0 — closes the lib re-export coverage gap for `fel_to_ui_json`
    /// and `fel_to_wire_json` cited in
    /// `thoughts/2026-05-23-phase-3-ci-gate-design.md:237`. The three encoders
    /// have distinct invariants (Wire: full round-trip; Ui: re-encode
    /// idempotency; Alias: byte-equal to Ui), so each is asserted separately
    /// — see [`assert_value_encoder_invariant`]. The exhaustive iteration
    /// over `ALL_ENCODERS` paired with the exhaustive `match` ensures that
    /// adding a new variant to `ValueEncoder` fails to compile until the
    /// invariant is decided.
    #[test]
    fn value_encoder_invariant_holds_for_all_encoders(v in arb_value(None)) {
        for encoder in ALL_ENCODERS {
            assert_value_encoder_invariant(*encoder, &v)?;
        }
    }
}
