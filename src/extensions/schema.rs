//! JSON Schema emission and UI-facing catalog JSON for builtin functions.
#![allow(clippy::missing_docs_in_private_items)]

use super::catalog::{BUILTIN_FUNCTIONS, builtin_function_catalog_for};
use super::types::*;

/// Synthesize a human-readable signature string from structured catalog data.
///
/// Format: `name(p1, p2?, ...p3) -> returnType`
/// - Required parameter: `name`
/// - Optional parameter: `name?`
/// - Variadic parameter: `...name`
pub(crate) fn synthesize_signature(
    name: &str,
    parameters: &[Parameter],
    returns: FelType,
) -> String {
    let params: Vec<String> = parameters
        .iter()
        .map(|p| {
            if p.variadic {
                format!("...{}", p.name)
            } else if !p.required {
                format!("{}?", p.name)
            } else {
                p.name.to_string()
            }
        })
        .collect();
    format!("{}({}) -> {}", name, params.join(", "), returns.as_str())
}

/// Emit a single `Parameter` as its schema JSON object.
fn emit_parameter(p: &Parameter) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert("name".into(), serde_json::Value::String(p.name.into()));
    obj.insert(
        "type".into(),
        serde_json::Value::String(p.fel_type.as_str().into()),
    );
    if let Some(desc) = p.description {
        obj.insert("description".into(), serde_json::Value::String(desc.into()));
    }
    if !p.required {
        obj.insert("required".into(), serde_json::Value::Bool(false));
    }
    if p.variadic {
        obj.insert("variadic".into(), serde_json::Value::Bool(true));
    }
    if let Some(vals) = p.allowed_values {
        obj.insert(
            "enum".into(),
            serde_json::Value::Array(
                vals.iter()
                    .map(|v| serde_json::Value::String((*v).into()))
                    .collect(),
            ),
        );
    }
    serde_json::Value::Object(obj)
}

/// Emit a single `Example` as its schema JSON object.
///
/// Returns `None` when `result_json` is not valid JSON (omitted from emitted catalogs).
///
/// Invalid static examples are dropped silently; [`super::tests::result_json_parses_for_all_examples`]
/// guards the committed catalog source.
fn emit_example(ex: &Example) -> Option<serde_json::Value> {
    let result: serde_json::Value = serde_json::from_str(ex.result_json).ok()?;
    let mut obj = serde_json::Map::new();
    obj.insert(
        "expression".into(),
        serde_json::Value::String(ex.expression.into()),
    );
    obj.insert("result".into(), result);
    if let Some(note) = ex.note {
        obj.insert("note".into(), serde_json::Value::String(note.into()));
    }
    Some(serde_json::Value::Object(obj))
}

/// Emit a single `BuiltinFunctionCatalogEntry` as its `FunctionEntry` JSON object.
fn emit_function_entry(e: &BuiltinFunctionCatalogEntry) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert("name".into(), serde_json::Value::String(e.name.into()));
    obj.insert(
        "category".into(),
        serde_json::Value::String(e.category.into()),
    );
    obj.insert(
        "parameters".into(),
        serde_json::Value::Array(e.parameters.iter().map(emit_parameter).collect()),
    );
    obj.insert(
        "returns".into(),
        serde_json::Value::String(e.returns.as_str().into()),
    );
    if let Some(rd) = e.return_description {
        obj.insert(
            "returnDescription".into(),
            serde_json::Value::String(rd.into()),
        );
    }
    obj.insert(
        "description".into(),
        serde_json::Value::String(e.description.into()),
    );
    if let Some(nh) = e.null_handling {
        obj.insert("nullHandling".into(), serde_json::Value::String(nh.into()));
    }
    if e.since_version != "1.0" {
        obj.insert(
            "sinceVersion".into(),
            serde_json::Value::String(e.since_version.to_string()),
        );
    }
    // Emit `deterministic` when it differs from the default (true) or when the entry opts in to
    // explicit emission for canonical-schema clarity (`emit_deterministic_explicitly`).
    if !e.deterministic || e.emit_deterministic_explicitly {
        obj.insert(
            "deterministic".into(),
            serde_json::Value::Bool(e.deterministic),
        );
    }
    if e.short_circuit {
        obj.insert("shortCircuit".into(), serde_json::Value::Bool(true));
    }
    let examples: Vec<_> = e.examples.iter().filter_map(emit_example).collect();
    if !examples.is_empty() {
        obj.insert("examples".into(), serde_json::Value::Array(examples));
    }
    serde_json::Value::Object(obj)
}

