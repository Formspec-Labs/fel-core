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
        dependencies_to_json_value(&deps)["selfReads"],
        json!(["config.title", "rows", "rows[*].selected"])
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

#[test]
fn literal_instance_keys_stay_distinct_from_array_subscripts_and_wildcards() {
    use fel_core::{FormspecEnvironment, evaluate, json_to_fel};

    let mut env = FormspecEnvironment::new();
    env.set_instance(
        "source",
        json_to_fel(&json!({
            "rows": [{"code": "array-member"}],
            "rows[1]": {"code": "literal-key"},
            "rows[*]": {"code": "literal-star"},
        })),
    );

    let literal = parse("instance('source', 'rows[1].code')").unwrap();
    let indexed = parse("@instance('source').rows[1].code").unwrap();
    let literal_star = parse("instance('source', 'rows[*].code')").unwrap();
    let projected = parse("some(@instance('source').rows, $.code = 'x')").unwrap();

    assert_eq!(
        evaluate(&literal, &env).value.as_string(),
        Some("literal-key")
    );
    assert_eq!(
        evaluate(&indexed, &env).value.as_string(),
        Some("array-member")
    );
    assert_eq!(
        evaluate(&literal_star, &env).value.as_string(),
        Some("literal-star")
    );

    let literal_deps = extract_dependencies(&literal);
    let indexed_deps = extract_dependencies(&indexed);
    let literal_star_deps = extract_dependencies(&literal_star);
    let projected_deps = extract_dependencies(&projected);

    assert_ne!(literal_deps.instance_reads, indexed_deps.instance_reads);
    assert_eq!(
        dependencies_to_json_value(&literal_deps)["instanceReads"],
        json!({"source": ["[\"rows[1]\"].code"]})
    );
    assert_eq!(
        dependencies_to_json_value(&indexed_deps)["instanceReads"],
        json!({"source": ["rows[1].code"]})
    );
    assert_ne!(literal_star_deps.instance_reads, projected_deps.instance_reads);
    assert_eq!(
        dependencies_to_json_value(&literal_star_deps)["instanceReads"],
        json!({"source": ["[\"rows[*]\"].code"]})
    );
}

#[test]
fn postfix_and_predicates_on_literal_instance_keys_keep_typed_segments() {
    use fel_core::{ReadPath, ReadPathSegment};

    let chained = extract_dependencies(&parse("instance('source', 'rows[1]').code").unwrap());
    let indexed = extract_dependencies(&parse("instance('source', 'rows[1]')[1].code").unwrap());
    let pred = extract_dependencies(&parse("some(instance('source', 'rows[1]'), $.code)").unwrap());

    assert_eq!(
        chained.instance_reads["source"]
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![ReadPath::new(vec![
            ReadPathSegment::Key("rows[1]".into()),
            ReadPathSegment::Key("code".into()),
        ])]
    );
    assert_eq!(
        dependencies_to_json_value(&chained)["instanceReads"],
        json!({"source": ["[\"rows[1]\"].code"]})
    );
    assert_eq!(
        dependencies_to_json_value(&indexed)["instanceReads"],
        json!({"source": ["[\"rows[1]\"][1].code"]})
    );
    assert_eq!(
        dependencies_to_json_value(&pred)["instanceReads"],
        json!({"source": ["[\"rows[1]\"]", "[\"rows[1]\"][*].code"]})
    );
}

#[test]
fn current_row_reads_preserve_indexed_and_wildcard_suffixes() {
    use fel_core::{ReadPath, ReadPathSegment};

    let deps = extract_dependencies(
        &parse("@current.nested[1].missing + some(@current.nested, $.flag) + @current[1].bad")
            .unwrap(),
    );
    let paths: Vec<_> = deps
        .current_reads
        .iter()
        .map(ReadPath::to_canonical_string)
        .collect();
    assert_eq!(
        paths,
        vec![
            "[1].bad",
            "nested",
            "nested[*].flag",
            "nested[1].missing",
        ]
    );
    assert!(deps.current_reads.contains(&ReadPath::new(vec![
        ReadPathSegment::Key("nested".into()),
        ReadPathSegment::Index(1),
        ReadPathSegment::Key("missing".into()),
    ])));
}

#[test]
fn read_path_canonical_round_trip_handles_edge_cases() {
    use fel_core::{ReadPath, ReadPathSegment as S};

    let cases = [
        (vec![], ""),
        (vec![S::Key("".into())], "[\"\"]"),
        (vec![S::Key("a".into()), S::Key("".into())], "a[\"\"]"),
        (vec![S::Key("rows[1]".into()), S::Key("code".into())], "[\"rows[1]\"].code"),
        (vec![S::Key("rows".into()), S::Index(1), S::Key("code".into())], "rows[1].code"),
        (vec![S::Key("rows[*]".into()), S::Key("code".into())], "[\"rows[*]\"].code"),
        (vec![S::Key("rows".into()), S::Wildcard, S::Key("code".into())], "rows[*].code"),
        (vec![S::Key("a.b".into()), S::Key("c".into())], "[\"a.b\"].c"),
        (vec![S::Key("a".into()), S::Key("b".into()), S::Key("c".into())], "a.b.c"),
        (vec![S::Key("a".into()), S::Key("1".into())], "a.1"),
        (vec![S::Key("a".into()), S::Index(1)], "a[1]"),
        (vec![S::Key("a\"b\\c".into())], "[\"a\\\"b\\\\c\"]"),
        (vec![S::Wildcard, S::Key("code".into())], "[*].code"),
        (vec![S::Index(1), S::Key("code".into())], "[1].code"),
    ];
    let mut rendered = std::collections::BTreeSet::new();
    for (segments, expected) in cases {
        let path = ReadPath::new(segments);
        let text = path.to_canonical_string();
        assert_eq!(text, expected);
        assert_eq!(ReadPath::parse_canonical(&text).unwrap(), path);
        assert!(rendered.insert(text), "duplicate canonical encoding");
    }
}
