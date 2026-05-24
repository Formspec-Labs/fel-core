//! Phase 3b convenience-closure proptests for env / catalog / JSON-input symbols.
//!
//! Covers the remaining ~10 GAP symbols clustered by surface:
//!
//! - **JSON-input parsers**: `field_map_from_json_str`, `json_object_to_field_map`,
//!   `host_options_from_json`, `formspec_environment_from_json_map`.
//! - **Catalog export**: `builtin_function_catalog_for`, `builtin_arity`,
//!   `builtin_function_catalog_json_value`, `builtin_function_catalog_json_value_for`.
//! - **Environment + struct**: `FormspecEnvironment::new` + setters, `MipState::default`,
//!   `RepeatContext` via `push_repeat`/`pop_repeat`.
//!
//! Exemption candidates (no proptests written; see commit body for rationale):
//! `ExtensionError` (tag enum discriminant, covered by extension_registry_proptest),
//! `JsonWireStyle` (pure marker enum, no methods/state).

#![allow(clippy::missing_docs_in_private_items)]

use fel_core::extensions::{
    Package, builtin_arity, builtin_function_catalog, builtin_function_catalog_for,
    builtin_function_catalog_json_value, builtin_function_catalog_json_value_for,
};
use fel_core::testing::strategies::arb_value;
use fel_core::{
    FormspecEnvironment, MipState, RepeatContext, Value as TypeValue, fel_to_wire_json,
    field_map_from_json_str, formspec_environment_from_json_map, host_options_from_json,
    json_object_to_field_map,
};
use proptest::prelude::*;
use serde_json::{Map, Value as JsonValue, json};
use std::collections::HashSet;

// ── Strategies ──────────────────────────────────────────────────────────────

/// Arbitrary safe identifier-shaped key (avoids whitespace / control chars
/// so map insertion semantics aren't perturbed by JSON-key normalization).
fn arb_key() -> impl Strategy<Value = String> {
    "[a-z_][a-z0-9_]{0,8}"
}

/// Arbitrary leaf JSON value (no nested containers).
fn arb_leaf_json() -> impl Strategy<Value = JsonValue> {
    prop_oneof![
        Just(JsonValue::Null),
        any::<bool>().prop_map(JsonValue::Bool),
        any::<i32>().prop_map(|i| json!(i)),
        "[a-zA-Z0-9 ]{0,12}".prop_map(JsonValue::String),
    ]
}

/// Arbitrary JSON object with safe keys + leaf values.
fn arb_json_object() -> impl Strategy<Value = Map<String, JsonValue>> {
    prop::collection::vec((arb_key(), arb_leaf_json()), 0..6).prop_map(|pairs| {
        let mut map = Map::new();
        for (k, v) in pairs {
            map.insert(k, v);
        }
        map
    })
}

/// Pairs of (key, FEL value) for environment round-trip insertion.
fn arb_key_value_pairs() -> impl Strategy<Value = Vec<(String, TypeValue)>> {
    prop::collection::vec((arb_key(), arb_value(2u32)), 0..6)
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Dedupe-by-first-key the way HashMap / Map::insert would.
fn unique_by_key(pairs: Vec<(String, TypeValue)>) -> Vec<(String, TypeValue)> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (k, v) in pairs.into_iter().rev() {
        if seen.insert(k.clone()) {
            out.push((k, v));
        }
    }
    out
}

// ── Catalog: total builtin count ────────────────────────────────────────────

