//! Static builtin catalog data (`BUILTIN_FUNCTIONS`) and reserved names.
#![allow(clippy::missing_docs_in_private_items)]

use std::sync::LazyLock;

use super::types::*;

mod aggregate;
mod date;
mod locale;
mod logical;
mod mip;
mod money;
mod numeric;
mod repeat;
mod string;
mod type_;

pub(crate) const RESERVED_WORDS: &[&str] = &[
    "true", "false", "null", "let", "in", "if", "then", "else", "and", "or", "not",
];

pub(crate) static BUILTIN_FUNCTIONS: LazyLock<Vec<BuiltinFunctionCatalogEntry>> =
    LazyLock::new(|| {
        let mut entries = Vec::new();
        entries.extend_from_slice(aggregate::ENTRIES);
        entries.extend_from_slice(string::ENTRIES);
        entries.extend_from_slice(numeric::ENTRIES);
        entries.extend_from_slice(date::ENTRIES);
        entries.extend_from_slice(logical::ENTRIES);
        entries.extend_from_slice(type_::ENTRIES);
        entries.extend_from_slice(money::ENTRIES);
        entries.extend_from_slice(locale::ENTRIES);
        entries.extend_from_slice(mip::ENTRIES);
        entries.extend_from_slice(repeat::ENTRIES);
        entries
    });

/// Slice of all built-in functions.
///
/// Names in this catalog are reserved for
/// [`ExtensionRegistry::register`](crate::extensions::ExtensionRegistry::register).
pub fn builtin_function_catalog() -> &'static [BuiltinFunctionCatalogEntry] {
    BUILTIN_FUNCTIONS.as_slice()
}

/// Catalog filtered to entries reachable from `package`.
///
/// `Package::Formspec` returns the union of `Universal` and `Formspec` entries
/// (formspec hosts can call everything). `Package::Universal` returns only
/// `Universal` entries - appropriate for hosts that use [`crate::MapEnvironment`] or
/// any non-formspec [`crate::Environment`] implementation.
pub fn builtin_function_catalog_for(
    package: Package,
) -> impl Iterator<Item = &'static BuiltinFunctionCatalogEntry> {
    BUILTIN_FUNCTIONS.iter().filter(move |e| match package {
        Package::Universal => matches!(e.package, Package::Universal),
        Package::Formspec => true,
    })
}
