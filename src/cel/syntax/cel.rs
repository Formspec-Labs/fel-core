//! Read unexpanded CEL syntax for editing and native conversion.
// Rust guideline compliant 2026-02-21

use super::{Form, atom, binary, function, operation, portable};
use cel_syntax::common::ast::{CallExpr, EntryExpr, Expr, IdedExpr, LiteralValue, operators as op};
use std::fmt::Write;

/// Parse with optional syntax and escaped field names, keeping macros as calls.
pub(super) fn parse(source: &str, marked: bool) -> Result<Form, String> {
    let tree = cel_syntax::Env::default()
        .parser()
        .enable_optional_syntax(true)
        .enable_ident_escape_syntax(true)
        .max_recursion_depth(96)
        .parse(source)
        .map_err(|e| e.to_string())?;
    read(&tree, marked)
}

/// Quote escaped CEL field names when they cannot be printed bare.
pub(super) fn field_name(name: &str) -> String {
    if portable(name) {
        name.to_owned()
    } else {
        format!("`{name}`")
    }
}

/// Preserve signed, unsigned and floating-point types across both grammars.
fn literal(v: &LiteralValue) -> Form {
    match v {
        LiteralValue::Null => Form::Null,
        LiteralValue::Boolean(b) => Form::Bool(*b.inner()),
        LiteralValue::Int(n) => Form::Int(*n.inner()),
        LiteralValue::UInt(n) => atom("bridge_cel_uint", format!("{}u", n.inner())),
        LiteralValue::Double(n) => {
            let n = *n.inner();
            let text = if n == f64::INFINITY {
                "1e999".to_owned()
            } else if n == f64::NEG_INFINITY {
                "-1e999".to_owned()
            } else {
                format!("{n:?}")
            };
            atom("bridge_cel_double", text)
        }
        LiteralValue::String(s) => Form::String(s.inner().to_owned()),
        LiteralValue::Bytes(b) => {
            let mut hex = String::with_capacity(b.inner().len() * 2);
            for byte in b.inner() {
                write!(hex, "{byte:02x}").expect("writing to a string");
            }
            atom("bridge_cel_bytes", hex)
        }
    }
}

/// Wrap an optional element or field independently of its value.
fn optional(value: Form, enabled: bool) -> Form {
    if enabled {
        operation("bridge_cel_optional", vec![value])
    } else {
        value
    }
}

/// Read an ordinary or root-qualified CEL name.
fn name_form(name: &str) -> Form {
    if name.starts_with('.') {
        atom("bridge_cel_root", name.to_owned())
    } else if name.split('.').all(portable) {
        Form::Path(name.to_owned())
    } else {
        atom("bridge_cel_name", name.to_owned())
    }
}

/// Convert the reference parser's operator calls to editable syntax.
fn call(call: &CallExpr, marked: bool) -> Result<Form, String> {
    let name = call.func_name.as_str();
    let mut args: Vec<_> = call
        .args
        .iter()
        .map(|arg| read(arg, marked))
        .collect::<Result<_, _>>()?;
    if let Some(receiver) = &call.target {
        let mut items = vec![read(receiver, marked)?, Form::String(name.to_owned())];
        items.append(&mut args);
        return Ok(operation("bridge_cel_method", items));
    }
    let binary_op = match name {
        op::ADD => "+",
        op::SUBSTRACT => "-",
        op::MULTIPLY => "*",
        op::MODULO => "%",
        op::EQUALS => "==",
        op::NOT_EQUALS => "!=",
        op::LESS => "<",
        op::LESS_EQUALS => "<=",
        op::GREATER => ">",
        op::GREATER_EQUALS => ">=",
        op::LOGICAL_AND => "&&",
        op::LOGICAL_OR => "||",
        op::IN => "in",
        _ => "",
    };
    if !binary_op.is_empty() {
        let [a, b] = args.try_into().map_err(|_| "expected two CEL operands")?;
        return Ok(binary(binary_op, a, b));
    }
    Ok(match name {
        op::CONDITIONAL => {
            let [c, t, f] = args
                .try_into()
                .map_err(|_| "expected three conditional operands")?;
            Form::Choice(Box::new(c), Box::new(t), Box::new(f))
        }
        op::LOGICAL_NOT => {
            let [arg] = args
                .try_into()
                .map_err(|_| "expected one negated operand")?;
            Form::Not(Box::new(arg))
        }
        op::NEGATE => {
            let [arg] = args.try_into().map_err(|_| "expected one signed operand")?;
            operation(
                "bridge_cel_prefix",
                vec![Form::String("-(".to_owned()), arg],
            )
        }
        op::DIVIDE => operation("bridge_cel_div", args),
        op::INDEX => operation("bridge_cel_index", args),
        op::OPT_INDEX => operation("bridge_cel_optional_index", args),
        op::OPT_SELECT => operation("bridge_cel_optional_select", args),
        _ => function(name, args, marked, "bridge_cel_call"),
    })
}

/// Build the shared syntax tree from CEL's parsed expression.
fn read(node: &IdedExpr, marked: bool) -> Result<Form, String> {
    Ok(match &node.expr {
        Expr::Literal(v) => literal(v),
        Expr::Ident(name) => name_form(name),
        Expr::Call(c) => call(c, marked)?,
        Expr::Select(s) => {
            let base = read(&s.operand, marked)?;
            if let Form::Path(path) = &base
                && portable(&s.field)
            {
                Form::Path(format!("{path}.{}", s.field))
            } else {
                operation(
                    "bridge_cel_select",
                    vec![base, Form::String(s.field.clone())],
                )
            }
        }
        Expr::List(xs) => {
            let values = xs
                .elements
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    Ok(optional(
                        read(e, marked)?,
                        xs.optional_indices.binary_search(&i).is_ok(),
                    ))
                })
                .collect::<Result<_, String>>()?;
            if xs.optional_indices.is_empty() {
                Form::List(values)
            } else {
                operation("bridge_cel_list", values)
            }
        }
        Expr::Map(m) => {
            let strings: Option<Vec<_>> = m
                .entries
                .iter()
                .map(|entry| {
                    if let EntryExpr::MapEntry(e) = &entry.expr
                        && !e.optional
                        && let Expr::Literal(LiteralValue::String(key)) = &e.key.expr
                    {
                        Some((key.inner(), &e.value))
                    } else {
                        None
                    }
                })
                .collect();
            if let Some(fields) = strings {
                Form::Object(
                    fields
                        .into_iter()
                        .map(|(key, value)| Ok((key.to_owned(), read(value, marked)?)))
                        .collect::<Result<_, String>>()?,
                )
            } else {
                let mut pairs = Vec::with_capacity(m.entries.len());
                for entry in &m.entries {
                    let EntryExpr::MapEntry(e) = &entry.expr else {
                        return Err("expected a CEL map entry".into());
                    };
                    let pair = Form::List(vec![read(&e.key, marked)?, read(&e.value, marked)?]);
                    pairs.push(optional(pair, e.optional));
                }
                operation("bridge_cel_map", pairs)
            }
        }
        Expr::Struct(s) => {
            let mut fields = vec![name_form(&s.type_name)];
            for entry in &s.entries {
                let EntryExpr::StructField(f) = &entry.expr else {
                    return Err("expected a CEL message field".into());
                };
                fields.push(optional(
                    Form::List(vec![Form::String(f.field.clone()), read(&f.value, marked)?]),
                    f.optional,
                ));
            }
            operation("bridge_cel_message", fields)
        }
        Expr::Comprehension(_) | Expr::Unspecified => {
            return Err("expected unexpanded CEL source".into());
        }
    })
}
