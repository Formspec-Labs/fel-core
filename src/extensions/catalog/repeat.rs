//! Static builtin catalog data (`BUILTIN_FUNCTIONS`) and reserved names.
#![allow(clippy::missing_docs_in_private_items)]

use super::super::types::*;

pub(super) const ENTRIES: &[BuiltinFunctionCatalogEntry] = &[
    // ── repeat ───────────────────────────────────────────────────────────────
    // Repeat-navigation builtins (`prev`, `next`, `parent`) currently return the
    // previous/next/parent ITEM as a whole; the optional `fieldName` argument is
    // documented as part of the eventual richer surface but is presently ignored
    // by the evaluator. Until a separate ticket implements field-projection
    // semantics, the parameter is declared optional so the uniform arity gate
    // accepts both the bare form `prev()` (return the prior item) and the
    // forward-compatible `prev('field')` shape.
    BuiltinFunctionCatalogEntry {
        name: "prev",
        category: "repeat",
        parameters: &[Parameter {
            name: "fieldName",
            fel_type: FelType::String,
            description: Some(
                "Reserved for the future field-projection form. Currently ignored; bare `prev()` returns the previous repeat item.",
            ),
            required: false,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Any,
        return_description: None,
        description: "Returns the previous repeat instance's value. Must be called within a repeat context. Returns null if at the first instance or not inside a repeat. The optional `fieldName` argument is reserved and not yet honored.",
        null_handling: Some("Returns null when no previous instance exists."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[Example {
            expression: "prev()",
            result_json: "500",
            note: Some("Value of the previous repeat item"),
        }],
        since_version: "1.0",
        package: Package::Formspec,
    },
    BuiltinFunctionCatalogEntry {
        name: "next",
        category: "repeat",
        parameters: &[Parameter {
            name: "fieldName",
            fel_type: FelType::String,
            description: Some(
                "Reserved for the future field-projection form. Currently ignored; bare `next()` returns the next repeat item.",
            ),
            required: false,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Any,
        return_description: None,
        description: "Returns the next repeat instance's value. Must be called within a repeat context. Returns null if at the last instance or not inside a repeat. The optional `fieldName` argument is reserved and not yet honored.",
        null_handling: Some("Returns null when no next instance exists."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[Example {
            expression: "next()",
            result_json: "200",
            note: Some("Value of the next repeat item"),
        }],
        since_version: "1.0",
        package: Package::Formspec,
    },
    BuiltinFunctionCatalogEntry {
        name: "parent",
        category: "repeat",
        parameters: &[Parameter {
            name: "fieldName",
            fel_type: FelType::String,
            description: Some(
                "Reserved for the future field-projection form. Currently ignored; bare `parent()` returns the parent repeat item.",
            ),
            required: false,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Any,
        return_description: None,
        description: "Returns the parent repeat instance's value when called from within a nested repeat. Returns null if no parent exists. The optional `fieldName` argument is reserved and not yet honored.",
        null_handling: Some("Returns null if no parent repeat exists."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[Example {
            expression: "parent()",
            result_json: "\"Infrastructure Upgrade\"",
            note: Some("Value of the enclosing repeat item"),
        }],
        since_version: "1.0",
        package: Package::Formspec,
    },
    // instance: category changed instance → logical to match schema (schema is authoritative).
    // Placed at the end to match canonical schema function ordering.
    BuiltinFunctionCatalogEntry {
        name: "instance",
        category: "logical",
        parameters: &[
            Parameter {
                name: "name",
                fel_type: FelType::String,
                description: Some(
                    "Name of the secondary data source (must match a key in the definition's 'instances' object).",
                ),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "path",
                fel_type: FelType::String,
                description: Some("Dot-notation path within the instance data."),
                required: false,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Any,
        return_description: None,
        description: "Retrieves data from a named secondary instance. Typically invoked via the '@instance(\"name\")' context reference syntax in FEL, which the parser translates to this function call. The optional path parameter drills into the instance data.",
        null_handling: Some(
            "Returns null/undefined if instance name not found or path doesn't exist.",
        ),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example {
                expression: "@instance('priorYear').totalExpenditure",
                result_json: "200000",
                note: None,
            },
            Example {
                expression: "@instance('agencies')",
                result_json: "[{\"code\": \"DOE\", \"name\": \"Dept of Energy\"}]",
                note: None,
            },
        ],
        since_version: "1.0",
        package: Package::Formspec,
    },
];
