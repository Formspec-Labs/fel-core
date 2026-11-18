//! Convert Common Expression Language and FEL source in either direction.
//!
//! Enable the `cel-bridge` Cargo feature. Use [`convert`] to edit and return
//! an expression in its original language. Keep the version comment with the
//! converted text. Formatting, comments and equivalent literal spellings can
//! change. Language-specific constructs use editable calls in the other grammar.
//! [`convert_native`] produces ordinary expressions for the shared subset.
//! Both interfaces operate on source text alone.
//!
//! # Examples
//!
//! ```
//! use fel_core::cel::{convert, Language};
//!
//! let cel = convert("money($fee, 'USD')", Language::Fel)?;
//! let fel = convert(&cel.replace("USD", "EUR"), Language::Cel)?;
//! assert_eq!(fel, "money($fee, 'EUR')");
//! # Ok::<(), fel_core::cel::ConversionError>(())
//! ```
// Rust guideline compliant 2026-02-21

mod bindings;
mod count;
mod limits;
mod native;
mod native_types;
mod numeric;
mod syntax;

use std::fmt;
use std::str::FromStr;

/// Maximum UTF-8 byte length of an input or output expression.
pub const MAX_SOURCE_BYTES: usize = 256 * 1024;

/// Language of the input expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    /// Common Expression Language.
    Cel,
    /// Formspec Expression Language.
    Fel,
}

impl Language {
    /// Return the destination language.
    fn other(self) -> Self {
        match self {
            Self::Cel => Self::Fel,
            Self::Fel => Self::Cel,
        }
    }

    /// Return the stable language spelling used in version markers.
    fn label(self) -> &'static str {
        match self {
            Self::Cel => "CEL",
            Self::Fel => "FEL",
        }
    }
}

/// Type a caller guarantees for a named native input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InputType {
    /// A string or null, the default for undeclared fields.
    NullableString,
    /// A non-null string.
    String,
    /// A signed 64-bit integer.
    Int,
    /// A non-null Boolean.
    Bool,
    /// A non-null list of form values, which may include null elements.
    List,
}

impl FromStr for InputType {
    type Err = ConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "string?" => Ok(Self::NullableString),
            "string" => Ok(Self::String),
            "int" => Ok(Self::Int),
            "bool" => Ok(Self::Bool),
            "list" => Ok(Self::List),
            _ => Err(ConversionError::Binding(format!(
                "unknown input type {s:?}. Use string?, string, int, bool, or list"
            ))),
        }
    }
}

/// Field mapping declared by the caller.
#[derive(Clone, Debug)]
struct Declaration {
    /// Name used in CEL source.
    name: String,
    /// Type guaranteed for the bound value.
    kind: InputType,
    /// Named FEL field path.
    field: String,
}

/// Field names and types for native conversion.
#[derive(Clone, Debug, Default)]
pub struct NativeOptions {
    /// Explicit declarations, or an empty list for automatic discovery.
    bindings: Vec<Declaration>,
    /// Select the named CEL input instance when true.
    snapshot: bool,
}

impl NativeOptions {
    /// Map a CEL variable to a typed FEL field.
    ///
    /// The first declaration replaces automatic same-name field discovery.
    /// A list field can include repeat wildcards such as `$rows[*].name`.
    #[must_use]
    pub fn with_binding(
        mut self,
        name: impl Into<String>,
        kind: InputType,
        field: impl Into<String>,
    ) -> Self {
        self.bindings.push(Declaration {
            name: name.into(),
            kind,
            field: field.into(),
        });
        self
    }

    /// Address inputs through FEL's named `celInputs` instance.
    #[must_use]
    pub fn with_snapshot(mut self) -> Self {
        self.snapshot = true;
        self
    }
}

/// Reason source conversion could not produce a reversible result.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ConversionError {
    /// Invalid syntax, an invalid notation edit, or an unrepresentable expression.
    Syntax(String),
    /// An invalid, ambiguous or duplicate field declaration.
    Binding(String),
    /// The types or operations fall outside the native conversion rules.
    Native(String),
    /// The input or generated expression exceeds a converter resource limit.
    Limit(String),
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(s) | Self::Binding(s) | Self::Native(s) | Self::Limit(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for ConversionError {}

/// Convert editable source, or restore source from its versioned notation.
///
/// # Errors
///
/// Returns an error for invalid source, malformed notation or resource limits.
pub fn convert(source: &str, from: Language) -> Result<String, ConversionError> {
    syntax_conversion(source, from, false)
}

/// Convert original source whose opening comment resembles a syntax marker.
///
/// # Errors
///
/// Returns the same conversion errors as [`convert`].
pub fn convert_literal(source: &str, from: Language) -> Result<String, ConversionError> {
    syntax_conversion(source, from, true)
}

/// Apply the same bounds to original and returned text.
fn syntax_conversion(
    source: &str,
    from: Language,
    literal: bool,
) -> Result<String, ConversionError> {
    limits::check(source, from)?;
    let output = limits::with_parser_stack(|| syntax::convert(source, from, literal))
        .map_err(ConversionError::Syntax)?;
    limits::check(&output, from.other())?;
    Ok(output)
}

/// Convert source to a native expression in the other language.
///
/// Use the same options in both directions. Undeclared inputs default to
/// nullable strings. The converter checks the generated expression and its
/// return trip before yielding text.
///
/// # Errors
///
/// Returns an error for an invalid field binding, an expression outside the
/// shared native subset, a failed round trip or a resource limit.
///
/// # Examples
///
/// ```
/// use fel_core::cel::{convert_native, InputType, Language, NativeOptions};
/// let fields = NativeOptions::default().with_binding("names", InputType::List, "$rows[*].name");
/// assert_eq!(convert_native("size(names)", Language::Cel, &fields)?, "length($rows[*].name)");
/// # Ok::<(), fel_core::cel::ConversionError>(())
/// ```
pub fn convert_native(
    source: &str,
    from: Language,
    options: &NativeOptions,
) -> Result<String, ConversionError> {
    limits::check(source, from)?;
    limits::with_parser_stack(|| native_conversion(source, from, options))
}

/// Validate native input types and use the same field map for the return trip.
fn native_conversion(
    source: &str,
    from: Language,
    options: &NativeOptions,
) -> Result<String, ConversionError> {
    if syntax::has_marker(source) {
        return Err(ConversionError::Native(
            "use syntax conversion to restore marked editing notation".into(),
        ));
    }
    let source = native::Source::parse(source, from).map_err(ConversionError::Native)?;
    let discovered;
    let declarations = if options.bindings.is_empty() {
        discovered = bindings::default_fields(&source);
        &discovered
    } else {
        &options.bindings
    };
    limits::check_bindings(declarations)?;
    let fields = bindings::Bindings::new(declarations, options.snapshot)
        .map_err(|e| ConversionError::Binding(e.to_string()))?;
    let output = native::translate(&fields, source).map_err(ConversionError::Native)?;
    limits::check(&output, from.other())?;
    Ok(output)
}
