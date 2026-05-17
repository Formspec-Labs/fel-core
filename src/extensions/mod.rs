//! FEL extension function registry with null propagation and conflict detection.
//!
//! Extensions cannot shadow reserved words or built-in function names.
//! All extension functions are null-propagating: if any argument is null, the result is null.
//!
//! Registration, dispatch, and `BUILTIN_FUNCTIONS` back the catalog / WASM surfaces.
//!
//! ## Design note (spec: core/spec.md §3.12, registry/extension-registry.md §7)
//!
//! `ExtensionRegistry` is intentionally isolated from the evaluator's built-in
//! function dispatch. The spec says extensions "MAY supplement but MUST NOT
//! override" built-ins. This is enforced structurally: the evaluator matches
//! built-in names first in `eval_function`, and only falls through to the
//! extension registry for unknown names. The registry itself independently
//! rejects registration of names that collide with built-ins or reserved words.
//!
//! This two-layer defense is by design, not accident. The evaluator's match
//! arms guarantee built-in semantics can never be replaced at runtime, while
//! the registry's registration-time check gives early feedback to extension
//! authors. Neither layer alone would be sufficient: without the evaluator
//! guard, a bug in the registry could allow shadowing; without the registry
//! guard, extensions would silently be ignored instead of rejected.
#![allow(clippy::missing_docs_in_private_items)]

mod catalog;
mod registry;
mod schema;
mod types;

pub use catalog::{builtin_function_catalog, builtin_function_catalog_for};
pub use registry::{ExtensionCallOutcome, ExtensionError, ExtensionRegistry};
pub use schema::{
    builtin_function_catalog_json_value, builtin_function_catalog_json_value_for, emit_schema_json,
};
pub use types::{
    BuiltinFunctionCatalogEntry, Example, ExtensionFn, ExtensionFunc, FelType, Package, Parameter,
};
#[cfg(test)]
mod tests {
    #![allow(clippy::missing_docs_in_private_items)]
    use super::catalog::BUILTIN_FUNCTIONS;
    use super::schema::synthesize_signature;
    use super::*;
    use crate::types::Value as TypeValue;
    use rust_decimal::Decimal;

    fn num(n: i64) -> TypeValue {
        TypeValue::Number(Decimal::from(n))
    }

    fn s(v: &str) -> TypeValue {
        TypeValue::String(v.to_string())
    }

    #[test]
    fn test_register_and_call() {
        let mut registry = ExtensionRegistry::new();
        registry
            .register("double", 1, Some(1), |args| match &args[0] {
                TypeValue::Number(n) => TypeValue::Number(*n * Decimal::from(2)),
                _ => TypeValue::Null,
            })
            .unwrap();

        assert!(registry.contains("double"));
        assert_eq!(
            registry.call("double", &[num(5)]),
            ExtensionCallOutcome::Ok(num(10))
        );
    }

    #[test]
    fn test_null_propagation() {
        let mut registry = ExtensionRegistry::new();
        registry
            .register("identity", 1, Some(1), |args| args[0].clone())
            .unwrap();

        assert_eq!(
            registry.call("identity", &[TypeValue::Null]),
            ExtensionCallOutcome::Ok(TypeValue::Null)
        );
        assert_eq!(
            registry.call("identity", &[num(42)]),
            ExtensionCallOutcome::Ok(num(42))
        );
    }

    #[test]
    fn test_cannot_shadow_reserved() {
        let mut registry = ExtensionRegistry::new();
        assert!(
            registry
                .register("if", 1, None, |_| TypeValue::Null)
                .is_err()
        );
        assert!(
            registry
                .register("true", 0, None, |_| TypeValue::Null)
                .is_err()
        );
        assert!(
            registry
                .register("and", 2, None, |_| TypeValue::Null)
                .is_err()
        );
    }

