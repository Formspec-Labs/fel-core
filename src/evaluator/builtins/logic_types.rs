#![allow(clippy::missing_docs_in_private_items)]
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

use crate::ast::*;
use crate::trace::TraceStep;
use crate::types::*;

use super::super::core::Evaluator;
use super::super::util::{extract_field_path, fel_cardinal_plural_category};

impl<'a> Evaluator<'a> {
    // ── Logical helpers ─────────────────────────────────────────

    pub(in crate::evaluator) fn fn_if(&mut self, args: &[Expr]) -> Value {
        if !self.require_exact_args(args, 3, "if") {
            return Value::Null;
        }
        let cond = self.eval(&args[0]);
        match cond {
            Value::Null => {
                self.diag("if: condition evaluated to null");
                Value::Null
            }
            Value::Boolean(true) => {
                if self.tracing() {
                    self.trace_step(TraceStep::IfBranch {
                        condition_value: serde_json::Value::Bool(true),
                        branch_taken: "then",
                    });
                }
                self.eval(&args[1])
            }
            Value::Boolean(false) => {
                if self.tracing() {
                    self.trace_step(TraceStep::IfBranch {
                        condition_value: serde_json::Value::Bool(false),
                        branch_taken: "else",
                    });
                }
                self.eval(&args[2])
            }
            other => self.reject_expected_type("if", "boolean", &other),
        }
    }

    pub(in crate::evaluator) fn fn_coalesce(&mut self, args: &[Expr]) -> Value {
        for arg in args {
            let val = self.eval(arg);
            if !val.is_null() {
                return val;
            }
        }
        Value::Null
    }

    pub(in crate::evaluator) fn fn_empty(&mut self, args: &[Expr]) -> Value {
        let val = self.eval_arg(args, 0);
        match &val {
            // Null is empty for bind/FEL semantics (e.g. `if empty(x) then …` with unset fields).
            Value::Null => Value::Boolean(true),
            Value::String(s) => Value::Boolean(s.is_empty()),
            Value::Array(a) => Value::Boolean(a.is_empty()),
            _ => Value::Boolean(false),
        }
    }

    /// Total boolean: `present(x)` ≡ `!empty(x)` with explicit handling so the result is never null.
    pub(in crate::evaluator) fn fn_present(&mut self, args: &[Expr]) -> Value {
        match self.fn_empty(args) {
            Value::Boolean(b) => Value::Boolean(!b),
            // `fn_empty` only returns Boolean; keep defensive path for API stability.
            other => other,
        }
    }

    pub(in crate::evaluator) fn fn_selected(&mut self, args: &[Expr]) -> Value {
        let container = self.eval_arg(args, 0);
        let arr = match &container {
            Value::Array(a) => a,
            Value::Null => return Value::Boolean(false),
            other => return self.reject_expected_type("selected", "array", other),
        };
        let val = self.eval_arg(args, 1);
        let found = arr
            .iter()
            .any(|e| matches!(self.eval_equality(e, &val), Value::Boolean(true)));
        Value::Boolean(found)
    }

    // ── Type checking ───────────────────────────────────────────

    pub(in crate::evaluator) fn fn_is_type(&mut self, args: &[Expr], type_name: &str) -> Value {
        let val = self.eval_arg(args, 0);
        Value::Boolean(val.type_name() == type_name)
    }

    // ── Casting ─────────────────────────────────────────────────

    pub(in crate::evaluator) fn fn_cast_number(&mut self, args: &[Expr]) -> Value {
        match self.eval_arg(args, 0) {
            Value::Number(n) => Value::Number(n),
            Value::String(s) => match s.trim().parse::<Decimal>() {
                Ok(n) => Value::Number(n),
                Err(_) => {
                    self.diag(format!("number: cannot parse '{s}'"));
                    Value::Null
                }
            },
            Value::Boolean(b) => Value::Number(if b { Decimal::ONE } else { Decimal::ZERO }),
            Value::Null => Value::Null,
            other => self.reject_expected_type(
                "number",
                "number, string, boolean, or null",
                &other,
            ),
        }
    }

    pub(in crate::evaluator) fn fn_cast_string(&mut self, args: &[Expr]) -> Value {
        let val = self.eval_arg(args, 0);
        match &val {
            Value::Null => Value::String(String::new()),
            Value::String(_) => val,
            Value::Number(n) => Value::String(format_number(*n)),
            Value::Boolean(b) => Value::String(if *b { "true" } else { "false" }.into()),
            Value::Date(d) => Value::String(d.format_iso()),
            _ => Value::String(val.to_string()),
        }
    }

