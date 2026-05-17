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
/// Emit [`crate::extensions::emit_schema_json`] to regenerate
/// `formspec/schemas/fel-functions.schema.json`.
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