    #[test]
    fn test_cannot_shadow_builtin() {
        let mut registry = ExtensionRegistry::new();
        assert!(
            registry
                .register("sum", 1, None, |_| TypeValue::Null)
                .is_err()
        );
        assert!(
            registry
                .register("round", 1, None, |_| TypeValue::Null)
                .is_err()
        );
        assert!(
            registry
                .register("today", 0, None, |_| TypeValue::Null)
                .is_err()
        );
    }

    #[test]
    fn test_unknown_extension_returns_none() {
        let registry = ExtensionRegistry::new();
        assert_eq!(
            registry.call("unknownExt", &[num(1)]),
            ExtensionCallOutcome::NotFound
        );
    }

    #[test]
    fn test_multi_arg_extension() {
        let mut registry = ExtensionRegistry::new();
        registry
            .register("concat3", 3, Some(3), |args| {
                let parts: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                TypeValue::String(parts.join("-"))
            })
            .unwrap();

        assert_eq!(
            registry.call("concat3", &[s("a"), s("b"), s("c")]),
            ExtensionCallOutcome::Ok(s("a-b-c"))
        );
    }

    #[test]
    fn test_call_rejects_too_few_args() {
        let mut registry = ExtensionRegistry::new();
        registry
            .register("needsTwo", 2, Some(2), |args| match (&args[0], &args[1]) {
                (TypeValue::Number(a), TypeValue::Number(b)) => TypeValue::Number(*a + *b),
                _ => TypeValue::Null,
            })
            .unwrap();

        match registry.call("needsTwo", &[num(1)]) {
            ExtensionCallOutcome::ArityMismatch {
                name,
                min_args,
                max_args,
                got,
            } => {
                assert_eq!(name, "needsTwo");
                assert_eq!((min_args, max_args, got), (2, Some(2), 1));
                assert_eq!(
                    crate::error::extension_arity_mismatch_message(&name, min_args, max_args, got),
                    "needsTwo: requires exactly 2 arguments"
                );
            }
            other => panic!("expected arity mismatch, got {other:?}"),
        }
    }

    #[test]
    fn test_call_rejects_too_many_args() {
        let mut registry = ExtensionRegistry::new();
        registry
            .register("atMostOne", 0, Some(1), |args| {
                args.first().cloned().unwrap_or(TypeValue::Null)
            })
            .unwrap();

        match registry.call("atMostOne", &[num(1), num(2)]) {
            ExtensionCallOutcome::ArityMismatch {
                name,
                min_args,
                max_args,
                got,
            } => {
                assert_eq!(name, "atMostOne");
                assert_eq!((min_args, max_args, got), (0, Some(1), 2));
                assert_eq!(
                    crate::error::extension_arity_mismatch_message(&name, min_args, max_args, got),
                    "atMostOne: requires at most 1 argument"
                );
            }
            other => panic!("expected arity mismatch, got {other:?}"),
        }
    }

    #[test]
    fn test_call_allows_unbounded_max_args() {
        let mut registry = ExtensionRegistry::new();
        registry
            .register("sumMany", 1, None, |args| {
                let total = args.iter().fold(Decimal::ZERO, |acc, v| match v {
                    TypeValue::Number(n) => acc + n,
                    _ => acc,
                });
                TypeValue::Number(total)
            })
            .unwrap();

        assert_eq!(
            registry.call("sumMany", &[num(1), num(2), num(3)]),
            ExtensionCallOutcome::Ok(num(6))
        );
    }

    #[test]
    fn package_filter_universal_excludes_formspec() {
        let universal: Vec<_> = builtin_function_catalog_for(Package::Universal).collect();
        assert!(
            universal
                .iter()
                .all(|e| matches!(e.package, Package::Universal))
        );
        // Formspec-only names must be absent
        assert!(!universal.iter().any(|e| e.name == "valid"));
        assert!(!universal.iter().any(|e| e.name == "prev"));
        assert!(!universal.iter().any(|e| e.name == "instance"));
        assert!(!universal.iter().any(|e| e.name == "locale"));
    }

