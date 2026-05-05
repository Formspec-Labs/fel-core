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
use std::collections::HashMap;

use crate::types::Value as TypeValue;

/// Host-package classification for a built-in.
///
/// Used by tooling (linters, IDE autocomplete) to filter the visible builtin
/// set per host. `Universal` builtins are reachable from any host;
/// `Formspec` builtins require formspec-shaped data (MIP queries, repeat
/// groups, instances, locale) and are no-ops against [`crate::MapEnvironment`].
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Package {
    /// Available to every host — pure language semantics.
    Universal,
    /// Requires formspec-shaped data: MIP queries, repeat groups, instances, locale.
    Formspec,
}

/// FEL type identifier used in structured catalog entries.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FelType {
    /// String type.
    String,
    /// Number type.
    Number,
    /// Boolean type.
    Boolean,
    /// Date type (ISO 8601 date).
    Date,
    /// DateTime type (ISO 8601 dateTime).
    DateTime,
    /// Time type (HH:MM:SS).
    Time,
    /// Money type ({amount, currency}).
    Money,
    /// Array type (`array<T>`).
    Array,
    /// Any type (accepts or returns multiple types).
    Any,
    /// Null type.
    Null,
}

impl FelType {
    /// Wire name used in the JSON schema.
    pub fn as_str(self) -> &'static str {
        match self {
            FelType::String => "string",
            FelType::Number => "number",
            FelType::Boolean => "boolean",
            FelType::Date => "date",
            FelType::DateTime => "dateTime",
            FelType::Time => "time",
            FelType::Money => "money",
            FelType::Array => "array",
            FelType::Any => "any",
            FelType::Null => "null",
        }
    }
}

/// One parameter in a built-in function signature.
#[non_exhaustive]
pub struct Parameter {
    /// Parameter name.
    pub name: &'static str,
    /// FEL type of the parameter.
    pub fel_type: FelType,
    /// Human-readable description of the parameter.
    pub description: Option<&'static str>,
    /// Whether the parameter is required (default true).
    pub required: bool,
    /// Whether the parameter is variadic — must be last (default false).
    pub variadic: bool,
    /// Closed set of allowed literal values (schema `enum` field).
    pub allowed_values: Option<&'static [&'static str]>,
}

/// One worked example attached to a built-in function.
#[non_exhaustive]
pub struct Example {
    /// FEL expression demonstrating the function.
    pub expression: &'static str,
    /// JSON literal for the example result, as a `&str`. Parsed to `serde_json::Value` at
    /// emission time. Use compact JSON; embedded nulls are the literal string `"null"`.
    pub result_json: &'static str,
    /// Optional clarifying note.
    pub note: Option<&'static str>,
}

/// Structured metadata for a built-in FEL function.
///
/// This is the canonical source of truth for the FEL function catalog.
/// Emit [`emit_schema_json`] to regenerate `formspec/schemas/fel-functions.schema.json`.
#[non_exhaustive]
pub struct BuiltinFunctionCatalogEntry {
    /// Function name as used in FEL source.
    pub name: &'static str,
    /// Functional category. Closed enum from schema:
    /// `aggregate|string|numeric|date|logical|type|money|mip|repeat|locale`.
    pub category: &'static str,
    /// Ordered parameter list. Variadic parameters must be last.
    pub parameters: &'static [Parameter],
    /// Return type of the function.
    pub returns: FelType,
    /// Clarification of the return value when `returns` alone is insufficient.
    pub return_description: Option<&'static str>,
    /// What the function does — behavior, edge cases, and constraints.
    pub description: &'static str,
    /// How the function behaves when one or more arguments are null.
    pub null_handling: Option<&'static str>,
    /// False if the function can return different results for the same arguments.
    pub deterministic: bool,
    /// True if `deterministic` should be emitted explicitly in the catalog JSON even when the
    /// value is `true` (the default). Used for entries whose canonical schema records the field
    /// explicitly for clarity (e.g., `pluralCategory`).
    pub emit_deterministic_explicitly: bool,
    /// True if the function evaluates arguments lazily.
    pub short_circuit: bool,
    /// Worked examples.
    pub examples: &'static [Example],
    /// Spec version in which the function was introduced (default `"1.0"`).
    pub since_version: &'static str,
    /// Host-package classification for filtering by tooling.
    pub package: Package,
}

/// Type alias for extension function implementations.
pub type ExtensionFn = Box<dyn Fn(&[TypeValue]) -> TypeValue + Send + Sync>;

/// A registered extension function.
pub struct ExtensionFunc {
    /// Human-readable name for diagnostics.
    pub name: String,
    /// Minimum number of arguments.
    pub min_args: usize,
    /// Maximum number of arguments (None = unbounded).
    pub max_args: Option<usize>,
    /// The implementation: receives pre-evaluated args, returns a value.
    /// Arguments are guaranteed non-null (null propagation handled by caller).
    pub func: ExtensionFn,
}

/// Registry of extension functions.
pub struct ExtensionRegistry {
    extensions: HashMap<String, ExtensionFunc>,
}

/// Reserved words and builtin names that cannot be shadowed.
const RESERVED_WORDS: &[&str] = &[
    "true", "false", "null", "let", "in", "if", "then", "else", "and", "or", "not",
];

