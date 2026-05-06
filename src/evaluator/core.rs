#![allow(clippy::missing_docs_in_private_items)]
use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::ast::*;
use crate::convert::fel_to_json;
use crate::error::Diagnostic;
use crate::extensions::ExtensionRegistry;
use crate::trace::{Trace, TraceStep};
use crate::types::*;

use super::util::{binary_op_symbol, dec, is_eager_traceable_function, render_field_path};

// ── Evaluation context ──────────────────────────────────────────

/// Resolves `$` field paths, `@` context, MIP queries, repeat navigation, and clock for FEL builtins.
pub trait Environment {
    /// Resolve `$a.b` style path as segment list (`["a","b"]`); empty slice is bare `$`.
    fn resolve_field(&self, segments: &[String]) -> Value;
    /// Resolve `@name`, `@name('arg')`, `@name.tail`.
    fn resolve_context(&self, name: &str, arg: Option<&str>, tail: &[String]) -> Value;

    /// `valid($path)` — default `true` when not overridden.
    fn mip_valid(&self, _path: &[String]) -> Value {
        Value::Boolean(true)
    }
    /// `relevant($path)` — default `true`.
    fn mip_relevant(&self, _path: &[String]) -> Value {
        Value::Boolean(true)
    }
    /// `readonly($path)` — default `false`.
    fn mip_readonly(&self, _path: &[String]) -> Value {
        Value::Boolean(false)
    }
    /// `required($path)` — default `false`.
    fn mip_required(&self, _path: &[String]) -> Value {
        Value::Boolean(false)
    }

    /// `prev()` in repeat scope — default null.
    fn repeat_prev(&self) -> Value {
        Value::Null
    }
    /// `next()` in repeat scope — default null.
    fn repeat_next(&self) -> Value {
        Value::Null
    }
    /// `parent()` in repeat scope — default null.
    fn repeat_parent(&self) -> Value {
        Value::Null
    }
    /// Calendar date for `today()` — default none (evaluator may still use literals).
    fn current_date(&self) -> Option<Date> {
        None
    }
    /// Date-time for `now()` — default none.
    fn current_datetime(&self) -> Option<Date> {
        None
    }
    /// Active locale code for `locale()` — default none (returns null).
    fn locale(&self) -> Option<&str> {
        None
    }
    /// Runtime metadata value for `runtimeMeta(key)` — default null.
    fn runtime_meta(&self, _key: &str) -> Value {
        Value::Null
    }
}

/// Flat `HashMap` environment for tests and simple hosts (no `@` context; fixed clock in default impl).
pub struct MapEnvironment {
    /// Top-level and nested values (nested via object values); keys may be dotted.
    pub fields: HashMap<String, Value>,
    /// Clock source for `today()` / `now()` lookups.
    pub current_datetime: Option<Date>,
}

impl MapEnvironment {
    /// Empty field map.
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
            current_datetime: Some(Date::DateTime {
                year: 2026,
                month: 3,
                day: 20,
                hour: 0,
                minute: 0,
                second: 0,
            }),
        }
    }

    /// Pre-populated field map.
    pub fn with_fields(fields: HashMap<String, Value>) -> Self {
        Self {
            fields,
            ..Self::new()
        }
    }

    /// Override the environment clock used by `today()` and `now()`.
    pub fn with_current_datetime(mut self, current_datetime: Option<Date>) -> Self {
        self.current_datetime = current_datetime;
        self
    }
}

/// Empty `MapEnvironment` (delegates to [`MapEnvironment::new`]).
impl Default for MapEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

/// Default clock: pinned to 2026-03-20 for deterministic tests/examples; resolves nested fields by dotted key or object walk.
impl Environment for MapEnvironment {
    fn resolve_field(&self, segments: &[String]) -> Value {
        if segments.is_empty() {
            return Value::Null;
        }
        let key = segments.join(".");
        if let Some(val) = self.fields.get(&key) {
            return val.clone();
        }
        // Walk nested objects
        let mut current = match self.fields.get(&segments[0]) {
            Some(v) => v.clone(),
            None => return Value::Null,
        };
        for seg in &segments[1..] {
            match &current {
                Value::Object(entries) => match entries.iter().find(|(k, _)| k == seg) {
                    Some((_, v)) => current = v.clone(),
                    None => return Value::Null,
                },
                _ => return Value::Null,
            }
        }
        current
    }

    fn resolve_context(&self, _name: &str, _arg: Option<&str>, _tail: &[String]) -> Value {
        Value::Null
    }

    fn current_date(&self) -> Option<Date> {
        match &self.current_datetime {
            Some(Date::Date { year, month, day }) => Some(Date::Date {
                year: *year,
                month: *month,
                day: *day,
            }),
            Some(Date::DateTime {
                year, month, day, ..
            }) => Some(Date::Date {
                year: *year,
                month: *month,
                day: *day,
            }),
            None => None,
        }
    }

    fn current_datetime(&self) -> Option<Date> {
        self.current_datetime.clone()
    }
}

