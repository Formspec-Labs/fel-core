//! Conformance tests for FEL §6.3 host-supplied context bindings.
#![allow(clippy::missing_docs_in_private_items)]

use std::collections::HashMap;
use std::fs;

use fel_core::ast::PathSegment;
use fel_core::{
    ContextBinding, ContextBindingCatalog, ContextBindingKind, EmptyCatalog, Environment,
    EvaluatorOptions, IndexMap, MapEnvironment, UNBOUND_CONTEXT_REF_CODE, Value, evaluate,
    evaluate_with_catalog, fel_to_json, json_to_fel, parse,
};
use serde_json::Value as JsonValue;

#[derive(Default)]
struct TestCatalog {
    bindings: HashMap<String, ContextBinding>,
}

impl TestCatalog {
    fn with(name: &str, binding: ContextBinding) -> Self {
        Self {
            bindings: HashMap::from([(name.to_string(), binding)]),
        }
    }
}

impl ContextBindingCatalog for TestCatalog {
    fn binding_kind(&self, name: &str) -> Option<ContextBindingKind> {
        self.bindings.get(name).map(|binding| binding.kind)
    }

    fn resolve(&self, name: &str, _arg: Option<&str>) -> Option<Value> {
        self.bindings.get(name).map(|binding| binding.value.clone())
    }
}

struct ArgCatalog;

impl ContextBindingCatalog for ArgCatalog {
    fn binding_kind(&self, name: &str) -> Option<ContextBindingKind> {
        match name {
            "clock" => Some(ContextBindingKind::Function),
            _ => None,
        }
    }

    fn resolve(&self, name: &str, arg: Option<&str>) -> Option<Value> {
        match (name, arg) {
            ("clock", Some(zone)) => Some(Value::String(format!("tick:{zone}"))),
            ("clock", None) => Some(Value::String("tick".to_string())),
            _ => None,
        }
    }
}

struct LegacyContextEnv;

struct FullPathEnv;

impl Environment for FullPathEnv {
    fn resolve_field(
        &self,
        root: Option<&str>,
        path: &[PathSegment],
    ) -> Result<Value, fel_core::FieldError> {
        let path = path
            .iter()
            .fold(root.unwrap_or("$").to_owned(), |mut text, segment| {
                match segment {
                    PathSegment::Dot(key) => text.push_str(&format!(".{key}")),
                    PathSegment::Index(index) => text.push_str(&format!("[{index}]")),
                    PathSegment::Wildcard => text.push_str("[*]"),
                }
                text
            });
        if root == Some("bad") {
            return Err(fel_core::FieldError::new("host refused the field path"));
        }
        Ok(Value::String(path))
    }

    fn resolve_context(&self, _name: &str, _arg: Option<&str>, _tail: &[String]) -> Value {
        Value::Null
    }

    fn precomputed(&self, expr: &fel_core::Expr) -> Option<Value> {
        match expr {
            fel_core::Expr::FunctionCall { name, .. } if name == "sum" => {
                Some(Value::Boolean(true))
            }
            _ => None,
        }
    }
}

#[test]
fn host_receives_complete_authored_field_path_after_let_lookup() {
    for (source, expected) in [
        ("$a.b[*].c", "a.b[*].c"),
        ("a.b[*].c", "a.b[*].c"),
        ("(((($a).b)[*]).c)", "a.b[*].c"),
        ("a.b", "a.b"),
        ("($a).b", "a.b"),
    ] {
        let result = evaluate(&parse(source).unwrap(), &FullPathEnv);
        assert_eq!(result.value, Value::String(expected.into()), "{source}");
        assert!(
            result.diagnostics.is_empty(),
            "{source}: {:?}",
            result.diagnostics
        );
    }
    let result = evaluate(
        &parse("let a = {b: [{c: 'local'}]} in ((a).b)[1].c").unwrap(),
        &FullPathEnv,
    );
    assert_eq!(result.value, Value::String("local".into()));
    assert_eq!(
        evaluate(&parse("$.tail").unwrap(), &FullPathEnv).value,
        Value::String("$.tail".into())
    );
    let result = evaluate(&parse("$bad.tail").unwrap(), &FullPathEnv);
    assert_eq!(result.value, Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "host refused the field path")
    );
}