/// Emit a single `BuiltinFunctionCatalogEntry` as its JSON object for UI-facing catalog
/// consumers. Identical to [`emit_function_entry`] but inserts a synthesized `"signature"`
/// string after `"category"` for display in autocomplete/highlight tooltips.
fn emit_function_entry_catalog(e: &BuiltinFunctionCatalogEntry) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert("name".into(), serde_json::Value::String(e.name.into()));
    obj.insert(
        "category".into(),
        serde_json::Value::String(e.category.into()),
    );
    obj.insert(
        "signature".into(),
        serde_json::Value::String(synthesize_signature(e.name, e.parameters, e.returns)),
    );
    obj.insert(
        "parameters".into(),
        serde_json::Value::Array(e.parameters.iter().map(emit_parameter).collect()),
    );
    obj.insert(
        "returns".into(),
        serde_json::Value::String(e.returns.as_str().into()),
    );
    if let Some(rd) = e.return_description {
        obj.insert(
            "returnDescription".into(),
            serde_json::Value::String(rd.into()),
        );
    }
    obj.insert(
        "description".into(),
        serde_json::Value::String(e.description.into()),
    );
    if let Some(nh) = e.null_handling {
        obj.insert("nullHandling".into(), serde_json::Value::String(nh.into()));
    }
    if e.since_version != "1.0" {
        obj.insert(
            "sinceVersion".into(),
            serde_json::Value::String(e.since_version.to_string()),
        );
    }
    if !e.deterministic || e.emit_deterministic_explicitly {
        obj.insert(
            "deterministic".into(),
            serde_json::Value::Bool(e.deterministic),
        );
    }
    if e.short_circuit {
        obj.insert("shortCircuit".into(), serde_json::Value::Bool(true));
    }
    let examples: Vec<_> = e.examples.iter().filter_map(emit_example).collect();
    if !examples.is_empty() {
        obj.insert("examples".into(), serde_json::Value::Array(examples));
    }
    serde_json::Value::Object(obj)
}

/// The `x-generated-from` marker embedded in the emitted schema.
const X_GENERATED_FROM: &str = "fel-core (https://github.com/Formspec-org/fel-core) — regenerate via `cargo run -p fel-core --bin emit-fel-schema > formspec/schemas/fel-functions.schema.json`";