/// Result of evaluation: a value plus any accumulated diagnostics.
#[derive(Debug, Clone)]
pub struct EvalResult {
    /// Computed value (may be null after errors).
    pub value: Value,
    /// Non-fatal issues (undefined functions, type errors, etc.).
    pub diagnostics: Vec<Diagnostic>,
}

/// Tree-walking evaluator with `let` scopes and diagnostic collection.
pub struct Evaluator<'a> {
    pub(super) env: &'a dyn Environment,
    pub(super) extensions: Option<&'a ExtensionRegistry>,
    pub(super) diagnostics: Vec<Diagnostic>,
    pub(super) let_scopes: Vec<HashMap<String, Value>>,
    /// Optional evaluation trace. `None` on the hot path, `Some` when the
    /// caller entered via [`evaluate_with_trace`].
    pub(super) trace: Option<Trace>,
    /// Stack of per-call argument caches used by traceable eager functions.
    call_arg_cache_stack: Vec<CallArgCache>,
}

#[derive(Debug)]
struct CallArgCache {
    args_ptr: *const Expr,
    values: Vec<Option<Value>>,
}

/// Evaluate an expression against an environment.
pub fn evaluate(expr: &Expr, env: &dyn Environment) -> EvalResult {
    let mut evaluator = Evaluator {
        env,
        extensions: None,
        diagnostics: Vec::new(),
        let_scopes: Vec::new(),
        trace: None,
        call_arg_cache_stack: Vec::new(),
    };
    let value = evaluator.eval(expr);
    EvalResult {
        value,
        diagnostics: evaluator.diagnostics,
    }
}

/// Evaluate an expression with optional extension registry fallback for unknown functions.
pub fn evaluate_with_extensions(
    expr: &Expr,
    env: &dyn Environment,
    extensions: &ExtensionRegistry,
) -> EvalResult {
    let mut evaluator = Evaluator {
        env,
        extensions: Some(extensions),
        diagnostics: Vec::new(),
        let_scopes: Vec::new(),
        trace: None,
        call_arg_cache_stack: Vec::new(),
    };
    let value = evaluator.eval(expr);
    EvalResult {
        value,
        diagnostics: evaluator.diagnostics,
    }
}

/// Evaluate and simultaneously record a structured [`Trace`] of key steps.
///
/// The returned [`EvalResult`] has identical `value` and `diagnostics` to
/// [`evaluate`] for the same input; only the side-channel trace is new.
/// Tracing is opt-in because it allocates per-step and projects values to
/// JSON — negligible for interactive use, but worth avoiding on hot paths.
pub fn evaluate_with_trace(expr: &Expr, env: &dyn Environment) -> (EvalResult, Trace) {
    let mut evaluator = Evaluator {
        env,
        extensions: None,
        diagnostics: Vec::new(),
        let_scopes: Vec::new(),
        trace: Some(Trace::new()),
        call_arg_cache_stack: Vec::new(),
    };
    let value = evaluator.eval(expr);
    let trace = evaluator.trace.take().unwrap_or_default();
    (
        EvalResult {
            value,
            diagnostics: evaluator.diagnostics,
        },
        trace,
    )
}

impl<'a> Evaluator<'a> {
    pub(super) fn diag(&mut self, msg: impl Into<String>) {
        self.diagnostics.push(Diagnostic::error(msg));
    }

    /// Evaluates `pred` with `$` bound to `elem` for one let-scope frame.
    pub(super) fn eval_under_dollar(&mut self, elem: &Value, pred: &Expr) -> Value {
        self.let_scopes
            .push(HashMap::from([("$".to_string(), elem.clone())]));
        let out = self.eval(pred);
        self.let_scopes.pop();
        out
    }

    /// Emits `{fn_name}: requires {min} arguments` and returns `false` when `args.len() < min`.
    pub(super) fn require_min_args(&mut self, args: &[Expr], min: usize, fn_name: &str) -> bool {
        if args.len() < min {
            self.diag(format!("{fn_name}: requires {min} arguments"));
            return false;
        }
        true
    }

    /// True iff this evaluator is recording a trace. Cheap predictable branch.
    #[inline]
    pub(super) fn tracing(&self) -> bool {
        self.trace.is_some()
    }

    /// Append a step, no-op when not tracing.
    #[inline]
    pub(super) fn trace_step(&mut self, step: TraceStep) {
        if let Some(t) = self.trace.as_mut() {
            t.push(step);
        }
    }
}

