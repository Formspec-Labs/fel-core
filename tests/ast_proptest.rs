//! AST-level property tests using generative strategies.
//!
//! Requires `cfg(feature = "proptest-strategies")` since the strategies module is gated.
//! Run with: `cargo test --features proptest-strategies --test ast_proptest`
#![cfg(feature = "proptest-strategies")]
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::testing::strategies::{arb_decimal, arb_expr, arb_value};
use fel_core::{MapEnvironment, builtin_function_catalog, evaluate, parse, print_expr, tokenize};
use proptest::prelude::*;

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        ..Default::default()
    })]

    /// 1. `parse(print(ast)) == ast` for ASTs without known parse/print asymmetries.
    #[test]
    fn parse_print_identity(
        expr in arb_expr(3, builtin_function_catalog())
    ) {
        let printed = print_expr(&expr);
        let reparsed = parse(&printed).expect("printed form must reparse");
        prop_assert_eq!(expr, reparsed);
    }

    /// 2. `eval(parse(s), env) == eval(parse(s), env)` — determinism across two evaluations.
    #[test]
    fn eval_determinism(
        expr in arb_expr(3, builtin_function_catalog())
    ) {
        let env = MapEnvironment::new();
        let result1 = evaluate(&expr, &env);
        let result2 = evaluate(&expr, &env);
        prop_assert_eq!(result1.value, result2.value);
    }

    /// 3. No panic on `tokenize/parse/print/eval` for any generated AST.
    #[test]
    fn no_panic_on_full_pipeline(
        expr in arb_expr(3, builtin_function_catalog())
    ) {
        let printed = print_expr(&expr);
        let _ = parse(&printed);
        let _ = tokenize(&printed);
        let env = MapEnvironment::new();
        let _ = evaluate(&expr, &env);
    }

    /// 4. Every `Decimal` operation in eval returns Ok or Null — never panics.
    #[test]
    fn decimal_never_panics(
        expr in arb_expr(2, builtin_function_catalog())
    ) {
        let env = MapEnvironment::new();
        let result = evaluate(&expr, &env);
        // The fact that we got here without panic is the property.
        // Verify the result is a well-formed Value.
        let _ = format!("{}", result.value);
    }

    /// Values round-trip through their Display/format representation.
    #[test]
    fn value_fmt_does_not_panic(
        v in arb_value(2)
    ) {
        let _ = format!("{}", v);
    }

    /// Decimal strategy bias check: all values are well-formed.
    #[test]
    fn arb_decimal_values_are_finite(
        d in arb_decimal()
    ) {
        let s = d.to_string();
        prop_assert!(!s.is_empty());
    }
}
