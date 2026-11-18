//! Translate the shared, type-checked expression subset.
// Rust guideline compliant 2026-02-21

use super::Language;
use super::bindings::Bindings;
use super::native_types::{Kind, kind};
use super::numeric::{fractional_double, integer, shared_quotient, truncation, whole_double};
use super::syntax::Form;
use crate::ast::{BinaryOp, PathSegment, UnaryOp};
use crate::{Expr, parse, print_expr};

/// The already parsed starting language, shared with field discovery.
pub(super) enum Source {
    /// An authored FEL expression.
    Fel(Expr),
    /// An unexpanded CEL expression.
    Cel(Form),
}

impl Source {
    /// Parse original text once before discovering bindings or checking native types.
    pub(super) fn parse(source: &str, from: Language) -> Result<Self, String> {
        match from {
            Language::Cel => from.parse(source, false).map(Self::Cel),
            Language::Fel => parse(source).map(Self::Fel).map_err(|e| e.to_string()),
        }
    }
}

/// Read a CEL double without conflating it with a CEL integer.
fn double(form: &Form) -> Option<f64> {
    let Form::Call(tag, args) = form else {
        return None;
    };
    match (tag.as_str(), args.as_slice()) {
        ("bridge_cel_double", [Form::String(s)]) => s.parse().ok(),
        ("bridge_cel_prefix", [Form::String(op), child]) if op == "-(" => Some(-double(child)?),
        _ => None,
    }
}

/// Translate and check a shared CEL operator.
fn binary(op: &str, a: &Form, b: &Form, bindings: &Bindings) -> Result<Expr, String> {
    if op == "/"
        && let Some((left, right)) = double(a)
            .and_then(whole_double)
            .zip(double(b).and_then(whole_double))
    {
        if !shared_quotient(&left, &right) {
            return Err("the binary and decimal quotients are not proved identical".into());
        }
        return Ok(Expr::BinaryOp {
            op: BinaryOp::Div,
            left: Box::new(left),
            right: Box::new(right),
        });
    }
    let left = Box::new(import_cel(a, bindings)?);
    let right = Box::new(import_cel(b, bindings)?);
    let operator = match op {
        "in" => {
            return Ok(Expr::Membership {
                value: left,
                container: right,
                negated: false,
            });
        }
        "/" => {
            let rounding = truncation(&left, &right)
                .ok_or("integer division requires two signed literals with a defined quotient")?;
            return Ok(Expr::FunctionCall {
                name: rounding.into(),
                args: vec![Expr::BinaryOp {
                    op: BinaryOp::Div,
                    left,
                    right,
                }],
            });
        }
        "==" => BinaryOp::Eq,
        "!=" => BinaryOp::NotEq,
        "<" => BinaryOp::Lt,
        "<=" => BinaryOp::LtEq,
        ">" => BinaryOp::Gt,
        ">=" => BinaryOp::GtEq,
        "&&" => BinaryOp::And,
        "||" => BinaryOp::Or,
        "+" if kind(&left, bindings)? == Kind::String => BinaryOp::Concat,
        "+" => BinaryOp::Add,
        "-" => BinaryOp::Sub,
        "*" => BinaryOp::Mul,
        "%" => BinaryOp::Mod,
        _ => return Err("unsupported CEL operator".into()),
    };
    Ok(Expr::BinaryOp {
        op: operator,
        left,
        right,
    })
}