impl<'a> Evaluator<'a> {
    pub(super) fn eval(&mut self, expr: &Expr) -> Value {
        match expr {
            Expr::Null => Value::Null,
            Expr::Boolean(b) => Value::Boolean(*b),
            Expr::Number(n) => Value::Number(*n),
            Expr::String(s) => Value::String(s.clone()),
            Expr::DateLiteral(s) => match parse_date_literal(s) {
                Some(d) => Value::Date(d),
                None => {
                    self.diag(format!("invalid date literal '{s}'"));
                    Value::Null
                }
            },
            Expr::DateTimeLiteral(s) => match parse_datetime_literal(s) {
                Some(d) => Value::Date(d),
                None => {
                    self.diag(format!("invalid datetime literal '{s}'"));
                    Value::Null
                }
            },
            Expr::Array(elems) => Value::Array(elems.iter().map(|e| self.eval(e)).collect()),
            Expr::Object(entries) => Value::Object(
                entries
                    .iter()
                    .map(|(k, v)| (k.clone(), self.eval(v)))
                    .collect(),
            ),
            Expr::FieldRef { name, path } => {
                let value = self.eval_field_ref(name, path);
                if self.tracing() {
                    let rendered = render_field_path(name, path);
                    self.trace_step(TraceStep::FieldResolved {
                        path: rendered,
                        value: fel_to_json(&value),
                    });
                }
                value
            }
            Expr::ContextRef { name, arg, tail } => {
                self.env.resolve_context(name, arg.as_deref(), tail)
            }
            Expr::UnaryOp { op, operand, .. } => {
                let val = self.eval(operand);
                self.eval_unary(*op, val)
            }
            Expr::BinaryOp { op, left, right } => self.eval_binary(*op, left, right),
            Expr::Ternary {
                condition,
                then_branch,
                else_branch,
            }
            | Expr::IfThenElse {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.eval(condition);
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
                        self.eval(then_branch)
                    }
                    Value::Boolean(false) => {
                        if self.tracing() {
                            self.trace_step(TraceStep::IfBranch {
                                condition_value: serde_json::Value::Bool(false),
                                branch_taken: "else",
                            });
                        }
                        self.eval(else_branch)
                    }
                    _ => {
                        self.diag(format!(
                            "if: condition must be boolean, got {}",
                            cond.type_name()
                        ));
                        Value::Null
                    }
                }
            }
            Expr::Membership {
                value,
                container,
                negated,
            } => {
                let val = self.eval(value);
                let cont = self.eval(container);
                self.eval_membership(val, cont, *negated)
            }
            Expr::NullCoalesce { left, right } => {
                let l = self.eval(left);
                if l.is_null() { self.eval(right) } else { l }
            }
            Expr::LetBinding { name, value, body } => {
                let val = self.eval(value);
                self.let_scopes.push(HashMap::from([(name.clone(), val)]));
                let result = self.eval(body);
                self.let_scopes.pop();
                result
            }
            Expr::FunctionCall { name, args } => {
                let should_trace_call = self.tracing() && is_eager_traceable_function(name);
                if should_trace_call {
                    self.begin_call_arg_cache(args);
                }
                let result = self.eval_function(name, args);
                if should_trace_call {
                    let arg_vals = self.finish_call_arg_cache_as_json(args);
                    self.trace_step(TraceStep::FunctionCalled {
                        name: name.clone(),
                        args: arg_vals,
                        result: fel_to_json(&result),
                    });
                }
                result
            }
            Expr::PostfixAccess { expr, path } => {
                if let Expr::FieldRef {
                    name: Some(name),
                    path: base_path,
                } = expr.as_ref()
                {
                    let mut segments = vec![name.clone()];
                    let mut combined = Vec::with_capacity(base_path.len() + path.len());
                    combined.extend(base_path.iter().cloned());
                    combined.extend(path.iter().cloned());
                    if combined
                        .iter()
                        .all(|segment| matches!(segment, PathSegment::Dot(_)))
                    {
                        // Let-bound identifiers resolve in `eval_field_ref` before the environment.
                        // Skipping `eval(expr)` would merge path segments and call `resolve_field`
                        // only, so e.g. `let x = {a: 1} in x.a` would wrongly yield null.
                        let bound_in_let =
                            self.let_scopes.iter().any(|scope| scope.contains_key(name));
                        if !bound_in_let {
                            for segment in &combined {
                                if let PathSegment::Dot(part) = segment {
                                    segments.push(part.clone());
                                }
                            }
                            return self.env.resolve_field(&segments);
                        }
                    }
                }
                let base = self.eval(expr);
                self.access_path(base, path)
            }
        }
    }

    // ── Field references ────────────────────────────────────────

    fn eval_field_ref(&mut self, name: &Option<String>, path: &[PathSegment]) -> Value {
        match name {
            None => {
                // Check let-scopes for bare `$` (predicate rebound in countWhere / every / some
                // and in *Where helpers via filter_where).
                for scope in self.let_scopes.iter().rev() {
                    if let Some(val) = scope.get("$") {
                        return if path.is_empty() {
                            val.clone()
                        } else {
                            self.access_path(val.clone(), path)
                        };
                    }
                }
                let base = self.env.resolve_field(&[]);
                if path.is_empty() {
                    base
                } else {
                    self.access_path(base, path)
                }
            }
            Some(n) => {
                // Check let-scopes first
                for scope in self.let_scopes.iter().rev() {
                    if let Some(val) = scope.get(n) {
                        return if path.is_empty() {
                            val.clone()
                        } else {
                            self.access_path(val.clone(), path)
                        };
                    }
                }
                // Build segments for environment resolution
                let mut segments = vec![n.clone()];
                let mut remaining_path = Vec::new();
                let mut hit_special = false;
                for seg in path {
                    if hit_special {
                        remaining_path.push(seg.clone());
                    } else {
                        match seg {
                            PathSegment::Dot(name) => segments.push(name.clone()),
                            _ => {
                                hit_special = true;
                                remaining_path.push(seg.clone());
                            }
                        }
                    }
                }
                let base = self.env.resolve_field(&segments);
                if matches!(base, Value::Null)
                    && path.iter().any(|seg| matches!(seg, PathSegment::Index(_)))
                {
                    let mut flat_segments = vec![n.clone()];
                    for seg in path {
                        match seg {
                            PathSegment::Dot(name) => flat_segments.push(name.clone()),
                            PathSegment::Index(idx) => {
                                if let Some(last) = flat_segments.last_mut() {
                                    last.push_str(&format!("[{idx}]"));
                                }
                            }
                            PathSegment::Wildcard => {
                                if let Some(last) = flat_segments.last_mut() {
                                    last.push_str("[*]");
                                }
                            }
                        }
                    }
                    let flat = self.env.resolve_field(&flat_segments);
                    if !matches!(flat, Value::Null) {
                        return flat;
                    }
                }
                if remaining_path.is_empty() {
                    base
                } else {
                    self.access_path(base, &remaining_path)
                }
            }
        }
    }

    fn access_path(&mut self, mut current: Value, path: &[PathSegment]) -> Value {
        for (i, seg) in path.iter().enumerate() {
            match seg {
                PathSegment::Dot(name) => match &current {
                    Value::Object(entries) => {
                        current = entries
                            .iter()
                            .find(|(k, _)| k == name)
                            .map(|(_, v)| v.clone())
                            .unwrap_or(Value::Null);
                    }
                    Value::Null => return Value::Null,
                    _ => {
                        self.diag(format!("cannot access '{name}' on {}", current.type_name()));
                        return Value::Null;
                    }
                },
                PathSegment::Index(idx) => match &current {
                    Value::Array(arr) => {
                        if *idx == 0 || *idx > arr.len() {
                            self.diag(format!("index {idx} out of bounds (len {})", arr.len()));
                            return Value::Null;
                        }
                        current = arr[*idx - 1].clone();
                    }
                    Value::Null => return Value::Null,
                    _ => {
                        self.diag(format!("cannot index into {}", current.type_name()));
                        return Value::Null;
                    }
                },
                PathSegment::Wildcard => match &current {
                    Value::Array(arr) => {
                        let remaining = &path[i + 1..];
                        if remaining.is_empty() {
                            return current;
                        }
                        return Value::Array(
                            arr.iter()
                                .map(|e| self.access_path(e.clone(), remaining))
                                .collect(),
                        );
                    }
                    Value::Null => return Value::Null,
                    _ => {
                        self.diag(format!("cannot wildcard on {}", current.type_name()));
                        return Value::Null;
                    }
                },
            }
        }
        current
    }

    // ── Unary operators ─────────────────────────────────────────

    fn eval_unary(&mut self, op: UnaryOp, val: Value) -> Value {
        match op {
            UnaryOp::Not => match val {
                Value::Null => Value::Null,
                Value::Boolean(b) => Value::Boolean(!b),
                _ => {
                    self.diag(format!("cannot apply 'not' to {}", val.type_name()));
                    Value::Null
                }
            },
            UnaryOp::Neg => match &val {
                Value::Null => Value::Null,
                Value::Number(n) => Value::Number(-n),
                Value::Array(arr) => {
                    Value::Array(arr.iter().map(|v| self.eval_unary(op, v.clone())).collect())
                }
                _ => {
                    self.diag(format!("cannot negate {}", val.type_name()));
                    Value::Null
                }
            },
        }
    }

    // ── Binary operators ────────────────────────────────────────

    fn eval_binary(&mut self, op: BinaryOp, left_expr: &Expr, right_expr: &Expr) -> Value {
        // Short-circuit for logical ops
        match op {
            BinaryOp::And => {
                let left = self.eval(left_expr);
                if left.is_null() {
                    return Value::Null;
                }
                let left_bool = match left {
                    Value::Boolean(b) => b,
                    other => {
                        self.diag(format!("cannot apply 'and' to {}", other.type_name()));
                        return Value::Null;
                    }
                };
                if !left_bool {
                    if self.tracing() {
                        self.trace_step(TraceStep::ShortCircuit {
                            op: "and".into(),
                            reason: "left of 'and' was false, skipped right".into(),
                        });
                    }
                    return Value::Boolean(false);
                }
                let right = self.eval(right_expr);
                if right.is_null() {
                    return Value::Null;
                }
                return match right {
                    Value::Boolean(b) => {
                        if self.tracing() {
                            self.trace_step(TraceStep::BinaryOp {
                                op: "and".into(),
                                lhs: serde_json::Value::Bool(true),
                                rhs: serde_json::Value::Bool(b),
                                result: serde_json::Value::Bool(b),
                            });
                        }
                        Value::Boolean(b)
                    }
                    other => {
                        self.diag(format!("cannot apply 'and' to {}", other.type_name()));
                        Value::Null
                    }
                };
            }
            BinaryOp::Or => {
                let left = self.eval(left_expr);
                if left.is_null() {
                    return Value::Null;
                }
                let left_bool = match left {
                    Value::Boolean(b) => b,
                    other => {
                        self.diag(format!("cannot apply 'or' to {}", other.type_name()));
                        return Value::Null;
                    }
                };
                if left_bool {
                    if self.tracing() {
                        self.trace_step(TraceStep::ShortCircuit {
                            op: "or".into(),
                            reason: "left of 'or' was true, skipped right".into(),
                        });
                    }
                    return Value::Boolean(true);
                }
                let right = self.eval(right_expr);
                if right.is_null() {
                    return Value::Null;
                }
                return match right {
                    Value::Boolean(b) => {
                        if self.tracing() {
                            self.trace_step(TraceStep::BinaryOp {
                                op: "or".into(),
                                lhs: serde_json::Value::Bool(false),
                                rhs: serde_json::Value::Bool(b),
                                result: serde_json::Value::Bool(b),
                            });
                        }
                        Value::Boolean(b)
                    }
                    other => {
                        self.diag(format!("cannot apply 'or' to {}", other.type_name()));
                        Value::Null
                    }
                };
            }
            _ => {}
        }

        let left = self.eval(left_expr);
        let right = self.eval(right_expr);

        // Equality does NOT propagate null
        match op {
            BinaryOp::Eq => {
                let result = self.eval_equality(&left, &right);
                if self.tracing()
                    && !matches!(left, Value::Array(_))
                    && !matches!(right, Value::Array(_))
                {
                    self.trace_step(TraceStep::BinaryOp {
                        op: "==".into(),
                        lhs: fel_to_json(&left),
                        rhs: fel_to_json(&right),
                        result: fel_to_json(&result),
                    });
                }
                return result;
            }
            BinaryOp::NotEq => {
                let eq = self.eval_equality(&left, &right);
                let result = match eq {
                    Value::Boolean(b) => Value::Boolean(!b),
                    other => other,
                };
                if self.tracing()
                    && !matches!(left, Value::Array(_))
                    && !matches!(right, Value::Array(_))
                {
                    self.trace_step(TraceStep::BinaryOp {
                        op: "!=".into(),
                        lhs: fel_to_json(&left),
                        rhs: fel_to_json(&right),
                        result: fel_to_json(&result),
                    });
                }
                return result;
            }
            _ => {}
        }

        // Array broadcasting
        match (&left, &right) {
            (Value::Array(la), Value::Array(ra)) => {
                if la.len() != ra.len() {
                    self.diag(format!(
                        "array length mismatch: {} vs {}",
                        la.len(),
                        ra.len()
                    ));
                    return Value::Null;
                }
                return Value::Array(
                    la.iter()
                        .zip(ra.iter())
                        .map(|(l, r)| self.apply_binary(op, l, r))
                        .collect(),
                );
            }
            (Value::Array(la), _) => {
                return Value::Array(
                    la.iter()
                        .map(|l| self.apply_binary(op, l, &right))
                        .collect(),
                );
            }
            (_, Value::Array(ra)) => {
                return Value::Array(ra.iter().map(|r| self.apply_binary(op, &left, r)).collect());
            }
            _ => {}
        }

        let result = self.apply_binary(op, &left, &right);
        if self.tracing() {
            self.trace_step(TraceStep::BinaryOp {
                op: binary_op_symbol(op).into(),
                lhs: fel_to_json(&left),
                rhs: fel_to_json(&right),
                result: fel_to_json(&result),
            });
        }
        result
    }

    fn apply_binary(&mut self, op: BinaryOp, left: &Value, right: &Value) -> Value {
        if left.is_null() || right.is_null() {
            return Value::Null;
        }

        match op {
            BinaryOp::Add => self.num_op(left, right, "+", |a, b| a + b),
            BinaryOp::Sub => self.num_op(left, right, "-", |a, b| a - b),
            BinaryOp::Mul => self.num_op(left, right, "*", |a, b| a * b),
            BinaryOp::Div => {
                if let (Value::Number(a), Value::Number(b)) = (left, right) {
                    if b.is_zero() {
                        self.diag("division by zero");
                        Value::Null
                    } else {
                        Value::Number(a / b)
                    }
                } else if let (Value::Money(m), Value::Number(n)) = (left, right) {
                    if n.is_zero() {
                        self.diag("division by zero");
                        Value::Null
                    } else {
                        Value::Money(Money {
                            amount: m.amount / n,
                            currency: m.currency.clone(),
                        })
                    }
                } else if let (Value::Money(a), Value::Money(b)) = (left, right) {
                    if a.currency != b.currency {
                        self.diag(format!(
                            "currency mismatch: {} vs {}",
                            a.currency, b.currency
                        ));
                        Value::Null
                    } else if b.amount.is_zero() {
                        self.diag("division by zero");
                        Value::Null
                    } else {
                        Value::Number(a.amount / b.amount)
                    }
                } else {
                    self.diag(format!(
                        "cannot divide {} by {}",
                        left.type_name(),
                        right.type_name()
                    ));
                    Value::Null
                }
            }
            BinaryOp::Mod => {
                if let (Value::Number(a), Value::Number(b)) = (left, right) {
                    if b.is_zero() {
                        self.diag("modulo by zero");
                        Value::Null
                    } else {
                        Value::Number(a % b)
                    }
                } else if let (Value::Money(m), Value::Number(n)) = (left, right) {
                    if n.is_zero() {
                        self.diag("modulo by zero");
                        Value::Null
                    } else {
                        Value::Money(Money {
                            amount: m.amount % n,
                            currency: m.currency.clone(),
                        })
                    }
                } else {
                    self.diag(format!(
                        "cannot modulo {} by {}",
                        left.type_name(),
                        right.type_name()
                    ));
                    Value::Null
                }
            }
            BinaryOp::Concat => {
                if let (Value::String(a), Value::String(b)) = (left, right) {
                    Value::String(format!("{a}{b}"))
                } else {
                    self.diag(format!(
                        "cannot concat {} and {}",
                        left.type_name(),
                        right.type_name()
                    ));
                    Value::Null
                }
            }
            BinaryOp::Lt => self.compare(left, right, |o| o.is_lt()),
            BinaryOp::Gt => self.compare(left, right, |o| o.is_gt()),
            BinaryOp::LtEq => self.compare(left, right, |o| o.is_le()),
            BinaryOp::GtEq => self.compare(left, right, |o| o.is_ge()),
            BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::And | BinaryOp::Or => {
                unreachable!("handled above")
            }
        }
    }

    fn num_op(
        &mut self,
        left: &Value,
        right: &Value,
        sym: &str,
        f: fn(Decimal, Decimal) -> Decimal,
    ) -> Value {
        match (left, right) {
            (Value::Number(a), Value::Number(b)) => Value::Number(f(*a, *b)),
            (Value::Money(a), Value::Money(b)) if sym == "+" || sym == "-" => {
                if a.currency != b.currency {
                    self.diag(format!(
                        "currency mismatch: {} vs {}",
                        a.currency, b.currency
                    ));
                    Value::Null
                } else {
                    Value::Money(Money {
                        amount: f(a.amount, b.amount),
                        currency: a.currency.clone(),
                    })
                }
            }
            (Value::Money(m), Value::Number(n)) if sym == "+" || sym == "-" => {
                Value::Money(Money {
                    amount: f(m.amount, *n),
                    currency: m.currency.clone(),
                })
            }
            (Value::Money(m), Value::Number(n)) if sym == "*" => Value::Money(Money {
                amount: m.amount * n,
                currency: m.currency.clone(),
            }),
            (Value::Number(n), Value::Money(m)) if sym == "*" => Value::Money(Money {
                amount: *n * m.amount,
                currency: m.currency.clone(),
            }),
            _ => {
                self.diag(format!(
                    "cannot apply '{sym}' to {} and {}",
                    left.type_name(),
                    right.type_name()
                ));
                Value::Null
            }
        }
    }

    pub(super) fn eval_equality(&mut self, left: &Value, right: &Value) -> Value {
        match (left, right) {
            (Value::Null, Value::Null) => Value::Boolean(true),
            (Value::Null, _) | (_, Value::Null) => Value::Boolean(false),
            (Value::Boolean(a), Value::Boolean(b)) => Value::Boolean(a == b),
            (Value::Number(a), Value::Number(b)) => Value::Boolean(a == b),
            (Value::String(a), Value::String(b)) => Value::Boolean(a == b),
            (Value::Date(a), Value::Date(b)) => Value::Boolean(a == b),
            (Value::Money(a), Value::Money(b)) => {
                Value::Boolean(a.currency == b.currency && a.amount == b.amount)
            }
            (Value::Array(a), Value::Array(b)) => {
                if a.len() != b.len() {
                    return Value::Boolean(false);
                }
                for (av, bv) in a.iter().zip(b.iter()) {
                    if !matches!(self.eval_equality(av, bv), Value::Boolean(true)) {
                        return Value::Boolean(false);
                    }
                }
                Value::Boolean(true)
            }
            (Value::Object(a), Value::Object(b)) => Value::Boolean(a == b),
            _ => {
                self.diag(format!(
                    "cannot compare {} with {}",
                    left.type_name(),
                    right.type_name()
                ));
                Value::Null
            }
        }
    }

    fn compare(
        &mut self,
        left: &Value,
        right: &Value,
        check: fn(std::cmp::Ordering) -> bool,
    ) -> Value {
        let ord = match (left, right) {
            (Value::Number(a), Value::Number(b)) => a.cmp(b),
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Date(a), Value::Date(b)) => a.ordinal().cmp(&b.ordinal()),
            // 9f: Money vs Number comparison — specific diagnostic with fix suggestion
            (Value::Money(_), Value::Number(_)) | (Value::Number(_), Value::Money(_)) => {
                self.diag("Type error: cannot compare money with number directly. Use moneyAmount($field) to extract the numeric amount.");
                return Value::Null;
            }
            // Money vs Money ordering — only equality is supported
            (Value::Money(_), Value::Money(_)) => {
                self.diag("Type error: cannot order money values directly. Use moneyAmount($field) to extract the numeric amount for ordering comparisons.");
                return Value::Null;
            }
            _ => {
                self.diag(format!(
                    "cannot compare {} with {}",
                    left.type_name(),
                    right.type_name()
                ));
                return Value::Null;
            }
        };
        Value::Boolean(check(ord))
    }

    fn eval_membership(&mut self, value: Value, container: Value, negated: bool) -> Value {
        match &container {
            Value::Array(arr) => {
                let found = arr
                    .iter()
                    .any(|e| matches!(self.eval_equality(&value, e), Value::Boolean(true)));
                Value::Boolean(if negated { !found } else { found })
            }
            Value::Null => Value::Null,
            _ => {
                self.diag(format!(
                    "membership requires array, got {}",
                    container.type_name()
                ));
                Value::Null
            }
        }
    }

    // ── Standard library functions ──────────────────────────────

    fn eval_function(&mut self, name: &str, args: &[Expr]) -> Value {
        match name {
            // Aggregates
            "sum" => self.fn_aggregate(args, "sum", |nums| nums.iter().copied().sum()),
            "count" => {
                let v = self.eval_arg(args, 0);
                self.fn_count(&v)
            }
            "avg" => self.fn_aggregate(args, "avg", |nums| {
                if nums.is_empty() {
                    Decimal::ZERO
                } else {
                    nums.iter().copied().sum::<Decimal>() / Decimal::from(nums.len() as i64)
                }
            }),
            "min" => self.fn_min_max(args, true),
            "max" => self.fn_min_max(args, false),
            "countWhere" => self.fn_count_where(args),
            "every" => self.fn_every(args),
            "some" => self.fn_some(args),
            "sumWhere" => self.fn_sum_where(args),
            "avgWhere" => self.fn_avg_where(args),
            "minWhere" => self.fn_min_where(args),
            "maxWhere" => self.fn_max_where(args),

            // String
            "length" => self.fn_length(args),
            "contains" => self.fn_str2(args, "contains", |s, sub| Value::Boolean(s.contains(sub))),
            "startsWith" => {
                self.fn_str2(args, "startsWith", |s, p| Value::Boolean(s.starts_with(p)))
            }
            "endsWith" => self.fn_str2(args, "endsWith", |s, p| Value::Boolean(s.ends_with(p))),
            "substring" => self.fn_substring(args),
            "replace" => self.fn_replace(args),
            "upper" => self.fn_str1(args, |s| Value::String(s.to_uppercase())),
            "lower" => self.fn_str1(args, |s| Value::String(s.to_lowercase())),
            "trim" => self.fn_str1(args, |s| Value::String(s.trim().to_string())),
            "matches" => self.fn_matches(args),
            "format" => self.fn_format(args),

            // Numeric
            "round" => self.fn_round(args),
            "floor" => self.fn_num1(args, |n| n.floor()),
            "ceil" => self.fn_num1(args, |n| n.ceil()),
            "abs" => self.fn_num1(args, |n| n.abs()),
            "power" => self.fn_power(args),

            // Date
            "today" => self.fn_today(),
            "now" => self.fn_now(),
            "year" => self.fn_date_part(args, |d| dec(d.year() as i64)),
            "month" => self.fn_date_part(args, |d| dec(d.month() as i64)),
            "day" => self.fn_date_part(args, |d| dec(d.day() as i64)),
            "hours" => self.fn_time_part(args, 0),
            "minutes" => self.fn_time_part(args, 1),
            "seconds" => self.fn_time_part(args, 2),
            "time" => self.fn_time(args),
            "timeDiff" => self.fn_time_diff(args),
            "duration" => self.fn_duration(args),
            "dateDiff" => self.fn_date_diff(args),
            "dateAdd" => self.fn_date_add(args),

            // Logical
            "if" => self.fn_if(args),
            "coalesce" => self.fn_coalesce(args),
            "empty" => self.fn_empty(args),
            "present" => {
                let e = self.fn_empty(args);
                match e {
                    Value::Boolean(b) => Value::Boolean(!b),
                    o => o,
                }
            }
            "selected" => self.fn_selected(args),

            // Type checking
            "isNumber" => self.fn_is_type(args, "number"),
            "isString" => self.fn_is_type(args, "string"),
            "isDate" => self.fn_is_type(args, "date"),
            "isNull" => {
                let v = self.eval_arg(args, 0);
                Value::Boolean(v.is_null())
            }
            "typeOf" => {
                let v = self.eval_arg(args, 0);
                Value::String(v.type_name().to_string())
            }

            // Casting
            "number" => self.fn_cast_number(args),
            "string" => self.fn_cast_string(args),
            "boolean" => self.fn_cast_boolean(args),
            "date" => self.fn_cast_date(args),

            // Money
            "money" => self.fn_money(args),
            "moneyAmount" => {
                let v = self.eval_arg(args, 0);
                match v {
                    Value::Money(m) => Value::Number(m.amount),
                    _ => Value::Null,
                }
            }
            "moneyCurrency" => {
                let v = self.eval_arg(args, 0);
                match v {
                    Value::Money(m) => Value::String(m.currency),
                    _ => Value::Null,
                }
            }
            "moneyAdd" => self.fn_money_add(args),
            "moneySum" => self.fn_money_sum(args),
            "moneySumWhere" => self.fn_money_sum_where(args),

            // MIP state queries
            "valid" => self.fn_mip(args, "valid"),
            "relevant" => self.fn_mip(args, "relevant"),
            "readonly" => self.fn_mip(args, "readonly"),
            "required" => self.fn_mip(args, "required"),

            // Repeat navigation
            "prev" => self.env.repeat_prev(),
            "next" => self.env.repeat_next(),
            "parent" => self.env.repeat_parent(),
            "instance" => self.fn_instance(args),

            // Locale
            "locale" => self.fn_locale(),
            "runtimeMeta" => self.fn_runtime_meta(args),
            "pluralCategory" => self.fn_plural_category(args),

            _ => {
                let evaluated_args: Vec<Value> = args.iter().map(|arg| self.eval(arg)).collect();
                if let Some(registry) = self.extensions {
                    if let Some(result) = registry.call(name, &evaluated_args) {
                        return result;
                    }
                }
                self.diagnostics.push(Diagnostic::undefined_function(name));
                Value::Null
            }
        }
    }

    pub(super) fn eval_arg(&mut self, args: &[Expr], idx: usize) -> Value {
        let args_ptr = args.as_ptr();
        if idx < args.len() {
            let use_top_cache = self
                .call_arg_cache_stack
                .last()
                .is_some_and(|cache| cache.args_ptr == args_ptr && idx < cache.values.len());
            if use_top_cache {
                if let Some(val) = self
                    .call_arg_cache_stack
                    .last()
                    .and_then(|cache| cache.values[idx].as_ref())
                {
                    return val.clone();
                }
                let evaluated = self.eval(&args[idx]);
                if let Some(cache) = self.call_arg_cache_stack.last_mut()
                    && cache.args_ptr == args_ptr
                    && idx < cache.values.len()
                {
                    cache.values[idx] = Some(evaluated.clone());
                }
                return evaluated;
            }
            self.eval(&args[idx])
        } else {
            Value::Null
        }
    }

    pub(super) fn get_array<'v>(&mut self, val: &'v Value, fn_name: &str) -> Option<&'v [Value]> {
        match val {
            Value::Array(a) => Some(a.as_slice()),
            Value::Null => None,
            _ => {
                self.diag(format!(
                    "{fn_name}: expected array, got {}",
                    val.type_name()
                ));
                None
            }
        }
    }

    fn begin_call_arg_cache(&mut self, args: &[Expr]) {
        self.call_arg_cache_stack.push(CallArgCache {
            args_ptr: args.as_ptr(),
            values: vec![None; args.len()],
        });
    }

    fn finish_call_arg_cache_as_json(&mut self, args: &[Expr]) -> Vec<serde_json::Value> {
        let mut cache = self
            .call_arg_cache_stack
            .pop()
            .expect("call arg cache stack should contain current call");
        assert_eq!(
            cache.args_ptr,
            args.as_ptr(),
            "call arg cache stack mismatch for traced function call"
        );
        cache
            .values
            .iter_mut()
            .enumerate()
            .map(|(idx, slot)| {
                let value = slot.take().unwrap_or_else(|| self.eval(&args[idx]));
                fel_to_json(&value)
            })
            .collect()
    }

    /// Filter array elements by predicate (shared by sumWhere / avgWhere / minWhere / maxWhere / moneySumWhere).
    pub(super) fn filter_where(&mut self, args: &[Expr], fn_name: &str) -> Option<Vec<Value>> {
        if !self.require_min_args(args, 2, fn_name) {
            return None;
        }
        let arr_val = self.eval(&args[0]);
        let arr = self.get_array(&arr_val, fn_name)?;
        let mut matched = Vec::new();
        for elem in arr {
            let pred = self.eval_under_dollar(elem, &args[1]);
            if pred.is_truthy() {
                matched.push(elem.clone());
            }
        }
        Some(matched)
    }
}
