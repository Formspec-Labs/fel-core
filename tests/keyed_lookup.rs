use fel_core::{
    EvalBudget, EvaluatorOptions, FormspecEnvironment, MapEnvironment, evaluate, evaluate_with,
    extract_dependencies, parse, print_expr,
};
use serde_json::json;

#[test]
fn keyed_lookup_is_a_single_row_selector() {
    let expr = parse("@instance('list').rows[id = subject('id')].name").unwrap();
    let mut env = FormspecEnvironment::default();
    env.subject_keys
        .insert("id".into(), fel_core::Value::String("A".into()));
    for (instance, expected, diagnostic) in [
        (
            json!({"rows": [{"id":"B"}, {"id":"A", "name":"Alpha"}]}),
            json!("Alpha"),
            None,
        ),
        (
            json!({"rows": [{"id":"B", "name":"Beta"}]}),
            json!(null),
            None,
        ),
        (
            json!({"rows": [{"id":"A", "name":"Alpha"}, {"id":"A", "name":"Other"}]}),
            json!(null),
            Some("FEL_AMBIGUOUS_ROW_LOOKUP"),
        ),
        (json!(null), json!(null), None),
    ] {
        env.instances
            .insert("list".into(), fel_core::json_to_fel(&instance));
        let result = evaluate(&expr, &env);
        assert_eq!(fel_core::fel_to_json(&result.value), expected);
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|d| d.code.as_deref())
                .collect::<Vec<_>>(),
            diagnostic.map(Some).into_iter().collect::<Vec<_>>(),
        );
    }
}

#[test]
fn keyed_lookup_uses_row_paths_and_ambient_dependencies() {
    let source = "@instance('list').rows[identity.id = (subject('id') ?? $id ?? @instance('fallback').id)].name";
    let expr = parse(source).unwrap();
    assert_eq!(parse(&print_expr(&expr)).unwrap(), expr);
    let deps = extract_dependencies(&expr);
    assert_eq!(deps.fields, ["id".to_string()].into());
    assert_eq!(
        deps.instance_refs,
        ["list".to_string(), "fallback".to_string()].into()
    );
    let dynamic = extract_dependencies(&parse("$rows[id = subject($keyName)]").unwrap());
    assert_eq!(
        dynamic.fields,
        ["rows", "rows[*].id", "keyName"].map(String::from).into()
    );
}

#[test]
fn keyed_lookup_records_the_key_and_result_paths() {
    let expr = parse("$groups[*].rows[identity.id = $sel].display.label").unwrap();
    let deps = extract_dependencies(&expr);
    assert_eq!(
        deps.fields,
        [
            "groups[*].rows",
            "groups[*].rows[*].identity.id",
            "groups[*].rows[*].display.label",
            "sel"
        ]
        .map(String::from)
        .into()
    );
    assert!(deps.has_wildcard);
}

#[test]
fn keyed_lookup_skips_null_keys_and_keeps_fel_equality() {
    let source =
        r#"[{}, {id: null, name: 'wrong'}, {id: '1'}, {id: 1, name: 'right'}][id = $key].name"#;
    let expr = parse(source).unwrap();
    let mut env = MapEnvironment::default();
    assert_eq!(evaluate(&expr, &env).value, fel_core::Value::Null);
    let nested = parse("[{identity: {id: 'A'}, name: 'Alpha'}][identity.id == 'A'].name").unwrap();
    assert_eq!(
        evaluate(&nested, &env).value,
        fel_core::Value::String("Alpha".into())
    );
    env.fields
        .insert("key".into(), fel_core::Value::Number(1.into()));
    assert_eq!(
        evaluate(&expr, &env).value,
        fel_core::Value::String("right".into())
    );
}

#[test]
fn keyed_lookup_rejects_incomplete_or_non_keyed_selectors() {
    for source in [
        "$rows[id = ]",
        "$rows[id]",
        "$rows[id != 'A']",
        "$rows[id < 3]",
        "$rows[id. = 'A']",
    ] {
        let error = parse(source).unwrap_err().to_string();
        assert!(error.contains("key = value"), "{source}: {error}");
    }
}

#[test]
fn keyed_lookup_budget_cannot_return_a_partial_match() {
    let expression = parse("$rows[id = 'A'].name").unwrap();
    let mut env = MapEnvironment::default();
    let mut rows = vec![json!({"id":"A", "name":"Alpha"})];
    rows.extend(std::iter::repeat_n(json!({"id":"B"}), 12));
    env.fields
        .insert("rows".into(), fel_core::json_to_fel(&json!(rows)));
    let result = evaluate_with(
        &expression,
        &env,
        EvaluatorOptions {
            budget: EvalBudget {
                max_steps: 8,
                ..EvalBudget::unlimited()
            },
            ..EvaluatorOptions::default()
        },
    );
    assert_eq!(result.value, fel_core::Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("budget exceeded"))
    );
}