/// Import the subset whose source types have a proven FEL representation.
fn import_cel(node: &Form, bindings: &Bindings) -> Result<Expr, String> {
    let visit = |arg| import_cel(arg, bindings);
    if let Some((list, cast)) = super::count::non_null_count(node) {
        let list = visit(list)?;
        if !cast
            && !matches!(kind(&list, bindings)?, Kind::List(element)
            if matches!(*element, Kind::Unspecified | Kind::Dynamic | Kind::Null | Kind::NullableString))
        {
            return Err(
                "this null comparison has no native FEL mapping for the element type".into(),
            );
        }
        return Ok(Expr::FunctionCall {
            name: "count".into(),
            args: vec![list],
        });
    }
    Ok(match node {
        Form::Null => Expr::Null,
        Form::Bool(b) => Expr::Boolean(*b),
        Form::Int(n) => parse(&n.to_string()).map_err(|e| e.to_string())?,
        Form::String(s) => Expr::String(s.clone()),
        Form::Path(s) => bindings
            .inputs
            .iter()
            .find(|i| &i.name == s)
            .map(|i| i.reference.clone())
            .ok_or_else(|| format!("CEL name {s} has no declared FEL field"))?,
        Form::List(items) => Expr::Array(items.iter().map(visit).collect::<Result<_, _>>()?),
        Form::Object(entries) => Expr::Object(
            entries
                .iter()
                .map(|(k, v)| Ok((k.clone(), visit(v)?)))
                .collect::<Result<_, String>>()?,
        ),
        Form::Not(v) => Expr::UnaryOp {
            op: UnaryOp::Not,
            operand: Box::new(visit(v)?),
            bang: false,
        },
        Form::Binary(op, a, b) => binary(op, a, b, bindings)?,
        Form::Choice(c, t, f) => Expr::Ternary {
            condition: Box::new(visit(c)?),
            then_branch: Box::new(visit(t)?),
            else_branch: Box::new(visit(f)?),
        },
        Form::Call(tag, args) => match (tag.as_str(), args.as_slice()) {
            ("bridge_cel_double", _) => double(node)
                .and_then(fractional_double)
                .ok_or("this CEL double has no exact reversible FEL decimal literal")?,
            ("bridge_cel_uint", _) => return Err("FEL has no unsigned integer type".into()),
            ("bridge_cel_bytes", _) => return Err("FEL has no bytes literal".into()),
            ("has", _) => {
                return Err(
                    "CEL has() tests key presence. FEL present() also tests the value".into(),
                );
            }
            ("bridge_cel_map", _) => {
                return Err("native FEL object keys must be literal strings".into());
            }
            ("bridge_cel_div", [a, b]) => binary("/", a, b, bindings)?,
            ("bridge_cel_prefix", [Form::String(op), v]) if op == "-(" => Expr::UnaryOp {
                op: UnaryOp::Neg,
                operand: Box::new(visit(v)?),
                bang: false,
            },
            ("bridge_cel_index", [base, Form::Int(at)]) => {
                let index = usize::try_from(*at)
                    .ok()
                    .and_then(|i| i.checked_add(1))
                    .ok_or("index is outside the convertible range")?;
                Expr::PostfixAccess {
                    expr: Box::new(visit(base)?),
                    path: vec![PathSegment::Index(index)],
                }
            }
            ("size", [arg]) => Expr::FunctionCall {
                name: "length".into(),
                args: vec![visit(arg)?],
            },
            ("bridge_cel_method", [receiver, Form::String(name), operands @ ..]) => {
                let name = match name.as_str() {
                    "size" if operands.is_empty() => "length",
                    "contains" | "startsWith" | "endsWith" if operands.len() == 1 => name,
                    _ => return Err("this CEL method has no native FEL mapping".into()),
                };
                let mut args = vec![visit(receiver)?];
                for arg in operands {
                    args.push(visit(arg)?);
                }
                Expr::FunctionCall {
                    name: name.into(),
                    args,
                }
            }
            _ => return Err(format!("CEL call {tag} has no native FEL mapping")),
        },
    })
}

/// Print native CEL, grouping nested operators to preserve their operands.
fn export_fel(expr: &Expr, bindings: &Bindings, nested: bool) -> Result<String, String> {
    let visit = |child| export_fel(child, bindings, false);
    let group = |text| if nested { format!("({text})") } else { text };
    match expr {
        Expr::Null => Ok("null".to_owned()),
        Expr::Boolean(value) => Ok(value.to_string()),
        Expr::Number(n) => Ok(n.normalize().to_string()),
        Expr::String(value) => serde_json::to_string(value).map_err(|e| e.to_string()),
        Expr::Array(items) => Ok(format!(
            "[{}]",
            items
                .iter()
                .map(visit)
                .collect::<Result<Vec<_>, _>>()?
                .join(", ")
        )),
        Expr::Object(entries) => {
            let mut fields = Vec::new();
            for (key, item) in entries {
                let key = serde_json::to_string(key).map_err(|e| e.to_string())?;
                fields.push(format!("{key}: {}", visit(item)?));
            }
            Ok(format!("{{{}}}", fields.join(", ")))
        }
        Expr::UnaryOp { op, operand, .. } => match op {
            UnaryOp::Not => Ok(format!("!({})", visit(operand)?)),
            UnaryOp::Neg => Ok(format!("-{}", export_fel(operand, bindings, true)?)),
        },
        Expr::BinaryOp { op, left, right } => {
            let operator = match op {
                BinaryOp::Eq => "==",
                BinaryOp::NotEq => "!=",
                BinaryOp::And => "&&",
                BinaryOp::Or => "||",
                BinaryOp::Lt => "<",
                BinaryOp::LtEq => "<=",
                BinaryOp::Gt => ">",
                BinaryOp::GtEq => ">=",
                BinaryOp::Add | BinaryOp::Concat => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Mod => "%",
                BinaryOp::Div => {
                    if let Some((a, b)) = integer(left).zip(integer(right))
                        && shared_quotient(left, right)
                    {
                        return Ok(group(format!("{a}.0 / {b}.0")));
                    }
                    return Err(
                        "FEL decimal division requires an exact numeric mapping for CEL".to_owned(),
                    );
                }
            };
            let left = export_fel(left, bindings, true)?;
            let right = export_fel(right, bindings, true)?;
            let text = format!("{left} {operator} {right}");
            // Preserve grouping even in a right-nested chain of the same operator.
            Ok(group(text))
        }
        Expr::Ternary {
            condition,
            then_branch,
            else_branch,
        } => Ok(group(format!(
            "{} ? {} : {}",
            export_fel(condition, bindings, true)?,
            export_fel(then_branch, bindings, true)?,
            export_fel(else_branch, bindings, true)?
        ))),
        Expr::Membership {
            value, container, ..
        } => Ok(group(format!(
            "{} in {}",
            export_fel(value, bindings, true)?,
            export_fel(container, bindings, true)?
        ))),
        Expr::FunctionCall { name, args } => match (name.as_str(), args.as_slice()) {
            (
                "floor" | "ceil",
                [
                    Expr::BinaryOp {
                        op: BinaryOp::Div,
                        left,
                        right,
                    },
                ],
            ) if truncation(left, right) == Some(name.as_str()) => {
                Ok(group(format!("{} / {}", visit(left)?, visit(right)?)))
            }
            ("length", [arg]) => Ok(format!("size({})", visit(arg)?)),
            ("count", [arg]) => Ok(format!(
                "{}.filter(item, dyn(item) != null).size()",
                export_fel(arg, bindings, true)?
            )),
            ("contains" | "startsWith" | "endsWith", [arg, pattern]) => Ok(format!(
                "{}.{name}({})",
                export_fel(arg, bindings, true)?,
                visit(pattern)?
            )),
            _ => Err(format!("unsupported FEL expression {}", print_expr(expr))),
        },
        Expr::PostfixAccess { expr: base, path } => {
            let mut text = export_fel(base, bindings, true)?;
            for segment in path {
                let PathSegment::Index(at) = segment else {
                    return Err("expected a literal list index".to_owned());
                };
                let cel_index = at
                    .checked_sub(1)
                    .ok_or("FEL index zero has no CEL element")?;
                use std::fmt::Write;
                write!(text, "[{cel_index}]").expect("writing into String is infallible");
            }
            Ok(text)
        }
        Expr::FieldRef { .. } | Expr::ContextRef { .. } | Expr::VarRef { .. } => bindings
            .inputs
            .iter()
            .find(|input| input.reference == *expr)
            .map(|input| input.name.clone())
            .ok_or_else(|| format!("FEL reference {} has no CEL binding here", print_expr(expr))),
        _ => Err(format!("unsupported FEL expression {}", print_expr(expr))),
    }
}