/// Emit the FEL function catalog as a `serde_json::Value` matching the schema at
/// `formspec/schemas/fel-functions.schema.json`.
///
/// The emitted value is byte-identical (up to JSON semantic equivalence — key order in
/// objects doesn't matter) to the canonical schema file. The round-trip test in
/// `tests/schema_round_trip.rs` enforces this invariant.
pub fn emit_schema_json() -> serde_json::Value {
    let functions: Vec<serde_json::Value> =
        BUILTIN_FUNCTIONS.iter().map(emit_function_entry).collect();

    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://formspec.org/specs/fel/functions/1.0",
        "title": "FEL Function Catalog",
        "description": "Structured catalog of all built-in functions in the Formspec Expression Language (FEL) v1.0. Each entry defines the function's name, signature, return type, null handling, and usage examples. This catalog is the normative reference for FEL function behavior; implementations in TypeScript (packages/formspec-engine) and Python (src/formspec/fel) must conform to these signatures and semantics.",
        "type": "object",
        "required": ["$formspecFelFunctions"],
        "properties": {
            "$formspecFelFunctions": {
                "type": "string",
                "const": "1.0",
                "description": "FEL function catalog specification version. MUST be '1.0'.",
                "examples": ["1.0"],
                "x-lm": {
                    "critical": true,
                    "intent": "Version pin for FEL function catalog document compatibility."
                }
            },
            "version": {
                "const": "1.0"
            },
            "functions": {
                "type": "array",
                "items": {
                    "$ref": "#/$defs/FunctionEntry"
                }
            }
        },
        "$defs": {
            "FELType": {
                "type": "string",
                "enum": ["string", "number", "boolean", "date", "dateTime", "time", "money", "array", "any", "null"],
                "description": "A FEL type identifier. 'any' means the function accepts or returns multiple types. 'array' means array<T> where T is specified in the description."
            },
            "Parameter": {
                "type": "object",
                "required": ["name", "type"],
                "properties": {
                    "name": { "type": "string" },
                    "type": { "$ref": "#/$defs/FELType" },
                    "description": { "type": "string" },
                    "required": { "type": "boolean", "default": true },
                    "variadic": { "type": "boolean", "default": false },
                    "enum": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "When present, restricts the parameter to these literal values."
                    }
                },
                "additionalProperties": false
            },
            "FunctionEntry": {
                "type": "object",
                "required": ["name", "category", "parameters", "returns", "description"],
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Function name as used in FEL expressions."
                    },
                    "category": {
                        "type": "string",
                        "enum": ["aggregate", "string", "numeric", "date", "logical", "type", "money", "mip", "repeat", "locale"],
                        "description": "Functional category for grouping and documentation."
                    },
                    "parameters": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/Parameter" },
                        "description": "Ordered parameter list. Variadic parameters must be last."
                    },
                    "returns": {
                        "$ref": "#/$defs/FELType",
                        "description": "Return type of the function."
                    },
                    "returnDescription": {
                        "type": "string",
                        "description": "Clarification of the return value when 'returns' alone is insufficient."
                    },
                    "description": {
                        "type": "string",
                        "description": "What the function does — behavior, edge cases, and constraints."
                    },
                    "nullHandling": {
                        "type": "string",
                        "description": "How the function behaves when one or more arguments are null."
                    },
                    "deterministic": {
                        "type": "boolean",
                        "default": true,
                        "description": "False if the function can return different results for the same arguments (e.g., today, now)."
                    },
                    "shortCircuit": {
                        "type": "boolean",
                        "default": false,
                        "description": "True if the function evaluates arguments lazily (e.g., if only evaluates the selected branch)."
                    },
                    "examples": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "expression": { "type": "string" },
                                "result": {},
                                "note": { "type": "string" }
                            },
                            "required": ["expression", "result"]
                        }
                    },
                    "sinceVersion": {
                        "type": "string",
                        "default": "1.0"
                    }
                },
                "additionalProperties": false
            }
        },
        "version": "1.0",
        "x-generated-from": X_GENERATED_FROM,
        "functions": functions
    })
}
/// Returns a JSON array of all builtin function entries (compact form, suitable for tooling that
/// iterates the catalog). Each entry includes a synthesized `"signature"` string for UI display.
/// For the full normative schema document, use [`emit_schema_json`].
pub fn builtin_function_catalog_json_value() -> serde_json::Value {
    serde_json::Value::Array(
        BUILTIN_FUNCTIONS
            .iter()
            .map(emit_function_entry_catalog)
            .collect(),
    )
}
/// `builtin_function_catalog_for(package)` rendered as a JSON array of function entries.
/// Each entry includes a synthesized `"signature"` string for UI display.
pub fn builtin_function_catalog_json_value_for(package: Package) -> serde_json::Value {
    serde_json::Value::Array(
        builtin_function_catalog_for(package)
            .map(emit_function_entry_catalog)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::types::Example;

    #[test]
    fn emit_example_omits_invalid_result_json_without_panicking() {
        let bad = Example {
            expression: "broken()",
            result_json: "not json",
            note: None,
        };
        assert!(emit_example(&bad).is_none());
    }

    #[test]
    fn emit_example_includes_valid_result_json() {
        let good = Example {
            expression: "true",
            result_json: "true",
            note: Some("always true"),
        };
        let v = emit_example(&good).expect("valid example");
        assert_eq!(v["expression"], "true");
        assert_eq!(v["result"], true);
        assert_eq!(v["note"], "always true");
    }
}
