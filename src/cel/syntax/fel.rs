//! Read FEL source for the reversible syntax converter.
// Rust guideline compliant 2026-02-21

use super::{Form, atom, binary, function, operation, portable};
use crate::ast::{BinaryOp as B, PathSegment as P, UnaryOp};
use crate::{Expr, print_expr};

/// Parse FEL source before recognizing editable notation.
pub(super) fn parse(source: &str, marked: bool) -> Result<Form, String> {
    crate::parse(source)
        .map(|e| read(&e, marked))
        .map_err(|e| e.to_string())
}

/// Preserve the FEL syntax tree in the common editing representation.
fn read(expr: &Expr, marked: bool) -> Form {
    let child = |e| read(e, marked);
    match expr {
        Expr::Null => Form::Null,
        Expr::Boolean(v) => Form::Bool(*v),
        Expr::String(s) => Form::String(s.clone()),
        Expr::Number(_) => {
            let s = print_expr(expr);
            s.parse()
                .map(Form::Int)
                .unwrap_or_else(|_| atom("bridge_fel_decimal", s))
        }
        Expr::DateLiteral(_) => atom("bridge_fel_date", print_expr(expr)),
        Expr::DateTimeLiteral(_) => atom("bridge_fel_datetime", print_expr(expr)),
        Expr::Array(xs) => Form::List(xs.iter().map(child).collect()),
        Expr::Object(xs) => Form::Object(xs.iter().map(|(k, v)| (k.clone(), child(v))).collect()),
        Expr::FieldRef { name, path } => {
            if name.as_deref().is_some_and(portable)
                && path.iter().all(|p| matches!(p,P::Dot(k) if portable(k)))
            {
                return Form::Path(print_expr(expr)[1..].to_owned());
            }
            atom("bridge_fel_field", print_expr(expr))
        }
        Expr::VarRef { .. } => atom("bridge_fel_variable", print_expr(expr)),
        Expr::ContextRef { .. } => atom("bridge_fel_context", print_expr(expr)),
        Expr::UnaryOp {
            op: UnaryOp::Neg,
            operand,
            ..
        } => {
            if matches!(operand.as_ref(), Expr::Number(_))
                && let Ok(n) = format!("-{}", print_expr(operand)).parse()
                && n != 0
            {
                return Form::Int(n);
            }
            operation("bridge_fel_negate", vec![child(operand)])
        }
        Expr::UnaryOp {
            op: UnaryOp::Not,
            operand,
            bang: true,
        } => Form::Not(Box::new(child(operand))),
        Expr::UnaryOp {
            op: UnaryOp::Not,
            operand,
            bang: false,
        } => operation("bridge_fel_not", vec![child(operand)]),
        Expr::BinaryOp { op, left, right } => {
            let a = child(left);
            let b = child(right);
            let glyph = match op {
                B::Add => "+",
                B::Sub => "-",
                B::Mul => "*",
                B::Mod => "%",
                B::Eq => "==",
                B::NotEq => "!=",
                B::Lt => "<",
                B::LtEq => "<=",
                B::Gt => ">",
                B::GtEq => ">=",
                B::And => "&&",
                B::Or => "||",
                B::Div => return operation("bridge_fel_div", vec![a, b]),
                B::Concat => return operation("bridge_fel_concat", vec![a, b]),
            };
            binary(glyph, a, b)
        }
        Expr::Ternary {
            condition,
            then_branch,
            else_branch,
        } => Form::Choice(
            Box::new(child(condition)),
            Box::new(child(then_branch)),
            Box::new(child(else_branch)),
        ),
        Expr::IfThenElse {
            condition,
            then_branch,
            else_branch,
        } => operation(
            "bridge_fel_if",
            vec![child(condition), child(then_branch), child(else_branch)],
        ),
        Expr::Membership {
            value,
            container,
            negated: false,
        } => binary("in", child(value), child(container)),
        Expr::Membership {
            value,
            container,
            negated: true,
        } => operation("bridge_fel_not_in", vec![child(value), child(container)]),
        Expr::NullCoalesce { left, right } => {
            operation("bridge_fel_coalesce", vec![child(left), child(right)])
        }
        Expr::LetBinding { name, value, body } => Form::Call(
            "bridge_fel_let".into(),
            vec![Form::String(name.clone()), child(value), child(body)],
        ),
        Expr::FunctionCall { name, args } => function(
            name,
            args.iter().map(child).collect(),
            marked,
            "bridge_fel_call",
        ),
        Expr::PostfixAccess { expr, path } => {
            let field = Expr::FieldRef {
                name: None,
                path: path.clone(),
            };
            Form::Call(
                "bridge_fel_access".into(),
                vec![
                    child(expr),
                    Form::String(print_expr(&field)[1..].to_owned()),
                ],
            )
        }
        Expr::RowLookup {
            source,
            key_path,
            key,
        } => Form::Call(
            "bridge_fel_lookup".into(),
            vec![child(source), Form::String(key_path.join(".")), child(key)],
        ),
    }
}
