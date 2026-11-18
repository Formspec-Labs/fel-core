//! Instance paths and ambient fields read by row expressions.
use fel_core::{dependencies_to_json_value, extract_dependencies, parse};
use serde_json::json;

#[test]
fn predicates_read_columns_from_their_own_rows() {
    for function in ["countWhere", "some", "every"] {
        let source = format!("{function}(@instance('survey').rows, $.flag = $filter)");
        let deps = extract_dependencies(&parse(&source).unwrap());
        assert_eq!(
            dependencies_to_json_value(&deps)["instanceReads"],
            json!({"survey":["rows", "rows[*].flag"]})
        );
        assert_eq!(deps.fields, ["filter".to_owned()].into());
    }
}

#[test]
fn keyed_lookup_reads_key_and_selected_column() {
    let source = "@instance('survey').rows[identity.id = @instance('criteria').id].value";
    let deps = extract_dependencies(&parse(source).unwrap());
    assert_eq!(
        dependencies_to_json_value(&deps)["instanceReads"],
        json!({"criteria":["id"], "survey":["rows","rows[*].identity.id","rows[*].value"]})
    );
    assert!(deps.fields.is_empty());
}

#[test]
fn aliases_and_nested_predicates_preserve_row_scope() {
    let source =
        "let rows = @instance('survey').rows in some(rows, every($.children, $.flag) and $.active)";
    let deps = extract_dependencies(&parse(source).unwrap());
    assert_eq!(
        dependencies_to_json_value(&deps)["instanceReads"],
        json!({"survey":["rows","rows[*].active","rows[*].children","rows[*].children[*].flag"]})
    );
    assert!(deps.fields.is_empty());
}

#[test]
fn literal_instance_function_paths_match_context_syntax() {
    let source = "instance('survey', 'totals.count') + @instance('survey').totals.count";
    let deps = extract_dependencies(&parse(source).unwrap());
    assert_eq!(
        dependencies_to_json_value(&deps)["instanceReads"],
        json!({"survey":["totals.count"]})
    );
}

#[test]
fn conditional_rows_keep_the_possible_instance_paths() {
    for source in [
        "coalesce(@instance('survey').rows, @instance('archive').rows)",
        "if($chosen, @instance('survey').rows, @instance('archive').rows)",
    ] {
        let expr = parse(&format!("some({source}, $.active)")).unwrap();
        let deps = extract_dependencies(&expr);
        assert_eq!(
            dependencies_to_json_value(&deps)["instanceReads"],
            json!({"archive":["rows","rows[*].active"],"survey":["rows","rows[*].active"]})
        );
    }
}

#[test]
fn constructed_rows_project_only_the_selected_member() {
    let source = "some([{row: @instance('a').primary}, {row: @instance('b').backup}], $.row.id)";
    let deps = extract_dependencies(&parse(source).unwrap());
    assert_eq!(
        dependencies_to_json_value(&deps)["instanceReads"],
        json!({"a":["primary","primary.id"],"b":["backup","backup.id"]})
    );
    assert!(deps.fields.is_empty());
}

#[test]
fn projection_preserves_element_paths_through_nested_arrays() {
    let source = "some(@instance('a').rows[*].children, every($, $.id))";
    let deps = extract_dependencies(&parse(source).unwrap());
    assert_eq!(
        dependencies_to_json_value(&deps)["instanceReads"],
        json!({"a":["rows[*].children","rows[*].children[*].id"]})
    );
}

#[test]
fn unresolved_sources_keep_ambient_reads_and_rebind_dollar() {
    let deps = extract_dependencies(&parse("some(x_rows($selector), $.name = $filter)").unwrap());
    assert_eq!(deps.fields, ["selector", "filter"].map(String::from).into());
    assert!(!deps.has_self_ref);
}

#[test]
fn current_field_reads_differ_from_nested_predicate_columns() {
    let source = "$.config.title ?? some($.rows, $.selected)";
    let deps = extract_dependencies(&parse(source).unwrap());
    assert_eq!(
        deps.self_reads,
        ["config.title", "rows", "rows[*].selected"]
            .map(String::from)
            .into()
    );
    assert!(deps.has_self_ref);
    assert!(deps.fields.is_empty());
}

#[test]
fn invalid_postfix_keeps_the_field_read_it_attempts() {
    for expression in ["($rows[*].value).missing", "($rows[*].value).missing[*]"] {
        let deps = extract_dependencies(&parse(expression).unwrap());
        assert_eq!(
            deps.fields,
            ["rows[*].value".to_owned()].into(),
            "{expression}"
        );
    }
}
