//! Static builtin catalog data (`BUILTIN_FUNCTIONS`) and reserved names.
#![allow(clippy::missing_docs_in_private_items)]

use super::super::types::*;

pub(super) const ENTRIES: &[BuiltinFunctionCatalogEntry] = &[
    // ── repeat ───────────────────────────────────────────────────────────────
    BuiltinFunctionCatalogEntry {
        name: "prev",
        category: "repeat",
        parameters: &[Parameter {
            name: "fieldName",
            fel_type: FelType::String,
            description: Some(
                "Name of the sibling field to read from the previous repeat instance.",
            ),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Any,
        return_description: None,
        description: "Returns the value of the named field from the previous repeat instance (index - 1). Must be called within a repeat context. Returns null if at the first instance or not inside a repeat.",
        null_handling: Some("Returns null when no previous instance exists."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[Example {
            expression: "prev('runningTotal')",
            result_json: "500",
            note: Some("Value from previous row"),
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
            description: Some("Name of the sibling field to read from the next repeat instance."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Any,
        return_description: None,
        description: "Returns the value of the named field from the next repeat instance (index + 1). Must be called within a repeat context. Returns null if at the last instance or not inside a repeat.",
        null_handling: Some("Returns null when no next instance exists."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[Example {
            expression: "next('amount')",
            result_json: "200",
            note: Some("Peek at next row's value"),
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
            description: Some("Name of the ancestor field to find."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Any,
        return_description: None,
        description: "Walks up the path hierarchy from the current item and returns the value of the first ancestor field matching the given name. Useful for accessing enclosing group data from within nested repeats.",
        null_handling: Some("Returns null if no ancestor field with that name is found."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[Example {
            expression: "parent('projectName')",
            result_json: "\"Infrastructure Upgrade\"",
            note: None,
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
