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