    #[test]
    fn package_filter_formspec_includes_all() {
        let formspec_count: usize = builtin_function_catalog_for(Package::Formspec).count();
        assert_eq!(formspec_count, BUILTIN_FUNCTIONS.len());
    }

    #[test]
    fn emit_schema_has_74_functions() {
        let v = emit_schema_json();
        let funcs = v["functions"].as_array().expect("functions array");
        assert_eq!(
            funcs.len(),
            74,
            "expected 74 functions, got {}",
            funcs.len()
        );
    }

    #[test]
    fn result_json_parses_for_all_examples() {
        for entry in BUILTIN_FUNCTIONS.iter() {
            for ex in entry.examples {
                serde_json::from_str::<serde_json::Value>(ex.result_json).unwrap_or_else(|e| {
                    panic!(
                        "{}::{} — invalid result_json {:?}: {e}",
                        entry.name, ex.expression, ex.result_json
                    )
                });
            }
        }
    }

    #[test]
    fn synthesize_signature_zero_arg() {
        // today() -> date
        assert_eq!(
            synthesize_signature("today", &[], FelType::Date),
            "today() -> date"
        );
    }

    #[test]
    fn synthesize_signature_single_required_arg() {
        // length(value) -> number
        let params = [Parameter {
            name: "value",
            fel_type: FelType::String,
            description: None,
            required: true,
            variadic: false,
            allowed_values: None,
        }];
        assert_eq!(
            synthesize_signature("length", &params, FelType::Number),
            "length(value) -> number"
        );
    }

    #[test]
    fn synthesize_signature_optional_arg() {
        // substring(value, start, length?) -> string
        let params = [
            Parameter {
                name: "value",
                fel_type: FelType::String,
                description: None,
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "start",
                fel_type: FelType::Number,
                description: None,
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "length",
                fel_type: FelType::Number,
                description: None,
                required: false,
                variadic: false,
                allowed_values: None,
            },
        ];
        assert_eq!(
            synthesize_signature("substring", &params, FelType::String),
            "substring(value, start, length?) -> string"
        );
    }

    #[test]
    fn synthesize_signature_variadic_arg() {
        // coalesce(...values) -> any
        let params = [Parameter {
            name: "values",
            fel_type: FelType::Any,
            description: None,
            required: true,
            variadic: true,
            allowed_values: None,
        }];
        assert_eq!(
            synthesize_signature("coalesce", &params, FelType::Any),
            "coalesce(...values) -> any"
        );
    }

    #[test]
    fn synthesize_signature_multi_required_args() {
        // dateDiff(date1, date2, unit) -> number
        let params = [
            Parameter {
                name: "date1",
                fel_type: FelType::Date,
                description: None,
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "date2",
                fel_type: FelType::Date,
                description: None,
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "unit",
                fel_type: FelType::String,
                description: None,
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ];
        assert_eq!(
            synthesize_signature("dateDiff", &params, FelType::Number),
            "dateDiff(date1, date2, unit) -> number"
        );
    }

    #[test]
    fn catalog_json_value_includes_signature_field() {
        let catalog = builtin_function_catalog_json_value();
        let arr = catalog.as_array().expect("catalog is an array");
        // Every entry must have a "signature" key that is a non-empty string.
        for entry in arr {
            let sig = entry["signature"].as_str().expect("signature is a string");
            assert!(!sig.is_empty(), "signature must not be empty for {entry}");
            assert!(
                sig.contains(" -> "),
                "signature must contain ' -> ' for {sig}"
            );
        }
    }

    #[test]
    fn emit_schema_json_has_no_signature_field() {
        // Round-trip invariant: the schema envelope must NOT contain "signature" on functions.
        let schema = emit_schema_json();
        let funcs = schema["functions"].as_array().expect("functions array");
        for entry in funcs {
            assert!(
                entry.get("signature").is_none(),
                "schema envelope must not have 'signature' field on {}",
                entry["name"]
            );
        }
    }
}