#[test]
fn postfix_after_parenthesized_wildcard_accesses_the_projected_value() {
    let mut env = fel_core::FormspecEnvironment::new();
    env.set_field(
        "rows",
        json_to_fel(&serde_json::json!([
            {"x": 1, "y": [10, 20]}, {"x": 2, "y": [30, 40]}
        ])),
    );
    for (source, expected) in [
        ("($rows[*].x)[1]", serde_json::json!(1)),
        ("($rows[*])[1]", serde_json::json!({"x": 1, "y": [10, 20]})),
        ("($rows[*].y)[1]", serde_json::json!([10, 20])),
    ] {
        let result = evaluate(&parse(source).unwrap(), &env);
        assert_eq!(fel_to_json(&result.value), expected, "{source}");
        assert!(
            result.diagnostics.is_empty(),
            "{source}: {:?}",
            result.diagnostics
        );
    }
    let result = evaluate(&parse("($rows[*]).x").unwrap(), &env);
    assert_eq!(result.value, Value::Null);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].message, "cannot access 'x' on array");
}

#[test]
fn invalid_field_projection_reports_one_error_and_returns_null() {
    let mut env = fel_core::FormspecEnvironment::new();
    env.set_field("items", json_to_fel(&serde_json::json!([1, 2, 3])));
    for source in ["$items[*].x", "count($items[*].x)"] {
        let result = evaluate(&parse(source).unwrap(), &env);
        assert_eq!(result.value, Value::Null, "{source}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{source}: {:?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].message, "cannot access 'x' on number");
    }
}

#[test]
fn precomputed_call_skips_argument_evaluation() {
    let result = evaluate(&parse("sum($bad.tail)").unwrap(), &FullPathEnv);
    assert_eq!(result.value, Value::Boolean(true));
    assert!(result.diagnostics.is_empty());
}

#[test]
fn resolve_value_segments_uses_one_based_indices() {
    let rows = Value::Array(vec![Value::String("first".into())]);
    assert_eq!(
        fel_core::resolve_value_segments(&rows, &[PathSegment::Index(1)]).unwrap(),
        Value::String("first".into())
    );
    assert!(fel_core::resolve_value_segments(&rows, &[PathSegment::Index(0)]).is_err());
}

impl Environment for LegacyContextEnv {
    fn resolve_field(
        &self,
        _root: Option<&str>,
        _path: &[PathSegment],
    ) -> Result<Value, fel_core::FieldError> {
        Ok(Value::Null)
    }

    fn resolve_context(&self, name: &str, _arg: Option<&str>, _tail: &[String]) -> Value {
        match name {
            "legacy" => Value::String("from-env".to_string()),
            "current" => Value::String("reserved-current".to_string()),
            _ => Value::Null,
        }
    }
}

fn object(entries: &[(&str, Value)]) -> Value {
    let mut fields = IndexMap::new();
    for (key, value) in entries {
        fields.insert((*key).to_string(), value.clone());
    }
    Value::Object(fields)
}

fn eval_with_catalog(source: &str, catalog: &dyn ContextBindingCatalog) -> fel_core::EvalResult {
    let expr = parse(source).expect("host-binding expression should parse");
    let env = MapEnvironment::new();
    evaluate_with_catalog(&expr, &env, EvaluatorOptions::default(), catalog)
}

fn fixture_catalog(fixture: &JsonValue) -> TestCatalog {
    let mut bindings = HashMap::new();
    let Some(catalog) = fixture["catalog"].as_object() else {
        return TestCatalog { bindings };
    };

    for (name, entry) in catalog {
        let kind = match entry["kind"]
            .as_str()
            .expect("fixture catalog entry should carry kind")
        {
            "value" => ContextBindingKind::Value,
            "object" => ContextBindingKind::Object,
            "function" => ContextBindingKind::Function,
            other => panic!("unsupported fixture binding kind: {other}"),
        };
        bindings.insert(
            name.clone(),
            ContextBinding {
                kind,
                value: json_to_fel(&entry["value"]),
            },
        );
    }

    TestCatalog { bindings }
}

#[test]
fn registered_object_binding_resolves_tail() {
    let catalog = TestCatalog::with(
        "response",
        ContextBinding::object(object(&[(
            "applicantName",
            Value::String("Alice".to_string()),
        )])),
    );

    let result = eval_with_catalog("@response.applicantName", &catalog);

    assert_eq!(result.value, Value::String("Alice".to_string()));
    assert!(result.diagnostics.is_empty());
}

#[test]
fn empty_catalog_rejects_unbound_context_ref() {
    let catalog = EmptyCatalog;

    let result = eval_with_catalog("@unbound", &catalog);

    assert_eq!(result.value, Value::Null);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_deref() == Some(UNBOUND_CONTEXT_REF_CODE)
            && diagnostic.message == "unbound context reference: @unbound"
    }));
}