/// Normalize alternative surface spellings before comparing the two syntax trees.
fn normalize(expr: &mut Expr) {
    match expr {
        Expr::UnaryOp { operand, bang, .. } => {
            *bang = false;
            normalize(operand);
        }
        Expr::BinaryOp { left, right, .. } => {
            normalize(left);
            normalize(right);
        }
        Expr::FunctionCall { name, args } if name == "if" && args.len() == 3 => {
            let [condition, then_branch, else_branch]: [Expr; 3] = std::mem::take(args)
                .try_into()
                .expect("three arguments checked");
            *expr = Expr::Ternary {
                condition: Box::new(condition),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            };
            normalize(expr);
        }
        Expr::Array(items) | Expr::FunctionCall { args: items, .. } => {
            for item in items {
                normalize(item);
            }
        }
        Expr::Object(entries) => {
            for (_, item) in entries {
                normalize(item);
            }
        }
        Expr::PostfixAccess { expr, .. } => normalize(expr),
        Expr::IfThenElse { .. } => {
            if let Expr::IfThenElse {
                condition,
                then_branch,
                else_branch,
            } = std::mem::replace(expr, Expr::Null)
            {
                *expr = Expr::Ternary {
                    condition,
                    then_branch,
                    else_branch,
                };
                normalize(expr);
            }
        }
        Expr::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            normalize(condition);
            normalize(then_branch);
            normalize(else_branch);
        }
        Expr::Membership {
            value,
            container,
            negated,
        } => {
            normalize(value);
            normalize(container);
            if *negated {
                *negated = false;
                let operand = Box::new(std::mem::replace(expr, Expr::Null));
                *expr = Expr::UnaryOp {
                    op: UnaryOp::Not,
                    operand,
                    bang: false,
                };
            }
        }
        _ => {}
    }
}

/// Print function arguments using the parentheses already supplied by the call.
fn print_fel(expr: &Expr) -> String {
    if let Expr::FunctionCall { name, args } = expr {
        // Each argument already has a delimiter on either side.
        let args = args.iter().map(print_fel).collect::<Vec<_>>().join(", ");
        format!("{name}({args})")
    } else {
        print_expr(expr)
    }
}

/// Check both target trees before returning the requested text.
pub(super) fn translate(bindings: &Bindings, source: Source) -> Result<String, String> {
    let (mut tree, from) = match source {
        Source::Fel(tree) => (tree, Language::Fel),
        Source::Cel(tree) => (import_cel(&tree, bindings)?, Language::Cel),
    };
    normalize(&mut tree);
    kind(&tree, bindings)?;
    let fel = print_fel(&tree);
    super::limits::check(&fel, Language::Fel).map_err(|e| e.to_string())?;
    if parse(&fel).map_err(|e| e.to_string())? != tree {
        return Err("the printed FEL expression changed when parsed".into());
    }
    let cel = export_fel(&tree, bindings, false)?;
    if import_cel(&Language::Cel.parse(&cel, false)?, bindings)? != tree {
        return Err("the round trip changed the FEL expression tree".into());
    }
    Ok(match from {
        Language::Cel => fel,
        Language::Fel => cel,
    })
}