    pub(in crate::evaluator) fn fn_cast_boolean(&mut self, args: &[Expr]) -> Value {
        match self.eval_arg(args, 0) {
            Value::Null => Value::Boolean(false),
            Value::Boolean(b) => Value::Boolean(b),
            Value::String(s) => match s.as_str() {
                "true" => Value::Boolean(true),
                "false" => Value::Boolean(false),
                _ => {
                    self.diag(format!("boolean: cannot convert '{s}'"));
                    Value::Null
                }
            },
            Value::Number(n) => {
                if n == Decimal::ZERO {
                    Value::Boolean(false)
                } else {
                    Value::Boolean(true)
                }
            }
            other => self.reject_expected_type(
                "boolean",
                "boolean, number, string, or null",
                &other,
            ),
        }
    }

    pub(in crate::evaluator) fn fn_cast_date(&mut self, args: &[Expr]) -> Value {
        match self.eval_arg(args, 0) {
            Value::Date(d) => Value::Date(d),
            Value::String(s) => {
                if let Some(d) = parse_date_literal(&format!("@{s}")) {
                    Value::Date(d)
                } else if let Some(d) = parse_datetime_literal(&format!("@{s}")) {
                    Value::Date(d)
                } else {
                    self.diag(format!("date: cannot parse '{s}'"));
                    Value::Null
                }
            }
            Value::Null => Value::Null,
            other => self.reject_expected_type("date", "date or string", &other),
        }
    }

    pub(in crate::evaluator) fn fn_instance(&mut self, args: &[Expr]) -> Value {
        let name = match self.eval_arg(args, 0) {
            Value::String(s) => s,
            Value::Null => return Value::Null,
            other => {
                self.diag(format!(
                    "instance: first argument must be string, got {}",
                    other.type_name()
                ));
                return Value::Null;
            }
        };

        let tail = match args.get(1) {
            None => Vec::new(),
            Some(expr) => match self.eval(expr) {
                Value::String(path) => {
                    if path.is_empty() {
                        Vec::new()
                    } else {
                        path.split('.').map(|segment| segment.to_string()).collect()
                    }
                }
                Value::Null => return Value::Null,
                other => {
                    self.diag(format!(
                        "instance: path argument must be string, got {}",
                        other.type_name()
                    ));
                    return Value::Null;
                }
            },
        };

        self.env.resolve_context("instance", Some(&name), &tail)
    }

    // ── MIP state queries ───────────────────────────────────────

    pub(in crate::evaluator) fn fn_mip(&mut self, args: &[Expr], kind: &str) -> Value {
        if args.is_empty() {
            self.diag(format!("{kind}: requires 1 argument"));
            return Value::Null;
        }
        let path = extract_field_path(&args[0]);
        match kind {
            "valid" => self.env.mip_valid(&path),
            "relevant" => self.env.mip_relevant(&path),
            "readonly" => self.env.mip_readonly(&path),
            "required" => self.env.mip_required(&path),
            _ => Value::Null,
        }
    }

    // ── Locale functions ───────────────────────────────────────────

    /// `locale()` — returns the active locale code or null.
    pub(in crate::evaluator) fn fn_locale(&self) -> Value {
        match self.env.locale() {
            Some(code) => Value::String(code.to_string()),
            None => Value::Null,
        }
    }

    /// `runtimeMeta(key)` — reads from the runtime metadata bag.
    pub(in crate::evaluator) fn fn_runtime_meta(&mut self, args: &[Expr]) -> Value {
        let key = self.eval_arg(args, 0);
        match key {
            Value::String(k) => self.env.runtime_meta(&k),
            Value::Null => Value::Null,
            _ => {
                self.diag("runtimeMeta: key must be a string".to_string());
                Value::Null
            }
        }
    }

    /// `pluralCategory(count, locale?)` — returns CLDR cardinal plural category.
    ///
    /// Uses the explicit locale parameter if provided, otherwise the environment locale.
    /// Non-integer counts use the truncated integer part (toward zero), then cardinal rules apply to that integer.
    /// Returns one of: "zero", "one", "two", "few", "many", "other".
    pub(in crate::evaluator) fn fn_plural_category(&mut self, args: &[Expr]) -> Value {
        let count_val = self.eval_arg(args, 0);
        let count = match &count_val {
            Value::Number(n) => n,
            Value::Null => return Value::Null,
            _ => {
                self.diag("pluralCategory: count must be a number".to_string());
                return Value::Null;
            }
        };

        // Determine locale: explicit arg or environment
        let locale_code = if args.len() >= 2 {
            match self.eval_arg(args, 1) {
                Value::String(s) => Some(s),
                Value::Null => return Value::Null,
                _ => {
                    self.diag("pluralCategory: locale must be a string".to_string());
                    return Value::Null;
                }
            }
        } else {
            self.env.locale().map(|s| s.to_string())
        };

        let Some(locale_str) = locale_code else {
            return Value::Null;
        };

        let n = count.trunc().to_i64().unwrap_or(0);
        match fel_cardinal_plural_category(&locale_str, n) {
            Some(cat) => Value::String(cat.to_string()),
            None => Value::Null,
        }
    }
}