#[test]
fn mapping_source_is_rejected_when_not_in_active_catalog() {
    let catalog = EmptyCatalog;

    let result = eval_with_catalog("@source", &catalog);

    assert_eq!(result.value, Value::Null);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_deref() == Some(UNBOUND_CONTEXT_REF_CODE)
            && diagnostic.message == "unbound context reference: @source"
    }));
}

#[test]
fn catalog_function_binding_requires_call_syntax() {
    let catalog = TestCatalog::with(
        "now",
        ContextBinding::function(Value::String("tick".into())),
    );

    let called = eval_with_catalog("@now()", &catalog);
    assert_eq!(called.value, Value::String("tick".to_string()));
    assert!(called.diagnostics.is_empty());

    let bare = eval_with_catalog("@now", &catalog);
    assert_eq!(bare.value, Value::Null);
    assert!(bare.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_deref() == Some("FEL-CONTEXT-BINDING-CALL-REQUIRED")
    }));
}

/// Spec: fel-grammar.md §6.3.2 — `Value`-kind bindings are bare-name only;
/// calling them with `()` produces a `FEL-CONTEXT-BINDING-NOT-CALLABLE`
/// diagnostic and a null result. (Symmetric to the call-required diag
/// above for Function bindings.)
#[test]
fn catalog_value_binding_rejects_call_syntax() {
    let catalog = TestCatalog::with(
        "response",
        ContextBinding::value(Value::String("R-1".into())),
    );

    // Bare access works.
    let bare = eval_with_catalog("@response", &catalog);
    assert_eq!(bare.value, Value::String("R-1".to_string()));
    assert!(bare.diagnostics.is_empty());

    // `@response()` is rejected: Value bindings are not callable.
    let called = eval_with_catalog("@response()", &catalog);
    assert_eq!(called.value, Value::Null);
    assert!(
        called
            .diagnostics
            .iter()
            .any(|d| { d.code.as_deref() == Some("FEL-CONTEXT-BINDING-NOT-CALLABLE") }),
        "expected FEL-CONTEXT-BINDING-NOT-CALLABLE diagnostic, got {:?}",
        called.diagnostics
    );
}

/// Spec: fel-grammar.md §6.3.2 — Same as above for `Object`-kind
/// bindings. Postfix traversal works; calling with `()` does not.
#[test]
fn catalog_object_binding_rejects_call_syntax() {
    let catalog = TestCatalog::with(
        "effects",
        ContextBinding::object(object(&[("kind", Value::String("decision".into()))])),
    );

    // Postfix `.kind` works.
    let traversal = eval_with_catalog("@effects.kind", &catalog);
    assert_eq!(traversal.value, Value::String("decision".to_string()));
    assert!(traversal.diagnostics.is_empty());

    // `@effects()` is rejected: Object bindings are not callable.
    let called = eval_with_catalog("@effects()", &catalog);
    assert_eq!(called.value, Value::Null);
    assert!(
        called
            .diagnostics
            .iter()
            .any(|d| { d.code.as_deref() == Some("FEL-CONTEXT-BINDING-NOT-CALLABLE") }),
        "expected FEL-CONTEXT-BINDING-NOT-CALLABLE diagnostic, got {:?}",
        called.diagnostics
    );
}

#[test]
fn function_binding_receives_string_argument() {
    let catalog = ArgCatalog;

    let result = eval_with_catalog("@clock('utc')", &catalog);

    assert_eq!(result.value, Value::String("tick:utc".to_string()));
    assert!(result.diagnostics.is_empty());
}

#[test]
fn object_binding_supports_postfix_index_access() {
    let catalog = TestCatalog::with(
        "effects",
        ContextBinding::object(Value::Array(vec![object(&[(
            "outcomeRef",
            Value::String("first".to_string()),
        )])])),
    );

    let result = eval_with_catalog("@effects[1].outcomeRef", &catalog);

    assert_eq!(result.value, Value::String("first".to_string()));
    assert!(result.diagnostics.is_empty());
}

