#![allow(clippy::missing_docs_in_private_items)]
use regex::RegexBuilder;
use rust_decimal::prelude::*;

use crate::ast::*;
use crate::types::*;

use super::super::core::Evaluator;
use super::super::util::dec;

impl<'a> Evaluator<'a> {
    fn push_format_tail(out: &mut String, mut rest: &str, values: &[String], percent_index: &mut usize) {
        while !rest.is_empty() {
            let Some(pos) = rest.find("%s") else {
                out.push_str(rest);
                return;
            };
            out.push_str(&rest[..pos]);
            if let Some(value) = values.get(*percent_index) {
                out.push_str(value);
                *percent_index += 1;
            } else {
                out.push_str("%s");
            }
            rest = &rest[pos + 2..];
        }
    }

    // ── String helpers ──────────────────────────────────────────

    pub(in crate::evaluator) fn fn_str1(
        &mut self,
        args: &[Expr],
        fn_name: &str,
        f: fn(&str) -> String,
    ) -> Value {
        match self.eval_arg(args, 0) {
            Value::String(s) => self.make_string(f(&s)),
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
        let start_idx = start.saturating_sub(1);
        let char_len = s.chars().count();
        if start_idx >= char_len {
            return self.make_string(String::new());
        }
        let byte_start = s
            .char_indices()
            .nth(start_idx)
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        if args.len() > 2 {
            let len = match self.eval_arg(args, 2) {
                Value::Number(n) => n.to_i64().unwrap_or(0).max(0) as usize,
                Value::Null => return Value::Null,
                other => return self.reject_expected_type("substring", "number", &other),
            };
            let end_idx = (start_idx + len).min(char_len);
            let byte_end = s
                .char_indices()
                .nth(end_idx)
                .map(|(i, _)| i)
                .unwrap_or(s.len());
            self.make_string(s[byte_start..byte_end].to_string())
        } else {
            self.make_string(s[byte_start..].to_string())
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
        self.make_string(s.replace(&old, &new))
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
            Self::push_format_tail(&mut sequential, rest, &values, &mut value_index);
            result = sequential;
        }
        self.make_string(result)
    }
}