const BUILTIN_FUNCTIONS: &[BuiltinFunctionCatalogEntry] = &[
    // ── aggregate ────────────────────────────────────────────────────────────
    BuiltinFunctionCatalogEntry {
        name: "sum",
        category: "aggregate",
        parameters: &[Parameter {
            name: "values",
            fel_type: FelType::Array,
            description: Some("Array of numbers (or money objects — extracts .amount)."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Sums all numeric elements in the array. Extracts .amount from money objects. Non-finite values treated as 0.",
        null_handling: Some("Null elements are skipped. Null argument returns 0."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "sum($items[*].amount)", result_json: "1500", note: None },
            Example { expression: "sum([10, null, 20])", result_json: "30", note: None },
            Example { expression: "sum([])", result_json: "0", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "count",
        category: "aggregate",
        parameters: &[Parameter {
            name: "values",
            fel_type: FelType::Array,
            description: Some("Array to count."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Returns the number of elements in the array, including nulls.",
        null_handling: Some("Null argument returns 0. Null elements ARE counted."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "count($items[*].name)", result_json: "3", note: None },
            Example { expression: "count([1, null, 3])", result_json: "3", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "countWhere",
        category: "aggregate",
        parameters: &[
            Parameter {
                name: "values",
                fel_type: FelType::Array,
                description: Some("Array to filter."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "predicate",
                fel_type: FelType::Boolean,
                description: Some("FEL expression evaluated per element with '$' rebound to the current element."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Number,
        return_description: None,
        description: "Counts array elements for which the predicate evaluates to true. The predicate receives each element via '$' (the self-reference is rebound per element). Special argument handling: the predicate is NOT pre-evaluated — it is evaluated once per element.",
        null_handling: Some("Null array returns 0. Null predicate result counts as false."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: true,
        examples: &[
            Example { expression: "countWhere($items[*].amount, $ > 100)", result_json: "2", note: None },
            Example { expression: "countWhere($scores[*], $ >= 60)", result_json: "4", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "sumWhere",
        category: "aggregate",
        parameters: &[
            Parameter {
                name: "values",
                fel_type: FelType::Array,
                description: Some("Array of numbers (or elements from which numeric matches are taken)."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "predicate",
                fel_type: FelType::Boolean,
                description: Some("FEL expression evaluated per element with '$' rebound to the current element."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Number,
        return_description: None,
        description: "Sums numeric array elements for which the predicate evaluates to true. The predicate is NOT pre-evaluated — it is evaluated once per element.",
        null_handling: Some("Null array returns null. Non-numeric matches are skipped. No numeric matches returns 0."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: true,
        examples: &[
            Example { expression: "sumWhere([1, 2, 3, 4], $ > 2)", result_json: "7", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "avgWhere",
        category: "aggregate",
        parameters: &[
            Parameter {
                name: "values",
                fel_type: FelType::Array,
                description: Some("Array of numbers."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "predicate",
                fel_type: FelType::Boolean,
                description: Some("FEL expression evaluated per element with '$' rebound to the current element."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Number,
        return_description: None,
        description: "Arithmetic mean of numeric elements for which the predicate is true. Returns null when no elements match.",
        null_handling: Some("Null array returns null. No matches returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: true,
        examples: &[
            Example { expression: "avgWhere([40, 50, 60], $ >= 50)", result_json: "55", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "minWhere",
        category: "aggregate",
        parameters: &[
            Parameter {
                name: "values",
                fel_type: FelType::Array,
                description: Some("Array of comparable values."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "predicate",
                fel_type: FelType::Boolean,
                description: Some("FEL expression evaluated per element with '$' rebound to the current element."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Any,
        return_description: None,
        description: "Smallest element among those satisfying the predicate. Also applies to dates and strings.",
        null_handling: Some("Null array returns null. No matches returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: true,
        examples: &[
            Example { expression: "minWhere([3, 1, 4], $ > 0)", result_json: "1", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "maxWhere",
        category: "aggregate",
        parameters: &[
            Parameter {
                name: "values",
                fel_type: FelType::Array,
                description: Some("Array of comparable values."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "predicate",
                fel_type: FelType::Boolean,
                description: Some("FEL expression evaluated per element with '$' rebound to the current element."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Any,
        return_description: None,
        description: "Largest element among those satisfying the predicate. Also applies to dates and strings.",
        null_handling: Some("Null array returns null. No matches returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: true,
        examples: &[
            Example { expression: "maxWhere([3, 1, 4], $ < 4)", result_json: "3", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "every",
        category: "aggregate",
        parameters: &[
            Parameter {
                name: "values",
                fel_type: FelType::Array,
                description: Some("Array to test."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "predicate",
                fel_type: FelType::Boolean,
                description: Some("FEL expression evaluated per element with '$' rebound to the current element."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Boolean,
        return_description: None,
        description: "True if the array is empty or the predicate is true for every element. The predicate is NOT pre-evaluated — it is evaluated once per element.",
        null_handling: Some("Null array returns null. Null predicate result is not true (element fails every, does not satisfy some)."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: true,
        examples: &[
            Example { expression: "every([1, 2, 3], $ > 0)", result_json: "true", note: None },
            Example { expression: "every([], $ > 0)", result_json: "true", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "some",
        category: "aggregate",
        parameters: &[
            Parameter {
                name: "values",
                fel_type: FelType::Array,
                description: Some("Array to test."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "predicate",
                fel_type: FelType::Boolean,
                description: Some("FEL expression evaluated per element with '$' rebound to the current element."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Boolean,
        return_description: None,
        description: "True if at least one element satisfies the predicate. The predicate is NOT pre-evaluated — it is evaluated once per element.",
        null_handling: Some("Null array returns null. Null predicate result counts as false."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: true,
        examples: &[
            Example { expression: "some([0, 2, 4], $ > 3)", result_json: "true", note: None },
            Example { expression: "some([], $ > 0)", result_json: "false", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "avg",
        category: "aggregate",
        parameters: &[Parameter {
            name: "values",
            fel_type: FelType::Array,
            description: Some("Array of numbers."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Arithmetic mean of all finite numeric elements. Skips nulls and non-numeric values.",
        null_handling: Some("Null/non-numeric elements skipped. Empty array or all-null returns 0."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "avg([10, 20, 30])", result_json: "20", note: None },
            Example { expression: "avg([10, null, 30])", result_json: "20", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "min",
        category: "aggregate",
        parameters: &[Parameter {
            name: "values",
            fel_type: FelType::Array,
            description: Some("Array of numbers."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Returns the smallest finite numeric value in the array.",
        null_handling: Some("Null/non-numeric elements skipped. Empty array returns 0."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "min([5, 2, 8])", result_json: "2", note: None },
            Example { expression: "min($items[*].amount)", result_json: "100", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "max",
        category: "aggregate",
        parameters: &[Parameter {
            name: "values",
            fel_type: FelType::Array,
            description: Some("Array of numbers."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Returns the largest finite numeric value in the array.",
        null_handling: Some("Null/non-numeric elements skipped. Empty array returns 0."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "max([5, 2, 8])", result_json: "8", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    // ── string ───────────────────────────────────────────────────────────────
    BuiltinFunctionCatalogEntry {
        name: "length",
        category: "string",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::String,
            description: Some("String to measure."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Returns the number of characters in the string.",
        null_handling: Some("Null returns 0."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "length('hello')", result_json: "5", note: None },
            Example { expression: "length(null)", result_json: "0", note: None },
            Example { expression: "length($name)", result_json: "12", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "contains",
        category: "string",
        parameters: &[
            Parameter {
                name: "haystack",
                fel_type: FelType::String,
                description: Some("String to search in."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "needle",
                fel_type: FelType::String,
                description: Some("Substring to find."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Boolean,
        return_description: None,
        description: "Returns true if haystack contains needle. Case-sensitive.",
        null_handling: Some("Null haystack treated as empty string."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "contains('hello world', 'world')", result_json: "true", note: None },
            Example { expression: "contains($email, '@')", result_json: "true", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "startsWith",
        category: "string",
        parameters: &[
            Parameter {
                name: "value",
                fel_type: FelType::String,
                description: Some("String to test."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "prefix",
                fel_type: FelType::String,
                description: Some("Expected prefix."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Boolean,
        return_description: None,
        description: "Returns true if value starts with prefix. Case-sensitive.",
        null_handling: Some("Null value treated as empty string."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "startsWith($url, 'https://')", result_json: "true", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "endsWith",
        category: "string",
        parameters: &[
            Parameter {
                name: "value",
                fel_type: FelType::String,
                description: Some("String to test."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "suffix",
                fel_type: FelType::String,
                description: Some("Expected suffix."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Boolean,
        return_description: None,
        description: "Returns true if value ends with suffix. Case-sensitive.",
        null_handling: Some("Null value treated as empty string."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "endsWith($email, '.gov')", result_json: "true", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "substring",
        category: "string",
        parameters: &[
            Parameter {
                name: "value",
                fel_type: FelType::String,
                description: Some("Source string."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "start",
                fel_type: FelType::Number,
                description: Some("1-based start position."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "length",
                fel_type: FelType::Number,
                description: Some("Number of characters to extract."),
                required: false,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::String,
        return_description: None,
        description: "Extracts a substring starting at the 1-based position. If length is omitted, extracts to the end of the string.",
        null_handling: Some("Null value treated as empty string."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "substring('abcdef', 2, 3)", result_json: "\"bcd\"", note: None },
            Example { expression: "substring('abcdef', 4)", result_json: "\"def\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "replace",
        category: "string",
        parameters: &[
            Parameter {
                name: "value",
                fel_type: FelType::String,
                description: Some("Source string."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "search",
                fel_type: FelType::String,
                description: Some("Literal substring to find (NOT regex)."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "replacement",
                fel_type: FelType::String,
                description: Some("Replacement string."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::String,
        return_description: None,
        description: "Replaces ALL occurrences of the search literal with the replacement. Not regex — literal string match only.",
        null_handling: Some("Null value treated as empty string."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "replace('hello world', 'world', 'there')", result_json: "\"hello there\"", note: None },
            Example { expression: "replace($phone, '-', '')", result_json: "\"5551234567\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "upper",
        category: "string",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::String,
            description: Some("String to convert."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::String,
        return_description: None,
        description: "Converts string to uppercase.",
        null_handling: Some("Null treated as empty string, returns empty string."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "upper('hello')", result_json: "\"HELLO\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "lower",
        category: "string",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::String,
            description: Some("String to convert."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::String,
        return_description: None,
        description: "Converts string to lowercase.",
        null_handling: Some("Null treated as empty string, returns empty string."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "lower('HELLO')", result_json: "\"hello\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "trim",
        category: "string",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::String,
            description: Some("String to trim."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::String,
        return_description: None,
        description: "Removes leading and trailing whitespace.",
        null_handling: Some("Null treated as empty string, returns empty string."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "trim('  hello  ')", result_json: "\"hello\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "matches",
        category: "string",
        parameters: &[
            Parameter {
                name: "value",
                fel_type: FelType::String,
                description: Some("String to test."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "pattern",
                fel_type: FelType::String,
                description: Some("Regular expression pattern."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Boolean,
        return_description: None,
        description: "Returns true if the string matches the regular expression pattern. Pattern syntax follows the host language's regex engine (ECMA-262 for TypeScript, Python re for Python).",
        null_handling: Some("Null value treated as empty string."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "matches($ein, '^[0-9]{2}-[0-9]{7}$')", result_json: "true", note: None },
            Example { expression: "matches($email, '^[^@]+@[^@]+$')", result_json: "true", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "format",
        category: "string",
        parameters: &[
            Parameter {
                name: "template",
                fel_type: FelType::String,
                description: Some("Format string with {0}, {1}, ... positional placeholders."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "args",
                fel_type: FelType::Any,
                description: Some("Values to substitute for placeholders."),
                required: true,
                variadic: true,
                allowed_values: None,
            },
        ],
        returns: FelType::String,
        return_description: None,
        description: "Positional string interpolation. Replaces {0}, {1}, etc. in the template with stringified arguments. Null arguments become empty string. Numbers strip trailing zeros. Booleans become 'true'/'false'.",
        null_handling: Some("Null template returns empty string. Null arguments substituted as empty string."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "format('{0} of {1}', $current, $total)", result_json: "\"3 of 10\"", note: None },
            Example { expression: "format('Hello, {0}!', $name)", result_json: "\"Hello, Alice!\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    // ── numeric ──────────────────────────────────────────────────────────────
    BuiltinFunctionCatalogEntry {
        name: "round",
        category: "numeric",
        parameters: &[
            Parameter {
                name: "value",
                fel_type: FelType::Number,
                description: Some("Number to round."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "precision",
                fel_type: FelType::Number,
                description: Some("Decimal places (default 0)."),
                required: false,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Number,
        return_description: None,
        description: "Rounds to the specified number of decimal places using banker's rounding (round half to even).",
        null_handling: Some("Null treated as 0."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "round(3.456, 2)", result_json: "3.46", note: None },
            Example { expression: "round(2.5)", result_json: "2", note: Some("Banker's rounding: half rounds to even") },
            Example { expression: "round(3.5)", result_json: "4", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "floor",
        category: "numeric",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Number,
            description: Some("Number to floor."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Returns the largest integer less than or equal to the value.",
        null_handling: Some("Null treated as 0."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "floor(3.7)", result_json: "3", note: None },
            Example { expression: "floor(-2.3)", result_json: "-3", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "ceil",
        category: "numeric",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Number,
            description: Some("Number to ceil."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Returns the smallest integer greater than or equal to the value.",
        null_handling: Some("Null treated as 0."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "ceil(3.2)", result_json: "4", note: None },
            Example { expression: "ceil(-2.7)", result_json: "-2", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "abs",
        category: "numeric",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Number,
            description: Some("Number to take absolute value of."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Returns the absolute value of the number.",
        null_handling: Some("Null treated as 0."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "abs(-42)", result_json: "42", note: None },
            Example { expression: "abs(42)", result_json: "42", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "power",
        category: "numeric",
        parameters: &[
            Parameter {
                name: "base",
                fel_type: FelType::Number,
                description: Some("Base number."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "exponent",
                fel_type: FelType::Number,
                description: Some("Exponent."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Number,
        return_description: None,
        description: "Returns base raised to the power of exponent.",
        null_handling: Some("Null arguments treated as 0."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "power(2, 10)", result_json: "1024", note: None },
            Example { expression: "power(10, 2)", result_json: "100", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    // ── date ─────────────────────────────────────────────────────────────────
    BuiltinFunctionCatalogEntry {
        name: "today",
        category: "date",
        parameters: &[],
        returns: FelType::Date,
        return_description: None,
        description: "Returns the current date as an ISO 8601 date string (YYYY-MM-DD). Non-deterministic.",
        null_handling: Some("N/A — no parameters."),
        deterministic: false,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "today()", result_json: "\"2025-07-10\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "now",
        category: "date",
        parameters: &[],
        returns: FelType::DateTime,
        return_description: None,
        description: "Returns the current date and time as an ISO 8601 dateTime string. Non-deterministic.",
        null_handling: Some("N/A — no parameters."),
        deterministic: false,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "now()", result_json: "\"2025-07-10T14:30:00.000Z\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "year",
        category: "date",
        parameters: &[Parameter {
            name: "date",
            fel_type: FelType::Date,
            description: Some("ISO 8601 date or dateTime string."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Extracts the 4-digit year from a date.",
        null_handling: Some("Null returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "year(@2025-07-10)", result_json: "2025", note: None },
            Example { expression: "year($birthDate)", result_json: "1990", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "month",
        category: "date",
        parameters: &[Parameter {
            name: "date",
            fel_type: FelType::Date,
            description: Some("ISO 8601 date or dateTime string."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Extracts the month (1-12) from a date.",
        null_handling: Some("Null returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "month(@2025-07-10)", result_json: "7", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "day",
        category: "date",
        parameters: &[Parameter {
            name: "date",
            fel_type: FelType::Date,
            description: Some("ISO 8601 date or dateTime string."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Extracts the day of month (1-31) from a date.",
        null_handling: Some("Null returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "day(@2025-07-10)", result_json: "10", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "hours",
        category: "date",
        parameters: &[Parameter {
            name: "dateTime",
            fel_type: FelType::DateTime,
            description: Some("ISO 8601 dateTime or time string."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Extracts the hour component (0-23) from a dateTime or time value.",
        null_handling: Some("Null returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "hours(@2025-07-10T14:30:00Z)", result_json: "14", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "minutes",
        category: "date",
        parameters: &[Parameter {
            name: "dateTime",
            fel_type: FelType::DateTime,
            description: Some("ISO 8601 dateTime or time string."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Extracts the minute component (0-59) from a dateTime or time value.",
        null_handling: Some("Null returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "minutes(@2025-07-10T14:30:00Z)", result_json: "30", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "seconds",
        category: "date",
        parameters: &[Parameter {
            name: "dateTime",
            fel_type: FelType::DateTime,
            description: Some("ISO 8601 dateTime or time string."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Extracts the second component (0-59) from a dateTime or time value.",
        null_handling: Some("Null returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "seconds(@2025-07-10T14:30:45Z)", result_json: "45", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "time",
        category: "date",
        parameters: &[
            Parameter {
                name: "hours",
                fel_type: FelType::Number,
                description: Some("Hour (0-23)."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "minutes",
                fel_type: FelType::Number,
                description: Some("Minute (0-59)."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "seconds",
                fel_type: FelType::Number,
                description: Some("Second (0-59)."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Time,
        return_description: None,
        description: "Constructs an HH:MM:SS time string from numeric components.",
        null_handling: Some("Null components treated as 0."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "time(14, 30, 0)", result_json: "\"14:30:00\"", note: None },
            Example { expression: "time(9, 5, 30)", result_json: "\"09:05:30\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "dateDiff",
        category: "date",
        parameters: &[
            Parameter {
                name: "date1",
                fel_type: FelType::Date,
                description: Some("First date (ISO 8601)."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "date2",
                fel_type: FelType::Date,
                description: Some("Second date (ISO 8601)."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "unit",
                fel_type: FelType::String,
                description: Some("Unit of measurement."),
                required: true,
                variadic: false,
                allowed_values: Some(&["days", "months", "years"]),
            },
        ],
        returns: FelType::Number,
        return_description: None,
        description: "Returns the difference date1 - date2 in the specified unit. Result is positive when date1 > date2, negative when date1 < date2. For months/years, incomplete periods are truncated (not rounded).",
        null_handling: Some("Null or invalid dates return null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "dateDiff(@2025-07-10, @2025-01-01, 'days')", result_json: "190", note: None },
            Example { expression: "dateDiff($endDate, $startDate, 'months')", result_json: "6", note: None },
            Example { expression: "dateDiff(today(), $birthDate, 'years')", result_json: "35", note: Some("Age calculation") },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "dateAdd",
        category: "date",
        parameters: &[
            Parameter {
                name: "date",
                fel_type: FelType::Date,
                description: Some("Base date (ISO 8601)."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "amount",
                fel_type: FelType::Number,
                description: Some("Number of units to add (negative to subtract)."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "unit",
                fel_type: FelType::String,
                description: Some("Unit of measurement."),
                required: true,
                variadic: false,
                allowed_values: Some(&["days", "months", "years"]),
            },
        ],
        returns: FelType::Date,
        return_description: Some("ISO 8601 date string (YYYY-MM-DD)."),
        description: "Adds the specified number of units to a date. Negative values subtract. Month/year arithmetic handles end-of-month overflow per the host language's Date implementation.",
        null_handling: Some("Null date returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "dateAdd(@2025-01-15, 30, 'days')", result_json: "\"2025-02-14\"", note: None },
            Example { expression: "dateAdd(today(), 1, 'years')", result_json: "\"2026-07-10\"", note: None },
            Example { expression: "dateAdd($startDate, -6, 'months')", result_json: "\"2025-01-10\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "timeDiff",
        category: "date",
        parameters: &[
            Parameter {
                name: "laterTime",
                fel_type: FelType::Time,
                description: Some("Later ISO 8601 time string (HH:MM:SS)."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "earlierTime",
                fel_type: FelType::Time,
                description: Some("Earlier ISO 8601 time string (HH:MM:SS)."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Number,
        return_description: Some("Signed difference in whole seconds: laterTime minus earlierTime (positive when the first argument is later in the day)."),
        description: "Difference in seconds between two time-of-day strings. Distinct from duration(), which parses an ISO 8601 duration and returns milliseconds.",
        null_handling: Some("Invalid time strings produce a diagnostic and null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "timeDiff('14:30:00', '13:00:00')", result_json: "5400", note: None },
            Example { expression: "timeDiff('13:00:00', '14:30:00')", result_json: "-5400", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "duration",
        category: "date",
        parameters: &[Parameter {
            name: "isoDuration",
            fel_type: FelType::String,
            description: Some("ISO 8601 duration (PnYnMnDTnHnMnS subset), optional leading minus."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Parses a duration string and returns its length in milliseconds. Years/months in the date part use fixed 365-day years and 30-day months (not calendar arithmetic). Distinct from timeDiff, which compares two clock times in seconds.",
        null_handling: Some("Null argument returns null. Invalid strings produce an error diagnostic and null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "duration('PT1H')", result_json: "3600000", note: None },
            Example { expression: "duration('P1D')", result_json: "86400000", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    // ── logical ──────────────────────────────────────────────────────────────
    BuiltinFunctionCatalogEntry {
        name: "if",
        category: "logical",
        parameters: &[
            Parameter {
                name: "condition",
                fel_type: FelType::Boolean,
                description: Some("Condition to evaluate."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "thenValue",
                fel_type: FelType::Any,
                description: Some("Value returned when condition is true."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "elseValue",
                fel_type: FelType::Any,
                description: Some("Value returned when condition is false."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Any,
        return_description: Some("Type matches whichever branch is selected."),
        description: "Conditional function. Returns thenValue when condition is true, elseValue when false. Only the selected branch is evaluated (short-circuit). Alternative syntax: 'if cond then a else b' (keyword form). Note: 'if' is a reserved word; this function is special-cased in the parser.",
        null_handling: Some("Null condition is an evaluation error."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: true,
        examples: &[
            Example { expression: "if($age >= 18, 'adult', 'minor')", result_json: "\"adult\"", note: None },
            Example { expression: "if($status = 'married', $income * 0.75, $income)", result_json: "45000", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "coalesce",
        category: "logical",
        parameters: &[Parameter {
            name: "values",
            fel_type: FelType::Any,
            description: Some("Values to check."),
            required: true,
            variadic: true,
            allowed_values: None,
        }],
        returns: FelType::Any,
        return_description: None,
        description: "Returns the first argument that is not null, undefined, or empty string. If all arguments are null/empty, returns null.",
        null_handling: Some("Core purpose is null handling — returns first non-null, non-empty value."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "coalesce($preferredName, $firstName, 'Unknown')", result_json: "\"Alice\"", note: None },
            Example { expression: "coalesce(null, '', 'fallback')", result_json: "\"fallback\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "empty",
        category: "logical",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Any,
            description: Some("Value to test."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Boolean,
        return_description: None,
        description: "Returns true if the value is null, undefined, empty string (''), or an empty array ([]). Broader than a simple null check.",
        null_handling: Some("Null returns true (that's the point)."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "empty(null)", result_json: "true", note: None },
            Example { expression: "empty('')", result_json: "true", note: None },
            Example { expression: "empty([])", result_json: "true", note: None },
            Example { expression: "empty('hello')", result_json: "false", note: None },
            Example { expression: "empty(0)", result_json: "false", note: Some("0 is not empty") },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "present",
        category: "logical",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Any,
            description: Some("Value to test."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Boolean,
        return_description: None,
        description: "Inverse of empty(). Returns true if the value is non-null, non-empty-string, and non-empty-array.",
        null_handling: Some("Null returns false."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "present($email)", result_json: "true", note: None },
            Example { expression: "present(null)", result_json: "false", note: None },
            Example { expression: "not empty($email) or not empty($phone)", result_json: "true", note: Some("Equivalent to: present($email) or present($phone)") },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "selected",
        category: "logical",
        parameters: &[
            Parameter {
                name: "value",
                fel_type: FelType::Any,
                description: Some("Field value (string for choice, array for multiChoice)."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "option",
                fel_type: FelType::String,
                description: Some("Option value to check for."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Boolean,
        return_description: None,
        description: "For multiChoice (array): returns true if the option is included in the array. For choice (string): returns true if value equals option. Designed for testing selected options in choice/multiChoice fields.",
        null_handling: Some("Null value returns false."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "selected($categories, 'personnel')", result_json: "true", note: Some("multiChoice field") },
            Example { expression: "selected($status, 'active')", result_json: "true", note: Some("choice field") },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    // ── type ─────────────────────────────────────────────────────────────────
    BuiltinFunctionCatalogEntry {
        name: "isNumber",
        category: "type",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Any,
            description: Some("Value to test."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Boolean,
        return_description: None,
        description: "Returns true if the value is a finite number (not NaN).",
        null_handling: Some("Null returns false."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "isNumber(42)", result_json: "true", note: None },
            Example { expression: "isNumber('42')", result_json: "false", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "isString",
        category: "type",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Any,
            description: Some("Value to test."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Boolean,
        return_description: None,
        description: "Returns true if the value is a string.",
        null_handling: Some("Null returns false."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "isString('hello')", result_json: "true", note: None },
            Example { expression: "isString(42)", result_json: "false", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "isDate",
        category: "type",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Any,
            description: Some("Value to test."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Boolean,
        return_description: None,
        description: "Returns true if the value can be parsed as a valid date.",
        null_handling: Some("Null returns false."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "isDate('2025-07-10')", result_json: "true", note: None },
            Example { expression: "isDate('not-a-date')", result_json: "false", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "isNull",
        category: "type",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Any,
            description: Some("Value to test."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Boolean,
        return_description: None,
        description: "Returns true if the value is null, undefined, or empty string. Note: broader than a strict null check — empty string is also considered 'null' in FEL.",
        null_handling: Some("Null returns true (that's the point)."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "isNull(null)", result_json: "true", note: None },
            Example { expression: "isNull('')", result_json: "true", note: None },
            Example { expression: "isNull(0)", result_json: "false", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "typeOf",
        category: "type",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Any,
            description: Some("Value to inspect."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::String,
        return_description: Some("One of: 'string', 'number', 'boolean', 'array', 'object', 'null'."),
        description: "Returns the FEL type name of the value.",
        null_handling: Some("Null returns 'null'."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "typeOf(42)", result_json: "\"number\"", note: None },
            Example { expression: "typeOf([1,2])", result_json: "\"array\"", note: None },
            Example { expression: "typeOf(null)", result_json: "\"null\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    // cast functions: category changed cast → type to match schema (schema is authoritative)
    BuiltinFunctionCatalogEntry {
        name: "number",
        category: "type",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Any,
            description: Some("Value to coerce to number."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Explicit type cast to number. Strings are parsed as numbers. Returns null if coercion fails.",
        null_handling: Some("Null returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "number('42')", result_json: "42", note: None },
            Example { expression: "number('3.14')", result_json: "3.14", note: None },
            Example { expression: "number('abc')", result_json: "null", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "string",
        category: "type",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Any,
            description: Some("Value to coerce to string."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::String,
        return_description: None,
        description: "Explicit type cast to string. Null becomes empty string. Numbers, booleans, and dates are stringified.",
        null_handling: Some("Null returns empty string ''."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "string(42)", result_json: "\"42\"", note: None },
            Example { expression: "string(true)", result_json: "\"true\"", note: None },
            Example { expression: "string(null)", result_json: "\"\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "boolean",
        category: "type",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Any,
            description: Some("Value to coerce to boolean."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Boolean,
        return_description: None,
        description: "Explicit type cast to boolean. Accepts: booleans (pass-through), numbers (0 = false, non-zero = true), strings 'true'/'false'. Other values produce an evaluation error.",
        null_handling: Some("Null returns false."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "boolean(1)", result_json: "true", note: None },
            Example { expression: "boolean(0)", result_json: "false", note: None },
            Example { expression: "boolean('true')", result_json: "true", note: None },
            Example { expression: "boolean(null)", result_json: "false", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "date",
        category: "type",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Any,
            description: Some("Value to validate/coerce as ISO 8601 date."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Date,
        return_description: None,
        description: "Validates and returns the input as an ISO 8601 date string. If the input is not a valid date, produces an evaluation error.",
        null_handling: Some("Null returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "date('2025-07-10')", result_json: "\"2025-07-10\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    // ── money ─────────────────────────────────────────────────────────────────
    BuiltinFunctionCatalogEntry {
        name: "money",
        category: "money",
        parameters: &[
            Parameter {
                name: "amount",
                fel_type: FelType::Number,
                description: Some("Monetary amount."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "currency",
                fel_type: FelType::String,
                description: Some("ISO 4217 currency code (e.g., 'USD', 'EUR')."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Money,
        return_description: Some("Object: {amount: number, currency: string}."),
        description: "Constructs a money object from an amount and currency code.",
        null_handling: Some("Null arguments produce a money object with null/undefined fields."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "money(50000, 'USD')", result_json: "{\"amount\": 50000, \"currency\": \"USD\"}", note: None },
            Example { expression: "money($total, 'EUR')", result_json: "{\"amount\": 12500, \"currency\": \"EUR\"}", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "moneyAmount",
        category: "money",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Money,
            description: Some("Money object."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Number,
        return_description: None,
        description: "Extracts the numeric amount from a money object.",
        null_handling: Some("Null or non-money value returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "moneyAmount($budget)", result_json: "50000", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "moneyCurrency",
        category: "money",
        parameters: &[Parameter {
            name: "value",
            fel_type: FelType::Money,
            description: Some("Money object."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::String,
        return_description: None,
        description: "Extracts the currency code from a money object.",
        null_handling: Some("Null or non-money value returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "moneyCurrency($budget)", result_json: "\"USD\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "moneyAdd",
        category: "money",
        parameters: &[
            Parameter {
                name: "a",
                fel_type: FelType::Money,
                description: Some("First money object."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "b",
                fel_type: FelType::Money,
                description: Some("Second money object. Must have the same currency as first."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Money,
        return_description: None,
        description: "Adds two money objects. Uses the currency from the first non-null operand. Per the spec, both operands SHOULD have the same currency; implementations MAY error on mismatched currencies.",
        null_handling: Some("Null operand returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "moneyAdd(money(100, 'USD'), money(250, 'USD'))", result_json: "{\"amount\": 350, \"currency\": \"USD\"}", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "moneySum",
        category: "money",
        parameters: &[Parameter {
            name: "values",
            fel_type: FelType::Array,
            description: Some("Array of money objects."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Money,
        return_description: None,
        description: "Sums an array of money objects. Returns a money object with the currency from the first element. All elements SHOULD have the same currency.",
        null_handling: Some("Null elements skipped. Empty array returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "moneySum($lineItems[*].cost)", result_json: "{\"amount\": 1500, \"currency\": \"USD\"}", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    BuiltinFunctionCatalogEntry {
        name: "moneySumWhere",
        category: "money",
        parameters: &[
            Parameter {
                name: "values",
                fel_type: FelType::Array,
                description: Some("Array of money objects."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "predicate",
                fel_type: FelType::Boolean,
                description: Some("FEL expression evaluated per element with '$' rebound to the current money value."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::Money,
        return_description: None,
        description: "Sums money array elements for which the predicate evaluates to true. Matched elements MUST share the same currency. The predicate is NOT pre-evaluated — it is evaluated once per element.",
        null_handling: Some("Null array returns null. No matches returns null."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: true,
        examples: &[
            Example { expression: "moneySumWhere([money(100,'USD'), money(200,'USD')], moneyAmount($) > 50)", result_json: "{\"amount\": 300, \"currency\": \"USD\"}", note: None },
        ],
        since_version: "1.0",
        package: Package::Universal,
    },
    // ── locale ───────────────────────────────────────────────────────────────
    BuiltinFunctionCatalogEntry {
        name: "locale",
        category: "locale",
        parameters: &[],
        returns: FelType::String,
        return_description: None,
        description: "Returns the active BCP 47 locale tag from the evaluation context.",
        null_handling: Some("When the host has not set a locale, returns null."),
        deterministic: false,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "locale()", result_json: "\"en-US\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Formspec,
    },
    BuiltinFunctionCatalogEntry {
        name: "runtimeMeta",
        category: "locale",
        parameters: &[Parameter {
            name: "key",
            fel_type: FelType::String,
            description: Some("Key in the host-provided runtime metadata bag."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Any,
        return_description: None,
        description: "Reads a value from runtime metadata supplied by the host.",
        null_handling: Some("Missing key returns null."),
        deterministic: false,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "runtimeMeta('tenantId')", result_json: "\"acme-42\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Formspec,
    },
    BuiltinFunctionCatalogEntry {
        name: "pluralCategory",
        category: "locale",
        parameters: &[
            Parameter {
                name: "count",
                fel_type: FelType::Number,
                description: Some("Numeric count for plural rule selection."),
                required: true,
                variadic: false,
                allowed_values: None,
            },
            Parameter {
                name: "localeTag",
                fel_type: FelType::String,
                description: Some("Optional BCP 47 tag; defaults to locale()."),
                required: false,
                variadic: false,
                allowed_values: None,
            },
        ],
        returns: FelType::String,
        return_description: None,
        description: "Returns the CLDR cardinal plural category (zero, one, two, few, many, other) for the count and locale.",
        null_handling: Some("Null count returns null."),
        deterministic: true,
        emit_deterministic_explicitly: true,
        short_circuit: false,
        examples: &[
            Example { expression: "pluralCategory(1)", result_json: "\"one\"", note: None },
            Example { expression: "pluralCategory(2, 'en')", result_json: "\"other\"", note: None },
        ],
        since_version: "1.0",
        package: Package::Formspec,
    },
    // ── mip ──────────────────────────────────────────────────────────────────
    BuiltinFunctionCatalogEntry {
        name: "valid",
        category: "mip",
        parameters: &[Parameter {
            name: "path",
            fel_type: FelType::String,
            description: Some("Field path (NOT evaluated as expression — extracted as literal path string)."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Boolean,
        return_description: None,
        description: "Returns true if the field at the given path has zero validation errors. The argument is a field reference path, not a general expression — the parser extracts it as a literal string rather than evaluating it.",
        null_handling: Some("N/A — path is a literal reference."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "valid($totalBudget)", result_json: "true", note: None },
            Example { expression: "valid($email) and valid($phone)", result_json: "false", note: None },
        ],
        since_version: "1.0",
        package: Package::Formspec,
    },
    BuiltinFunctionCatalogEntry {
        name: "relevant",
        category: "mip",
        parameters: &[Parameter {
            name: "path",
            fel_type: FelType::String,
            description: Some("Field path (literal, not evaluated)."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Boolean,
        return_description: None,
        description: "Returns the computed relevance (visibility) state of the field at the given path. True means the field is visible/active.",
        null_handling: Some("N/A — path is a literal reference."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "relevant($spouseInfo)", result_json: "false", note: None },
        ],
        since_version: "1.0",
        package: Package::Formspec,
    },
    BuiltinFunctionCatalogEntry {
        name: "readonly",
        category: "mip",
        parameters: &[Parameter {
            name: "path",
            fel_type: FelType::String,
            description: Some("Field path (literal, not evaluated)."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Boolean,
        return_description: None,
        description: "Returns the computed readonly state of the field at the given path.",
        null_handling: Some("N/A — path is a literal reference."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "readonly($approvedAmount)", result_json: "true", note: None },
        ],
        since_version: "1.0",
        package: Package::Formspec,
    },
    BuiltinFunctionCatalogEntry {
        name: "required",
        category: "mip",
        parameters: &[Parameter {
            name: "path",
            fel_type: FelType::String,
            description: Some("Field path (literal, not evaluated)."),
            required: true,
            variadic: false,
            allowed_values: None,
        }],
        returns: FelType::Boolean,
        return_description: None,
        description: "Returns the computed required state of the field at the given path.",
        null_handling: Some("N/A — path is a literal reference."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "required($email)", result_json: "true", note: None },
        ],
        since_version: "1.0",
        package: Package::Formspec,
    },
    // ── repeat ───────────────────────────────────────────────────────────────
    BuiltinFunctionCatalogEntry {
        name: "prev",
        category: "repeat",
        parameters: &[Parameter {
            name: "fieldName",
            fel_type: FelType::String,
            description: Some("Name of the sibling field to read from the previous repeat instance."),
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
        examples: &[
            Example { expression: "prev('runningTotal')", result_json: "500", note: Some("Value from previous row") },
        ],
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
        examples: &[
            Example { expression: "next('amount')", result_json: "200", note: Some("Peek at next row's value") },
        ],
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
        examples: &[
            Example { expression: "parent('projectName')", result_json: "\"Infrastructure Upgrade\"", note: None },
        ],
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
                description: Some("Name of the secondary data source (must match a key in the definition's 'instances' object)."),
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
        null_handling: Some("Returns null/undefined if instance name not found or path doesn't exist."),
        deterministic: true,
        emit_deterministic_explicitly: false,
        short_circuit: false,
        examples: &[
            Example { expression: "@instance('priorYear').totalExpenditure", result_json: "200000", note: None },
            Example { expression: "@instance('agencies')", result_json: "[{\"code\": \"DOE\", \"name\": \"Dept of Energy\"}]", note: None },
        ],
        since_version: "1.0",
        package: Package::Formspec,
    },
];

/// Emit a single `Parameter` as its schema JSON object.
fn emit_parameter(p: &Parameter) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert("name".into(), serde_json::Value::String(p.name.into()));
    obj.insert("type".into(), serde_json::Value::String(p.fel_type.as_str().into()));
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
                vals.iter().map(|v| serde_json::Value::String((*v).into())).collect(),
            ),
        );
    }
    serde_json::Value::Object(obj)
}

/// Emit a single `Example` as its schema JSON object.
fn emit_example(ex: &Example) -> serde_json::Value {
    let result: serde_json::Value = serde_json::from_str(ex.result_json)
        .unwrap_or_else(|e| panic!("invalid result_json for example '{}': {e}", ex.expression));
    let mut obj = serde_json::Map::new();
    obj.insert("expression".into(), serde_json::Value::String(ex.expression.into()));
    obj.insert("result".into(), result);
    if let Some(note) = ex.note {
        obj.insert("note".into(), serde_json::Value::String(note.into()));
    }
    serde_json::Value::Object(obj)
}

/// Emit a single `BuiltinFunctionCatalogEntry` as its `FunctionEntry` JSON object.
fn emit_function_entry(e: &BuiltinFunctionCatalogEntry) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert("name".into(), serde_json::Value::String(e.name.into()));
    obj.insert("category".into(), serde_json::Value::String(e.category.into()));
    obj.insert(
        "parameters".into(),
        serde_json::Value::Array(e.parameters.iter().map(emit_parameter).collect()),
    );
    obj.insert("returns".into(), serde_json::Value::String(e.returns.as_str().into()));
    if let Some(rd) = e.return_description {
        obj.insert("returnDescription".into(), serde_json::Value::String(rd.into()));
    }
    obj.insert("description".into(), serde_json::Value::String(e.description.into()));
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
        obj.insert("deterministic".into(), serde_json::Value::Bool(e.deterministic));
    }
    if e.short_circuit {
        obj.insert("shortCircuit".into(), serde_json::Value::Bool(true));
    }
    if !e.examples.is_empty() {
        obj.insert(
            "examples".into(),
            serde_json::Value::Array(e.examples.iter().map(emit_example).collect()),
        );
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

/// Slice of all built-in functions (names reserved for [`ExtensionRegistry::register`]).
pub fn builtin_function_catalog() -> &'static [BuiltinFunctionCatalogEntry] {
    BUILTIN_FUNCTIONS
}

/// Returns a JSON array of all builtin function entries (compact form, suitable for tooling that
/// iterates the catalog). For the full normative schema document, use [`emit_schema_json`].
pub fn builtin_function_catalog_json_value() -> serde_json::Value {
    serde_json::Value::Array(BUILTIN_FUNCTIONS.iter().map(emit_function_entry).collect())
}

/// Catalog filtered to entries reachable from `package`.
///
/// `Package::Formspec` returns the union of `Universal` and `Formspec` entries
/// (formspec hosts can call everything). `Package::Universal` returns only
/// `Universal` entries — appropriate for hosts that use [`crate::MapEnvironment`] or
/// any non-formspec [`crate::Environment`] implementation.
pub fn builtin_function_catalog_for(
    package: Package,
) -> impl Iterator<Item = &'static BuiltinFunctionCatalogEntry> {
    BUILTIN_FUNCTIONS.iter().filter(move |e| match package {
        Package::Universal => matches!(e.package, Package::Universal),
        Package::Formspec => true, // Universal ∪ Formspec
    })
}

/// `builtin_function_catalog_for(package)` rendered as a JSON array of function entries.
pub fn builtin_function_catalog_json_value_for(package: Package) -> serde_json::Value {
    serde_json::Value::Array(
        builtin_function_catalog_for(package)
            .map(emit_function_entry)
            .collect(),
    )
}

/// Error type for extension registration failures.
#[derive(Debug, Clone)]
pub enum ExtensionError {
    /// Registration rejected: name matches a reserved word or built-in function.
    NameConflict(String),
}

impl std::fmt::Display for ExtensionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtensionError::NameConflict(name) => {
                write!(
                    f,
                    "cannot register extension '{name}': conflicts with reserved word or built-in function"
                )
            }
        }
    }
}

impl std::error::Error for ExtensionError {}

impl ExtensionRegistry {
    /// Empty registry (no custom extensions).
    pub fn new() -> Self {
        Self {
            extensions: HashMap::new(),
        }
    }

    /// Register an extension function.
    ///
    /// Returns an error if the name conflicts with a reserved word or built-in.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        min_args: usize,
        max_args: Option<usize>,
        func: impl Fn(&[TypeValue]) -> TypeValue + Send + Sync + 'static,
    ) -> Result<(), ExtensionError> {
        let name = name.into();

        if RESERVED_WORDS.contains(&name.as_str())
            || BUILTIN_FUNCTIONS
                .iter()
                .any(|entry| entry.name == name.as_str())
        {
            return Err(ExtensionError::NameConflict(name));
        }

        self.extensions.insert(
            name.clone(),
            ExtensionFunc {
                name: name.clone(),
                min_args,
                max_args,
                func: Box::new(func),
            },
        );
        Ok(())
    }

    /// Look up an extension function by name.
    /// Lookup registered extension by name.
    pub fn get(&self, name: &str) -> Option<&ExtensionFunc> {
        self.extensions.get(name)
    }

    /// Check if a name is registered.
    /// True if `name` is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.extensions.contains_key(name)
    }

    /// Call an extension function with null propagation.
    ///
    /// If any argument is null, returns null without calling the function.
    /// Returns None if the extension is not found.
    /// Invoke extension if present; returns `None` if unknown (caller may treat as undefined function).
    pub fn call(&self, name: &str, args: &[TypeValue]) -> Option<TypeValue> {
        let ext = self.extensions.get(name)?;

        // Null propagation: any null arg → null result
        if args.iter().any(|a| a.is_null()) {
            return Some(TypeValue::Null);
        }

        Some((ext.func)(args))
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_docs_in_private_items)]
    use super::*;
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
        assert_eq!(registry.call("double", &[num(5)]), Some(num(10)));
    }

    #[test]
    fn test_null_propagation() {
        let mut registry = ExtensionRegistry::new();
        registry
            .register("identity", 1, Some(1), |args| args[0].clone())
            .unwrap();

        assert_eq!(
            registry.call("identity", &[TypeValue::Null]),
            Some(TypeValue::Null)
        );
        assert_eq!(registry.call("identity", &[num(42)]), Some(num(42)));
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
        assert_eq!(registry.call("unknownExt", &[num(1)]), None);
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
            Some(s("a-b-c"))
        );
    }

    #[test]
    fn package_filter_universal_excludes_formspec() {
        let universal: Vec<_> = builtin_function_catalog_for(Package::Universal).collect();
        assert!(universal.iter().all(|e| matches!(e.package, Package::Universal)));
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
    fn emit_schema_has_72_functions() {
        let v = emit_schema_json();
        let funcs = v["functions"].as_array().expect("functions array");
        assert_eq!(funcs.len(), 72, "expected 72 functions, got {}", funcs.len());
    }

    #[test]
    fn result_json_parses_for_all_examples() {
        for entry in BUILTIN_FUNCTIONS {
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
}