#[test]
fn value_binding_rejects_postfix_index_access() {
    let catalog = TestCatalog::with(
        "ids",
        ContextBinding::value(Value::Array(vec![Value::String("a".to_string())])),
    );

    let result = eval_with_catalog("@ids[1]", &catalog);

    assert_eq!(result.value, Value::Null);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_deref() == Some("FEL-CONTEXT-BINDING-PATH")
            && diagnostic.message == "context binding @ids does not support postfix traversal"
    }));
}

#[test]
fn host_binding_fixture_files_match_harness() {
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("conformance")
        .join("host-bindings");
    let mut fixture_paths: Vec<_> = fs::read_dir(&fixture_dir)
        .expect("host-binding fixture directory should exist")
        .map(|entry| entry.expect("fixture dir entry should be readable").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect();
    fixture_paths.sort();
    assert!(
        !fixture_paths.is_empty(),
        "host-binding fixture directory should not be empty"
    );

    for path in fixture_paths {
        let fixture: JsonValue = serde_json::from_str(
            &fs::read_to_string(&path).expect("host-binding fixture should be readable"),
        )
        .unwrap_or_else(|error| panic!("{} should be valid JSON: {error}", path.display()));
        let catalog = fixture_catalog(&fixture);
        let result = eval_with_catalog(
            fixture["expression"]
                .as_str()
                .expect("fixture expression should be string"),
            &catalog,
        );

        assert_eq!(
            fel_to_json(&result.value),
            fixture["expected"]["value"],
            "{} value mismatch",
            path.display()
        );

        let diagnostic_codes: Vec<&str> = result
            .diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.code.as_deref())
            .collect();
        let expected_codes: Vec<&str> = fixture["expected"]["diagnostics"]
            .as_array()
            .expect("fixture expected diagnostics should be an array")
            .iter()
            .filter_map(|diagnostic| diagnostic["code"].as_str())
            .collect();
        assert_eq!(
            diagnostic_codes,
            expected_codes,
            "{} diagnostic-code mismatch",
            path.display()
        );
    }
}

#[test]
fn evaluate_without_catalog_preserves_environment_context() {
    let expr = parse("@legacy").expect("legacy context expression should parse");
    let env = LegacyContextEnv;

    let result = evaluate(&expr, &env);

    assert_eq!(result.value, Value::String("from-env".to_string()));
    assert!(result.diagnostics.is_empty());
}

#[test]
fn grammar_reserved_context_uses_environment_when_catalog_is_active() {
    let expr = parse("@current").expect("reserved context expression should parse");
    let env = LegacyContextEnv;
    let catalog = EmptyCatalog;

    let result = evaluate_with_catalog(&expr, &env, EvaluatorOptions::default(), &catalog);

    assert_eq!(result.value, Value::String("reserved-current".to_string()));
    assert!(result.diagnostics.is_empty());
}

/// Spec: fel-grammar.md §6.3.4 — `@index` and `@count` are reserved
/// context names alongside `@current`. When the catalog is active but
/// doesn't bind these names, lookup falls through to the environment.
/// Symmetric to the `@current` test above. Addresses env/host swarm
/// review FUT-11.
struct ReservedNamesEnv;
impl Environment for ReservedNamesEnv {
    fn resolve_field(
        &self,
        _root: Option<&str>,
        _path: &[PathSegment],
    ) -> Result<Value, fel_core::FieldError> {
        Ok(Value::Null)
    }
    fn resolve_context(&self, name: &str, _arg: Option<&str>, _tail: &[String]) -> Value {
        match name {
            "index" => Value::Number(rust_decimal::Decimal::from(3)),
            "count" => Value::Number(rust_decimal::Decimal::from(10)),
            _ => Value::Null,
        }
    }
}

#[test]
fn reserved_at_index_falls_through_to_environment() {
    let expr = parse("@index").expect("reserved @index parses");
    let env = ReservedNamesEnv;
    let catalog = EmptyCatalog;
    let result = evaluate_with_catalog(&expr, &env, EvaluatorOptions::default(), &catalog);
    assert_eq!(result.value, Value::Number(rust_decimal::Decimal::from(3)));
    assert!(result.diagnostics.is_empty());
}

#[test]
fn reserved_at_count_falls_through_to_environment() {
    let expr = parse("@count").expect("reserved @count parses");
    let env = ReservedNamesEnv;
    let catalog = EmptyCatalog;
    let result = evaluate_with_catalog(&expr, &env, EvaluatorOptions::default(), &catalog);
    assert_eq!(result.value, Value::Number(rust_decimal::Decimal::from(10)));
    assert!(result.diagnostics.is_empty());
}