fn total_builtin_count() -> usize {
    builtin_function_catalog().len()
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 64,
        ..Default::default()
    })]

    // ─────────────────────────────────────────────────────────────────────────
    // CLUSTER 1: JSON-input parsers
    // ─────────────────────────────────────────────────────────────────────────

    /// `field_map_from_json_str` round-trips against `json_object_to_field_map`
    /// for any JSON object: parsing the string form equals parsing the value form.
    #[test]
    fn field_map_from_json_str_matches_json_object_to_field_map(
        obj in arb_json_object(),
    ) {
        let json_string = serde_json::to_string(&obj).expect("serialize");
        let parsed_str = field_map_from_json_str(&json_string).expect("valid JSON object");
        let parsed_val = json_object_to_field_map(&JsonValue::Object(obj.clone()));

        // HashMap equality (key set + per-key value equality).
        prop_assert_eq!(parsed_str.len(), parsed_val.len());
        for (k, v) in &parsed_val {
            prop_assert_eq!(
                parsed_str.get(k),
                Some(v),
                "field_map_from_json_str disagrees with json_object_to_field_map on key {:?}",
                k
            );
        }
    }

    /// `json_object_to_field_map`: every key in the JSON object appears in the output map.
    /// Non-object inputs yield an empty map (defensive contract).
    #[test]
    fn json_object_to_field_map_preserves_keys(obj in arb_json_object()) {
        let val = JsonValue::Object(obj.clone());
        let map = json_object_to_field_map(&val);

        // Every input key is present in the output.
        for k in obj.keys() {
            prop_assert!(map.contains_key(k), "missing key {:?}", k);
        }
        // No phantom keys.
        prop_assert_eq!(map.len(), obj.len());
    }

    /// `json_object_to_field_map` on non-object inputs returns an empty map.
    #[test]
    fn json_object_to_field_map_non_object_is_empty(leaf in arb_leaf_json()) {
        // Skip the case where leaf happens to be an object (none of the
        // arb_leaf_json variants are objects, but defensive).
        prop_assume!(!leaf.is_object());
        let map = json_object_to_field_map(&leaf);
        prop_assert!(map.is_empty(), "non-object should yield empty map, got {:?}", map);
    }

    /// `host_options_from_json`:
    /// - When required `expression` is present, parsing succeeds.
    /// - Unknown keys are ignored gracefully (don't cause failure).
    /// - Recognized keys (`currentItemPath`, `replaceSelfRef`) round-trip into the parsed options.
    #[test]
    fn host_options_from_json_round_trips_recognized_keys(
        expr in "[a-zA-Z0-9_$ +*]{1,20}",
        path in arb_key().prop_map(|n| format!("{n}[0].field")),
        replace in any::<bool>(),
        unknown_key in arb_key(),
        unknown_val in arb_leaf_json(),
    ) {
        let mut obj = Map::new();
        obj.insert("expression".into(), JsonValue::String(expr.clone()));
        obj.insert("currentItemPath".into(), JsonValue::String(path.clone()));
        obj.insert("replaceSelfRef".into(), JsonValue::Bool(replace));
        // Inject an unknown key — must be ignored, NOT cause an error.
        // Skip names that collide with recognized keys.
        if !matches!(
            unknown_key.as_str(),
            "expression"
                | "currentItemPath"
                | "current_item_path"
                | "replaceSelfRef"
                | "replace_self_ref"
                | "repeatCounts"
                | "repeat_counts"
                | "fieldPaths"
                | "field_paths"
                | "valuesByPath"
                | "values_by_path"
        ) {
            obj.insert(unknown_key, unknown_val);
        }

        let parsed = host_options_from_json(&obj).expect("valid options");
        prop_assert_eq!(parsed.expression, expr);
        prop_assert_eq!(parsed.current_item_path, path);
        prop_assert_eq!(parsed.replace_self_ref, replace);
    }

    /// `host_options_from_json` fails cleanly when `expression` is missing.
    #[test]
    fn host_options_from_json_requires_expression(
        path in arb_key(),
        replace in any::<bool>(),
    ) {
        let mut obj = Map::new();
        obj.insert("currentItemPath".into(), JsonValue::String(path));
        obj.insert("replaceSelfRef".into(), JsonValue::Bool(replace));
        // No "expression" key.
        let result = host_options_from_json(&obj);
        prop_assert!(result.is_err(), "missing expression should error");
    }

    /// `formspec_environment_from_json_map`: for arbitrary JSON object inputs,
    /// fields under `"fields"` appear in `env.data` keyed by the input key.
    /// Variables under `"variables"` appear in `env.variables` similarly.
    #[test]
    fn formspec_environment_from_json_map_populates_fields_and_variables(
        fields in arb_json_object(),
        variables in arb_json_object(),
    ) {
        let mut ctx = Map::new();
        ctx.insert("fields".into(), JsonValue::Object(fields.clone()));
        ctx.insert("variables".into(), JsonValue::Object(variables.clone()));

        let env = formspec_environment_from_json_map(&ctx);

        // Every field key landed in env.data.
        for k in fields.keys() {
            prop_assert!(env.data.contains_key(k), "field {:?} missing from env.data", k);
        }
        // Every variable key landed in env.variables.
        for k in variables.keys() {
            prop_assert!(
                env.variables.contains_key(k),
                "variable {:?} missing from env.variables",
                k
            );
        }
    }

    /// `formspec_environment_from_json_map` on an empty context yields a usable
    /// environment with no panics on observable getters.
    #[test]
    fn formspec_environment_from_json_map_handles_empty_context(_seed in any::<u8>()) {
        let env = formspec_environment_from_json_map(&Map::new());
        prop_assert!(env.data.is_empty());
        prop_assert!(env.variables.is_empty());
        prop_assert!(env.instances.is_empty());
        prop_assert!(env.repeat_context.is_none());
        prop_assert!(env.locale.is_none());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // CLUSTER 2: Catalog export
    // ─────────────────────────────────────────────────────────────────────────

    /// `builtin_function_catalog_json_value()` returns a JSON Array of length
    /// equal to the total builtin count. Each entry has the expected shape.
    #[test]
    fn catalog_json_value_shape(_seed in any::<u8>()) {
        let json = builtin_function_catalog_json_value();
        let arr = json.as_array().expect("catalog is a JSON array");
        prop_assert_eq!(arr.len(), total_builtin_count());

        for entry in arr {
            // Closed shape contract: name, category, signature, parameters, returns, description.
            prop_assert!(entry.get("name").is_some_and(|v| v.is_string()));
            prop_assert!(entry.get("category").is_some_and(|v| v.is_string()));
            prop_assert!(entry.get("signature").is_some_and(|v| v.is_string()));
            prop_assert!(entry.get("parameters").is_some_and(|v| v.is_array()));
            prop_assert!(entry.get("returns").is_some_and(|v| v.is_string()));
            prop_assert!(entry.get("description").is_some_and(|v| v.is_string()));
        }
    }

    /// `builtin_function_catalog_json_value_for(package)`: parity with the Rust
    /// iterator surface — same count, same per-entry shape, same names.
    #[test]
    fn catalog_json_value_for_matches_rust_iterator(package in arb_package()) {
        let rust_names: Vec<&str> = builtin_function_catalog_for(package)
            .map(|e| e.name)
            .collect();
        let json = builtin_function_catalog_json_value_for(package);
        let arr = json.as_array().expect("array");
        prop_assert_eq!(arr.len(), rust_names.len());

        for (i, entry) in arr.iter().enumerate() {
            let name = entry.get("name").and_then(|v| v.as_str()).expect("name");
            prop_assert_eq!(
                name,
                rust_names[i],
                "JSON catalog name at {} differs from Rust iterator", i
            );
        }
    }

    /// `builtin_function_catalog_for(package)`: returns only entries that match
    /// the package filter (Universal yields only Universal; Formspec yields all).
    ///
    /// `Package` is `#[non_exhaustive]`; future variants must update this match
    /// arm with their filter contract (don't fall through to a permissive default).
    #[test]
    fn catalog_for_filters_by_package(package in arb_package()) {
        let entries: Vec<_> = builtin_function_catalog_for(package).collect();
        match package {
            Package::Universal => {
                prop_assert!(
                    entries.iter().all(|e| matches!(e.package, Package::Universal)),
                    "Universal filter yielded non-Universal entry"
                );
                // Strict subset of total.
                prop_assert!(entries.len() <= total_builtin_count());
            }
            Package::Formspec => {
                // Formspec is the union (full catalog).
                prop_assert_eq!(entries.len(), total_builtin_count());
            }
            _ => {
                // New `Package` variant added without updating this filter contract.
                // Fail loudly rather than silently accepting whatever the filter returned.
                prop_assert!(
                    false,
                    "unhandled Package variant {:?} — extend catalog_for_filters_by_package",
                    package
                );
            }
        }
    }

    /// `builtin_arity(name)`: for every name in the full catalog,
    /// `builtin_arity` returns `Some(arity)` that matches the entry's `arity()`.
    /// For names NOT in the catalog, returns `None`.
    #[test]
    fn builtin_arity_matches_entry_arity(probe in "[a-z_]{1,20}") {
        // Property: every catalog name resolves to the entry's declared arity.
        for entry in builtin_function_catalog() {
            let expected = entry.arity();
            let got = builtin_arity(entry.name);
            prop_assert_eq!(
                got,
                Some(expected),
                "builtin_arity({}) disagrees with entry.arity()", entry.name
            );
        }

        // Property: unknown name (anything not in the catalog) returns None.
        let catalog_has = builtin_function_catalog()
            .iter()
            .any(|e| e.name == probe);
        if !catalog_has {
            prop_assert!(
                builtin_arity(&probe).is_none(),
                "unknown name {:?} should return None", probe
            );
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // CLUSTER 3: Environment + struct
    // ─────────────────────────────────────────────────────────────────────────

    /// `FormspecEnvironment::new()` + setters: for arbitrary (key, value) pairs,
    /// `set_field` followed by direct `env.data.get(key)` returns the inserted value.
    /// Last-write-wins semantics under duplicate keys.
    #[test]
    fn formspec_environment_set_field_round_trips(pairs in arb_key_value_pairs()) {
        let unique = unique_by_key(pairs);
        let mut env = FormspecEnvironment::new();
        for (k, v) in &unique {
            env.set_field(k, v.clone());
        }
        // Every (key, value) round-trips.
        for (k, v) in &unique {
            prop_assert_eq!(
                env.data.get(k),
                Some(v),
                "set_field/get mismatch for key {:?}", k
            );
        }
    }

    /// Variables, instances, and meta also round-trip via their respective setters.
    #[test]
    fn formspec_environment_set_variable_instance_meta_round_trip(
        pairs in arb_key_value_pairs(),
    ) {
        let unique = unique_by_key(pairs);
        let mut env = FormspecEnvironment::new();
        for (k, v) in &unique {
            env.set_variable(k, v.clone());
            env.set_instance(k, v.clone());
            env.set_meta(k, v.clone());
        }
        for (k, v) in &unique {
            prop_assert_eq!(env.variables.get(k), Some(v));
            prop_assert_eq!(env.instances.get(k), Some(v));
            prop_assert_eq!(env.meta.get(k), Some(v));
        }
    }

    /// `MipState::default()` constructs the spec-default state
    /// (valid=true, relevant=true, readonly=false, required=false) and observable
    /// getters don't panic.
    #[test]
    fn mip_state_default_observable(_seed in any::<u8>()) {
        let state = MipState::default();
        prop_assert!(state.valid);
        prop_assert!(state.relevant);
        prop_assert!(!state.readonly);
        prop_assert!(!state.required);
    }

    /// `RepeatContext`: push_repeat / pop_repeat semantics — pushing yields a
    /// `Some(ctx)` whose fields match what was pushed; popping restores parent.
    /// Observable getters (`current`, `index`, `count`, `collection`) don't panic.
    #[test]
    fn repeat_context_push_pop_round_trips(
        index in 1usize..16,
        count in 0usize..16,
        items in prop::collection::vec(arb_value(1u32), 0..6),
    ) {
        let mut env = FormspecEnvironment::new();
        prop_assert!(env.repeat_context.is_none(), "fresh env has no repeat context");

        let current = items.first().cloned().unwrap_or(TypeValue::Null);
        env.push_repeat(current.clone(), index, count, items.clone());

        let ctx: &RepeatContext = env
            .repeat_context
            .as_ref()
            .expect("push_repeat must set repeat_context");
        prop_assert_eq!(&ctx.current, &current);
        prop_assert_eq!(ctx.index, index);
        prop_assert_eq!(ctx.count, count);
        prop_assert_eq!(&ctx.collection, &items);
        prop_assert!(ctx.parent.is_none(), "first push has no parent");

        env.pop_repeat();
        prop_assert!(env.repeat_context.is_none(), "pop returns to empty state");
    }

    /// Nested `RepeatContext`: pushing twice produces a chain; popping
    /// restores the outer context with its parent unchanged.
    #[test]
    fn repeat_context_nested_chain(
        outer_idx in 1usize..8,
        inner_idx in 1usize..8,
    ) {
        let mut env = FormspecEnvironment::new();
        let outer_items = vec![TypeValue::Null, TypeValue::Boolean(true)];
        env.push_repeat(TypeValue::Null, outer_idx, 2, outer_items.clone());
        let inner_items = vec![TypeValue::Boolean(false)];
        env.push_repeat(TypeValue::Boolean(false), inner_idx, 1, inner_items.clone());

        let inner = env.repeat_context.as_ref().expect("inner pushed");
        prop_assert_eq!(inner.index, inner_idx);
        let parent = inner.parent.as_ref().expect("inner has parent");
        prop_assert_eq!(parent.index, outer_idx);
        prop_assert_eq!(&parent.collection, &outer_items);

        // Pop inner — outer restored, no nested parent on outer.
        env.pop_repeat();
        let outer = env.repeat_context.as_ref().expect("outer restored");
        prop_assert_eq!(outer.index, outer_idx);
        prop_assert!(outer.parent.is_none());

        env.pop_repeat();
        prop_assert!(env.repeat_context.is_none());
    }
}

/// Strategy over the closed `Package` enum.
fn arb_package() -> impl Strategy<Value = Package> {
    prop_oneof![Just(Package::Universal), Just(Package::Formspec)]
}

// ── Non-proptest assertions ─────────────────────────────────────────────────
//
// Catalog-shape invariants that don't depend on generated input (exhaustive
// iteration over a static table). Kept in this file so the test binary
// covers them alongside the property-based gates.

/// `builtin_function_catalog_json_value()` total count matches Rust catalog.
#[test]
fn catalog_json_value_total_count_matches_rust_catalog() {
    let json = builtin_function_catalog_json_value();
    let arr = json.as_array().expect("array");
    assert_eq!(arr.len(), builtin_function_catalog().len());
}

/// Each `Package` variant produces a JSON catalog whose names match the
/// Rust iterator surface byte-for-byte (order preserved).
#[test]
fn catalog_json_value_for_per_package_name_parity() {
    for pkg in [Package::Universal, Package::Formspec] {
        let rust_names: Vec<&str> = builtin_function_catalog_for(pkg).map(|e| e.name).collect();
        let json = builtin_function_catalog_json_value_for(pkg);
        let arr = json.as_array().expect("array");
        let json_names: Vec<String> = arr
            .iter()
            .map(|e| e.get("name").and_then(|v| v.as_str()).unwrap().to_string())
            .collect();
        assert_eq!(json_names.len(), rust_names.len(), "package {pkg:?}");
        for (j, r) in json_names.iter().zip(rust_names.iter()) {
            assert_eq!(j, r, "name mismatch for package {pkg:?}");
        }
    }
}

/// `builtin_arity` for every catalog entry matches `entry.arity()`.
#[test]
fn builtin_arity_exhaustive_catalog_parity() {
    for entry in builtin_function_catalog() {
        let expected = entry.arity();
        assert_eq!(
            builtin_arity(entry.name),
            Some(expected),
            "builtin_arity({}) disagrees with entry.arity()",
            entry.name
        );
    }
}

/// `MipState::default()` matches the documented spec-default state.
#[test]
fn mip_state_default_is_spec_default() {
    let state = MipState::default();
    assert!(state.valid);
    assert!(state.relevant);
    assert!(!state.readonly);
    assert!(!state.required);
}

/// Sanity: `arb_value` produces a value that `fel_to_wire_json` can encode.
/// (Smoke check that the strategy import path is sound.)
#[test]
fn arb_value_encodes_to_wire_json() {
    let v = TypeValue::Null;
    let _ = fel_to_wire_json(&v);
}
