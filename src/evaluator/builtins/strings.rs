#![allow(clippy::missing_docs_in_private_items)]
use regex::RegexBuilder;
use rust_decimal::prelude::*;

use crate::ast::*;
use crate::types::*;

use super::super::core::Evaluator;
use super::super::util::dec;

impl<'a> Evaluator<'a> {
    // ── String helpers ──────────────────────────────────────────

    pub(in crate::evaluator) fn fn_str1(
        &mut self,
        args: &[Expr],
        fn_name: &str,
        f: fn(&str) -> Value,
    ) -> Value {
        match self.eval_arg(args, 0) {
            Value::String(s) => f(&s),
            Value::Null => Value::Null,
            other => self.reject_expected_type(fn_name, "string", &other),
        }
    }

    pub(in crate::evaluator) fn fn_str2(
        &mut self,
        args: &[Expr],
        name: &str,
        f: fn(&str, &str) -> Value,
    ) -> Value {
        let s = match self.eval_arg(args, 0) {
            Value::String(s) => s,
            Value::Null => return Value::Null,
            other => return self.reject_expected_type(name, "string", &other),
        };
        let s2 = match self.eval_arg(args, 1) {
            Value::String(s) => s,
            Value::Null => return Value::Null,
            other => return self.reject_expected_type(name, "string", &other),
        };
        f(&s, &s2)
    }

    pub(in crate::evaluator) fn fn_length(&mut self, args: &[Expr]) -> Value {
        match self.eval_arg(args, 0) {
            Value::String(s) => Value::Number(dec(s.chars().count() as i64)),
            Value::Array(a) => Value::Number(dec(a.len() as i64)),
            Value::Null => Value::Number(Decimal::ZERO),
            other => self.reject_expected_type("length", "string or array", &other),
        }
    }

    pub(in crate::evaluator) fn fn_substring(&mut self, args: &[Expr]) -> Value {
        let s = match self.eval_arg(args, 0) {
            Value::String(s) => s,
            Value::Null => return Value::Null,
            other => return self.reject_expected_type("substring", "string", &other),
        };
        let start = match self.eval_arg(args, 1) {
            Value::Number(n) => n.to_i64().unwrap_or(1).max(1) as usize,
            Value::Null => return Value::Null,
            other => return self.reject_expected_type("substring", "number", &other),
        };
        let chars: Vec<char> = s.chars().collect();
        let start_idx = start.saturating_sub(1);
        if args.len() > 2 {
            let len = match self.eval_arg(args, 2) {
                Value::Number(n) => n.to_i64().unwrap_or(0).max(0) as usize,
                Value::Null => return Value::Null,
                other => return self.reject_expected_type("substring", "number", &other),
            };
            let end = (start_idx + len).min(chars.len());
            Value::String(chars[start_idx.min(chars.len())..end].iter().collect())
        } else {
            Value::String(chars[start_idx.min(chars.len())..].iter().collect())
        }
    }

    pub(in crate::evaluator) fn fn_replace(&mut self, args: &[Expr]) -> Value {
        let s = match self.eval_arg(args, 0) {
            Value::String(s) => s,
            Value::Null => return Value::Null,
            other => return self.reject_expected_type("replace", "string", &other),
        };
        let old = match self.eval_arg(args, 1) {
            Value::String(s) => s,
            Value::Null => return Value::Null,
            other => return self.reject_expected_type("replace", "string", &other),
        };
        let new = match self.eval_arg(args, 2) {
            Value::String(s) => s,
            Value::Null => return Value::Null,
            other => return self.reject_expected_type("replace", "string", &other),
        };
        Value::String(s.replace(&old, &new))
    }

    pub(in crate::evaluator) fn fn_matches(&mut self, args: &[Expr]) -> Value {
        let s = match self.eval_arg(args, 0) {
            Value::String(s) => s,
            Value::Null => return Value::Null,
            other => return self.reject_expected_type("matches", "string", &other),
        };
        let pattern = match self.eval_arg(args, 1) {
            Value::String(s) => s,
            Value::Null => return Value::Null,
            other => return self.reject_expected_type("matches", "string", &other),
        };
        match RegexBuilder::new(&pattern).size_limit(1_000_000).build() {
            Ok(re) => Value::Boolean(re.is_match(&s)),
            Err(e) => {
                self.diag(format!(
                    "matches: invalid regex pattern '{}': {}",
                    pattern, e
                ));
                Value::Null
            }
        }
    }

    pub(in crate::evaluator) fn fn_format(&mut self, args: &[Expr]) -> Value {
        if args.is_empty() {
            return Value::Null;
        }
        let template = match self.eval(&args[0]) {
            Value::String(s) => s,
            Value::Null => return Value::Null,
            other => return self.reject_expected_type("format", "string", &other),
        };
        let values: Vec<String> = args[1..]
            .iter()
            .map(|arg| self.eval(arg).to_string())
            .collect();
        let mut result = template;
        for (i, value) in values.iter().enumerate() {
            result = result.replace(&format!("{{{i}}}"), value);
        }
        if result.contains("%s") {
            let mut sequential = String::with_capacity(result.len());
            let mut rest = result.as_str();
            let mut value_index = 0usize;
            while let Some(pos) = rest.find("%s") {
                sequential.push_str(&rest[..pos]);
                if let Some(value) = values.get(value_index) {
                    sequential.push_str(value);
                    value_index += 1;
                } else {
                    sequential.push_str("%s");
                }
                rest = &rest[pos + 2..];
            }
            sequential.push_str(rest);
            result = sequential;
        }
        Value::String(result)
    }
}
