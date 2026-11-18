//! Declares the names and form fields that mean the same thing in each language.
// Rust guideline compliant 2026-02-21

use super::native::Source;
use super::syntax::Form;
use super::{Declaration, InputType, Language};
use crate::ast::PathSegment;
use crate::{Expr, parse};
use std::collections::BTreeSet;

/// A CEL variable and the FEL reference emitted for it.
pub(super) struct Input {
    /// CEL variable name.
    pub(super) name: String,
    /// Author's field used to enforce unique reverse mappings.
    field: Expr,
    /// Direct field or named-instance reference.
    pub(super) reference: Expr,
    /// Type guaranteed by the caller.
    pub(super) kind: InputType,
}

/// Collect value names, respecting the local variable in count filters.
fn cel_names(node: &Form, names: &mut BTreeSet<String>) {
    if let Some((list, _)) = super::count::non_null_count(node) {
        return cel_names(list, names);
    }
    match node {
        Form::Path(name) => {
            names.insert(name.clone());
        }
        Form::Call(_, args) | Form::List(args) => {
            for a in args {
                cel_names(a, names);
            }
        }
        Form::Object(entries) => {
            for (_, val) in entries {
                cel_names(val, names);
            }
        }
        Form::Not(arg) => cel_names(arg, names),
        Form::Binary(_, a, b) => {
            cel_names(a, names);
            cel_names(b, names);
        }
        Form::Choice(c, t, f) => {
            for x in [c, t, f] {
                cel_names(x, names);
            }
        }
        _ => {}
    }
}

/// Map undeclared value names to same-name nullable FEL fields.
pub(super) fn default_fields(source: &Source) -> Vec<Declaration> {
    let mut fields = BTreeSet::new();
    match source {
        Source::Fel(tree) => {
            for path in crate::extract_dependencies(tree).fields {
                if path.split('.').all(crate::is_valid_fel_identifier) {
                    fields.insert(path);
                }
            }
        }
        Source::Cel(tree) => cel_names(tree, &mut fields),
    }
    fields
        .into_iter()
        .map(|name| Declaration {
            field: format!("${name}"),
            name,
            kind: InputType::NullableString,
        })
        .collect()
}

/// Validated field mappings shared by both directions.
pub(super) struct Bindings {
    /// Unique input references in declaration order.
    pub(super) inputs: Vec<Input>,
}

impl Bindings {
    /// Validate identifiers, field paths and uniqueness before translation.
    pub(super) fn new(
        declared: &[Declaration],
        snapshot: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut inputs: Vec<Input> = Vec::with_capacity(declared.len());
        let mut names = BTreeSet::new();
        for declaration in declared {
            let Declaration {
                name,
                field: path,
                kind,
            } = declaration;
            if !name.split('.').all(|part| {
                let mut bytes = part.bytes();
                bytes
                    .next()
                    .is_some_and(|b| b == b'_' || b.is_ascii_alphabetic())
                    && bytes.all(|b| b == b'_' || b.is_ascii_alphanumeric())
            }) {
                return Err(format!("{name:?} must be a dotted CEL identifier").into());
            }
            if !names.insert(name) {
                return Err(format!("duplicate CEL binding {name}").into());
            }
            if !matches!(Language::Cel.parse(name, false)?, Form::Path(ref path) if path == name)
                || (snapshot && name.contains('.'))
                || matches!(
                    name.as_str(),
                    "bool"
                        | "bytes"
                        | "double"
                        | "duration"
                        | "int"
                        | "list"
                        | "map"
                        | "null_type"
                        | "string"
                        | "timestamp"
                        | "type"
                        | "uint"
                )
            {
                return Err(format!("{name:?} must be an available CEL variable name").into());
            }
            let field = parse(path)?;
            if !matches!(&field, Expr::FieldRef { name: Some(_), path }
                if path.iter().all(|part| matches!(part, PathSegment::Dot(_))
                    || (*kind == InputType::List && matches!(part, PathSegment::Wildcard))))
            {
                return Err(format!(
                    "{path:?} must be a named FEL field path. List bindings can include [*]"
                )
                .into());
            }
            if inputs.iter().any(|input| input.field == field) {
                return Err(format!("two CEL names map to the same FEL field {path}").into());
            }
            let reference = if snapshot {
                parse(&format!("@instance('celInputs').{name}"))?
            } else {
                field.clone()
            };
            inputs.push(Input {
                name: name.to_owned(),
                field,
                reference,
                kind: *kind,
            });
        }
        Ok(Self { inputs })
    }
}
