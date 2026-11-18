#![allow(clippy::missing_docs_in_private_items)]
use indexmap::IndexMap;
use rust_decimal::Decimal;
use std::borrow::Cow;
use std::collections::HashMap;

use crate::ast::*;
use crate::convert::fel_to_json;
use crate::error::{Diagnostic, MissingTimezoneContextError};
use crate::extensions::{ExtensionCallOutcome, ExtensionFunctions, call_extension};
use crate::trace::{Trace, TraceStep};
use crate::types::*;

use super::budget::{BudgetExceededKind, EvalBudget};
use super::util::{binary_op_symbol, dec, is_eager_traceable_function, render_field_path};

/// Maximum [`Evaluator::eval`] recursion depth (stack overflow guard; left-associative chains are deep).
///
/// Kept modest so evaluation stops before the OS thread stack overflows (well above stress tests
/// that use ~48-term chains; far below LibFuzzer inputs that nested thousands of operators).
const MAX_EVAL_DEPTH: usize = 128;

/// Diagnostic code for unregistered host context references.
pub const UNBOUND_CONTEXT_REF_CODE: &str = "FEL-UNBOUND-CONTEXT";

// ── Evaluation context ──────────────────────────────────────────

/// A field reference the host could not resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError(String);

impl FieldError {
    /// Describe the invalid field access.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for FieldError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for FieldError {}

/// Resolves `$` field paths, `@` context, MIP queries, repeat navigation, and clock for FEL builtins.
pub trait Environment {
    /// Resolve the whole authored field reference from the current site.
    ///
    /// `None` is bare `$`. Path subscripts are 1-based. An absent value is null.
    ///
    /// # Errors
    ///
    /// Returns a field access error for an invalid subscript or path.
    fn resolve_field(&self, root: Option<&str>, path: &[PathSegment]) -> Result<Value, FieldError>;
    /// Return a precomputed result for this function call, if available at the site.
    fn precomputed(&self, _expr: &Expr) -> Option<Value> {
        None
    }
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
    /// Calendar date for `today()` per ADR 0069 D-6 — hosts MUST configure a
    /// timezone-equivalent clock or return [`MissingTimezoneContextError`].
    ///
    /// The default impl returns
    /// [`MissingTimezoneContextError::not_configured`]; this is deliberate —
    /// no implementation MAY return a silent UTC fallback. The evaluator
    /// translates `Err(_)` into a [`crate::error::DiagnosticKind::MissingTimezoneContext`]
    /// diagnostic on every call site.
    fn current_date(&self) -> Result<Date, MissingTimezoneContextError> {
        Err(MissingTimezoneContextError::not_configured())
    }
    /// Date-time for `now()` per ADR 0069 D-6 — see [`Self::current_date`] for
    /// the contract. Hosts that override one method typically override both;
    /// returning different timezone contexts from the two paths is a host bug.
    fn current_datetime(&self) -> Result<Date, MissingTimezoneContextError> {
        Err(MissingTimezoneContextError::not_configured())
    }
    /// Active locale code for `locale()` — default none (returns null).
    fn locale(&self) -> Option<&str> {
        None
    }
    /// The Locale document's pattern for a `formatDate` style (`short` … `full`), when the
    /// active locale authored one (Locale spec §2.4). Default none: built-in rendering.
    fn date_format(&self, _style: &str) -> Option<&str> {
        None
    }
    /// Runtime metadata value for `runtimeMeta(key)` — default null.
    fn runtime_meta(&self, _key: &str) -> Value {
        Value::Null
    }
    /// Response identity value for `subject(key)` or null when absent.
    fn subject_key(&self, _key: &str) -> Value {
        Value::Null
    }
}

/// Classifies a host context binding root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextBindingKind {
    /// Scalar or otherwise indivisible value; dot traversal is rejected.
    Value,
    /// Object-like root; the evaluator owns dot-segment traversal.
    Object,
    /// Callable context binding; the catalog materializes its return value.
    Function,
}

/// Materialized host context binding returned by a catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextBinding {
    /// Binding kind declared by the host catalog.
    pub kind: ContextBindingKind,
    /// Root FEL value supplied by the host catalog.
    pub value: Value,
}

impl ContextBinding {
    /// Creates a value-kind host binding.
    pub fn value(value: Value) -> Self {
        Self {
            kind: ContextBindingKind::Value,
            value,
        }
    }

    /// Creates an object-kind host binding.
    pub fn object(value: Value) -> Self {
        Self {
            kind: ContextBindingKind::Object,
            value,
        }
    }

    /// Creates a function-kind host binding result.
    pub fn function(value: Value) -> Self {
        Self {
            kind: ContextBindingKind::Function,
            value,
        }
    }
}

/// Resolves host-supplied `@name` context bindings.
///
/// The catalog returns the binding root only. Dot-segment traversal is performed
/// by the evaluator so every host receives the same path behavior.
pub trait ContextBindingCatalog {
    /// Returns the declared kind for `name`, or `None` when unregistered.
    fn binding_kind(&self, name: &str) -> Option<ContextBindingKind>;

    /// Resolves the binding root for `name`.
    ///
    /// Function bindings receive the optional string literal from `@name('arg')`.
    /// Value and object bindings receive `None`.
    fn resolve(&self, name: &str, arg: Option<&str>) -> Option<Value>;
}

/// No-op host context binding catalog.
#[derive(Default, Debug, Clone, Copy)]
pub struct EmptyCatalog;

impl ContextBindingCatalog for EmptyCatalog {
    fn binding_kind(&self, _name: &str) -> Option<ContextBindingKind> {
        None
    }

    fn resolve(&self, _name: &str, _arg: Option<&str>) -> Option<Value> {
        None
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
    ///
    /// Passing `None` removes the clock so calls to `today()` / `now()`
    /// surface `MissingTimezoneContextError::NotConfigured`. Prefer the
    /// readable [`MapEnvironment::without_clock`] constructor at call
    /// sites that intend to exercise the no-clock contract.
    pub fn with_current_datetime(mut self, current_datetime: Option<Date>) -> Self {
        self.current_datetime = current_datetime;
        self
    }

    /// Remove the environment clock so `today()` / `now()` produce the
    /// `MissingTimezoneContext` diagnostic. Read-aloud companion to
    /// [`MapEnvironment::with_current_datetime`] for the `None` case.
    pub fn without_clock(self) -> Self {
        self.with_current_datetime(None)
    }
}

/// Empty `MapEnvironment` (delegates to [`MapEnvironment::new`]).
impl Default for MapEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

impl MapEnvironment {
    fn lookup_dotted(&self, segments: &[String]) -> &Value {
        if segments.is_empty() {
            return &Value::Null;
        }
        let key = segments.join(".");
        if let Some(val) = self.fields.get(&key) {
            return val;
        }
        // Walk nested objects
        let mut current = self.fields.get(&segments[0]).unwrap_or(&Value::Null);
        for seg in &segments[1..] {
            match current {
                Value::Object(entries) => match entries.get(seg.as_str()) {
                    Some(v) => current = v,
                    None => return &Value::Null,
                },
                _ => return &Value::Null,
            }
        }
        current
    }
}

/// Default clock: pinned to 2026-03-20 for deterministic tests/examples; resolves nested fields by dotted key or object walk.
impl Environment for MapEnvironment {
    fn resolve_field(&self, root: Option<&str>, path: &[PathSegment]) -> Result<Value, FieldError> {
        crate::environment::resolve_legacy_field(root, path, |segments| {
            Cow::Borrowed(self.lookup_dotted(segments))
        })
    }

    fn resolve_context(&self, _name: &str, _arg: Option<&str>, _tail: &[String]) -> Value {
        Value::Null
    }

    fn current_date(&self) -> Result<Date, MissingTimezoneContextError> {
        match &self.current_datetime {
            Some(Date::Date { year, month, day }) => Ok(Date::Date {
                year: *year,
                month: *month,
                day: *day,
            }),
            Some(Date::DateTime {
                year, month, day, ..
            }) => Ok(Date::Date {
                year: *year,
                month: *month,
                day: *day,
            }),
            None => Err(MissingTimezoneContextError::not_configured()),
        }
    }

    fn current_datetime(&self) -> Result<Date, MissingTimezoneContextError> {
        self.current_datetime
            .clone()
            .ok_or_else(MissingTimezoneContextError::not_configured)
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
    pub(super) extensions: Option<&'a dyn ExtensionFunctions>,
    pub(super) context_bindings: Option<&'a dyn ContextBindingCatalog>,
    pub(super) diagnostics: Vec<Diagnostic>,
    pub(super) let_scopes: Vec<HashMap<String, Value>>,
    /// Optional evaluation trace. `None` on the hot path, `Some` when the
    /// caller passed [`EvaluatorOptions::trace`] to [`evaluate_with`].
    pub(super) trace: Option<Trace>,
    /// Stack of per-call argument caches used by traceable eager functions.
    call_arg_cache_stack: Vec<CallArgCache>,
    /// Nesting depth for `eval` recursion (capped at [`MAX_EVAL_DEPTH`]).
    eval_depth: usize,
    /// Resource budget for this evaluation run.
    budget: EvalBudget,
    /// Incrementing step counter checked against [`EvalBudget::max_steps`].
    step_count: u64,
    /// Best-effort allocation counter checked against [`EvalBudget::max_alloc_bytes`].
    alloc_bytes: u64,
    /// Set on first budget breach to suppress redundant diagnostics.
    budget_breached: bool,
}

/// Memoizes evaluated argument [`Value`]s for one traced eager builtin call.
///
/// [`Self::args_ptr`] equals [`slice::as_ptr`] on the `&[Expr]` argument slice passed into
/// `eval_function`. That slice is borrowed from the caller's [`Expr::FunctionCall`] and remains
/// stable for the whole call; the evaluator must not hold this cache across a point where that
/// `Vec` could reallocate. Teardown asserts the pointer still matches the slice being finalized.
#[derive(Debug)]
struct CallArgCache {
    args_ptr: *const Expr,
    values: Vec<Option<Value>>,
}

/// Configuration for an evaluation run.
pub struct EvaluatorOptions<'a> {
    /// Optional trace sink — when `Some`, the evaluator records structured steps into this trace.
    pub trace: Option<&'a mut Trace>,
    /// Optional host extension functions (Core §3.12) for resolving unknown function names.
    pub extensions: Option<&'a dyn ExtensionFunctions>,
    /// Resource budget for this evaluation run.
    pub budget: EvalBudget,
}

impl Default for EvaluatorOptions<'_> {
    fn default() -> Self {
        Self {
            trace: None,
            extensions: None,
            budget: EvalBudget::unlimited(),
        }
    }
}

fn is_grammar_reserved_context(name: &str) -> bool {
    matches!(name, "current" | "index" | "count" | "instance")
}

fn context_tail_path(tail: &[String]) -> Vec<PathSegment> {
    tail.iter().cloned().map(PathSegment::Dot).collect()
}

fn split_context_env_path(path: &[PathSegment]) -> (Vec<String>, &[PathSegment]) {
    for (index, segment) in path.iter().enumerate() {
        if !matches!(segment, PathSegment::Dot(_)) {
            let prefix = path[..index]
                .iter()
                .filter_map(|segment| match segment {
                    PathSegment::Dot(name) => Some(name.clone()),
                    _ => None,
                })
                .collect();
            return (prefix, &path[index..]);
        }
    }
    let prefix = path
        .iter()
        .filter_map(|segment| match segment {
            PathSegment::Dot(name) => Some(name.clone()),
            _ => None,
        })
        .collect();
    (prefix, &[])
}

/// Evaluate an expression against an environment (no budget, no trace, no extensions).
pub fn evaluate(expr: &Expr, env: &dyn Environment) -> EvalResult {
    evaluate_with(expr, env, EvaluatorOptions::default())
}

/// Evaluate with full configuration via [`EvaluatorOptions`].
pub fn evaluate_with(expr: &Expr, env: &dyn Environment, options: EvaluatorOptions) -> EvalResult {
    evaluate_configured(expr, env, options, None)
}

/// Evaluate with a host-supplied context binding catalog.
///
/// This sibling entry point preserves the [`EvaluatorOptions`] struct shape for
/// existing callers while enabling FEL §6.3 catalog-aware evaluation.
pub fn evaluate_with_catalog<'a>(
    expr: &Expr,
    env: &dyn Environment,
    options: EvaluatorOptions<'a>,
    catalog: &'a dyn ContextBindingCatalog,
) -> EvalResult {
    evaluate_configured(expr, env, options, Some(catalog))
}

fn evaluate_configured<'a>(
    expr: &Expr,
    env: &dyn Environment,
    mut options: EvaluatorOptions<'a>,
    context_bindings: Option<&'a dyn ContextBindingCatalog>,
) -> EvalResult {
    let caller_trace = options.trace.take();
    let wants_trace = caller_trace.is_some();
    let mut evaluator = Evaluator {
        env,
        extensions: options.extensions,
        context_bindings,
        diagnostics: Vec::new(),
        let_scopes: Vec::new(),
        trace: if wants_trace {
            Some(Trace::new())
        } else {
            None
        },
        call_arg_cache_stack: Vec::new(),
        eval_depth: 0,
        budget: options.budget,
        step_count: 0,
        alloc_bytes: 0,
        budget_breached: false,
    };
    let raw_value = evaluator.eval(expr);
    if let (Some(ev_trace), Some(caller)) = (evaluator.trace, caller_trace) {
        *caller = ev_trace;
    }
    // When the budget was breached during evaluation, null out any partial
    // result. Partial values alongside a "budget exceeded" diagnostic are
    // an attractive nuisance — hosts looking only at value would see a
    // plausible-but-incomplete number / array. The harness in
    // fuzz/fuzz_targets/fel_budget.rs encodes this contract; libFuzzer
    // surfaced inputs where the existing code leaked partial subexpression
    // results past the budget breach.
    let value = if evaluator.budget_breached {
        Value::Null
    } else {
        raw_value
    };
    EvalResult {
        value,
        diagnostics: evaluator.diagnostics,
    }
}

impl<'a> Evaluator<'a> {
    pub(super) fn diag(&mut self, msg: impl Into<String>) {
        self.diagnostics.push(Diagnostic::error(msg));
    }

    pub(super) fn diag_coded(&mut self, code: impl Into<String>, msg: impl Into<String>) {
        self.diagnostics.push(Diagnostic::error_coded(code, msg));
    }

    pub(in crate::evaluator) fn make_string(&mut self, s: String) -> Value {
        self.track_alloc(s.len() as u64);
        if self.alloc_limit_breached() {
            return Value::Null;
        }
        Value::String(s)
    }

    fn make_array(&mut self, elems: Vec<Value>) -> Value {
        self.track_alloc(
            (elems.len() * 16) as u64 + elems.iter().map(value_size_estimate).sum::<u64>(),
        );
        if self.alloc_limit_breached() {
            return Value::Null;
        }
        Value::Array(elems)
    }

    fn make_object(&mut self, entries: IndexMap<String, Value>) -> Value {
        self.track_alloc(
            (entries.len() * 40) as u64
                + entries
                    .iter()
                    .map(|(k, v)| k.len() as u64 + value_size_estimate(v))
                    .sum::<u64>(),
        );
        if self.alloc_limit_breached() {
            return Value::Null;
        }
        Value::Object(entries)
    }

    /// Bump step counter and check budget; when exceeded, emit diagnostic and return `true`.
    /// Suppresses duplicate budget diagnostics via [`Self::budget_breached`].
    #[inline]
    fn check_budget(&mut self) -> bool {
        self.step_count += 1;
        if self.budget_breached {
            return true;
        }
        match self.budget.check(self.step_count, self.alloc_bytes) {
            Ok(()) => false,
            Err(kind) => {
                let label = match kind {
                    BudgetExceededKind::Steps => "steps",
                    BudgetExceededKind::Alloc => "alloc",
                    BudgetExceededKind::Deadline => "deadline",
                };
                self.diag(format!("budget exceeded ({label})"));
                self.budget_breached = true;
                true
            }
        }
    }

    /// Track an estimated allocation (best-effort, not byte-accurate).
    /// Also checks the updated total against the budget, emitting a diagnostic if exceeded.
    /// Suppresses duplicate budget diagnostics via [`Self::budget_breached`].
    #[inline]
    fn track_alloc(&mut self, bytes: u64) {
        self.alloc_bytes = self.alloc_bytes.saturating_add(bytes);
        if self.alloc_limit_breached() && !self.budget_breached {
            self.diag("budget exceeded (alloc)");
            self.budget_breached = true;
        }
    }

    /// True after [`Self::track_alloc`] when estimated bytes exceed [`EvalBudget::max_alloc_bytes`].
    #[inline]
    fn alloc_limit_breached(&self) -> bool {
        self.alloc_bytes > self.budget.max_alloc_bytes
    }

    /// Evaluates `pred` with `$` bound to `elem` for one let-scope frame.
    pub(super) fn eval_under_dollar(&mut self, elem: &Value, pred: &Expr) -> Value {
        self.let_scopes
            .push(HashMap::from([("$".to_string(), elem.clone())]));
        let out = self.eval(pred);
        self.let_scopes.pop();
        out
    }

    /// Records a structured type mismatch diagnostic (no [`Value`] returned).
    pub(super) fn diag_expected_type(&mut self, fn_name: &str, expected: &str, got_type: &str) {
        self.diagnostics
            .push(Diagnostic::type_mismatch(fn_name, expected, got_type));
    }

    /// Records a type mismatch diagnostic and returns [`Value::Null`].
    pub(super) fn reject_expected_type(
        &mut self,
        fn_name: &str,
        expected: &str,
        got: &Value,
    ) -> Value {
        self.diag_expected_type(fn_name, expected, got.type_name());
        Value::Null
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

    fn eval_context_ref(
        &mut self,
        name: &str,
        called: bool,
        arg: Option<&str>,
        tail: &[String],
    ) -> Value {
        let path = context_tail_path(tail);
        self.eval_context_ref_path(name, called, arg, &path)
    }

    fn eval_context_ref_path(
        &mut self,
        name: &str,
        called: bool,
        arg: Option<&str>,
        path: &[PathSegment],
    ) -> Value {
        if is_grammar_reserved_context(name) || self.context_bindings.is_none() {
            let (env_tail, remaining_path) = split_context_env_path(path);
            let base = self.env.resolve_context(name, arg, &env_tail);
            return if remaining_path.is_empty() {
                base
            } else {
                self.access_path(base, remaining_path)
            };
        }

        let Some(catalog) = self.context_bindings else {
            let (env_tail, remaining_path) = split_context_env_path(path);
            let base = self.env.resolve_context(name, arg, &env_tail);
            return if remaining_path.is_empty() {
                base
            } else {
                self.access_path(base, remaining_path)
            };
        };
        let Some(kind) = catalog.binding_kind(name) else {
            self.diag_coded(
                UNBOUND_CONTEXT_REF_CODE,
                format!("unbound context reference: @{name}"),
            );
            return Value::Null;
        };

        match kind {
            ContextBindingKind::Value => {
                if called {
                    self.diag_coded(
                        "FEL-CONTEXT-BINDING-NOT-CALLABLE",
                        format!("context binding @{name} is not callable"),
                    );
                    return Value::Null;
                }
                if !path.is_empty() {
                    self.diag_coded(
                        "FEL-CONTEXT-BINDING-PATH",
                        format!("context binding @{name} does not support postfix traversal"),
                    );
                    return Value::Null;
                }
                self.resolve_context_binding_value(catalog, name, None, path)
            }
            ContextBindingKind::Object => {
                if called {
                    self.diag_coded(
                        "FEL-CONTEXT-BINDING-NOT-CALLABLE",
                        format!("context binding @{name} is not callable"),
                    );
                    return Value::Null;
                }
                self.resolve_context_binding_value(catalog, name, None, path)
            }
            ContextBindingKind::Function => {
                if !called {
                    self.diag_coded(
                        "FEL-CONTEXT-BINDING-CALL-REQUIRED",
                        format!("context binding @{name} must be called"),
                    );
                    return Value::Null;
                }
                self.resolve_context_binding_value(catalog, name, arg, path)
            }
        }
    }

    fn resolve_context_binding_value(
        &mut self,
        catalog: &dyn ContextBindingCatalog,
        name: &str,
        arg: Option<&str>,
        path: &[PathSegment],
    ) -> Value {
        let Some(value) = catalog.resolve(name, arg) else {
            self.diag_coded(
                UNBOUND_CONTEXT_REF_CODE,
                format!("unbound context reference: @{name}"),
            );
            return Value::Null;
        };
        self.access_path(value, path)
    }
}

impl<'a> Evaluator<'a> {
    pub(super) fn eval(&mut self, expr: &Expr) -> Value {
        if self.eval_depth >= MAX_EVAL_DEPTH {
            self.diag("expression evaluation depth exceeded");
            return Value::Null;
        }
        if self.check_budget() {
            return Value::Null;
        }
        self.eval_depth += 1;
        let out = self.eval_impl(expr);
        self.eval_depth -= 1;
        out
    }

    fn eval_impl(&mut self, expr: &Expr) -> Value {
        match expr {
            Expr::Null => Value::Null,
            Expr::Boolean(b) => Value::Boolean(*b),
            Expr::Number(n) => Value::Number(*n),
            Expr::String(s) => self.make_string(s.clone()),
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
            Expr::Array(elems) => {
                let values: Vec<Value> = elems.iter().map(|e| self.eval(e)).collect();
                self.make_array(values)
            }
            Expr::Object(entries) => {
                let pairs: IndexMap<String, Value> = entries
                    .iter()
                    .map(|(k, v)| (k.clone(), self.eval(v)))
                    .collect();
                self.make_object(pairs)
            }
            Expr::FieldRef { name, path } => self.eval_field_ref(name.as_deref(), path),
            Expr::VarRef { name, path } => self.eval_field_ref(Some(name), path),
            Expr::ContextRef {
                name,
                called,
                arg,
                tail,
            } => self.eval_context_ref(name, *called, arg.as_deref(), tail),
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
                    _ => self.reject_expected_type("if", "boolean", &cond),
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
                self.track_alloc(64);
                if self.alloc_limit_breached() {
                    return Value::Null;
                }
                self.let_scopes.push(HashMap::from([(name.clone(), val)]));
                let result = self.eval(body);
                self.let_scopes.pop();
                result
            }
            Expr::FunctionCall { name, args } => {
                if let Some(value) = self.env.precomputed(expr) {
                    return value;
                }
                // Eager traceable builtins rely on per-call arg caching (see `CallArgCache`);
                // only pure functions may remain on the eager trace list.
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
            Expr::PostfixAccess { .. } => self.eval_postfix(expr),
        }
    }

    fn eval_postfix(&mut self, mut base: &Expr) -> Value {
        let mut suffixes = Vec::new();
        while let Expr::PostfixAccess { expr, path } = base {
            if suffixes.len() + self.eval_depth >= MAX_EVAL_DEPTH {
                self.diag("expression evaluation depth exceeded");
                return Value::Null;
            }
            suffixes.push(path.as_slice());
            base = expr;
        }
        let tail = suffixes.into_iter().rev().flatten().cloned();
        match base {
            // A wildcard already inside the atom yields a value for the outer postfix.
            Expr::FieldRef { name, path } if !path.contains(&PathSegment::Wildcard) => {
                let path: Vec<_> = path.iter().cloned().chain(tail).collect();
                self.eval_field_ref(name.as_deref(), &path)
            }
            Expr::VarRef { name, path } if !path.contains(&PathSegment::Wildcard) => {
                let path: Vec<_> = path.iter().cloned().chain(tail).collect();
                self.eval_field_ref(Some(name), &path)
            }
            Expr::ContextRef {
                name,
                called,
                arg,
                tail: context_tail,
            } => {
                let path: Vec<_> = context_tail
                    .iter()
                    .cloned()
                    .map(PathSegment::Dot)
                    .chain(tail)
                    .collect();
                self.eval_context_ref_path(name, *called, arg.as_deref(), &path)
            }
            _ => {
                let value = self.eval(base);
                self.access_path(value, &tail.collect::<Vec<_>>())
            }
        }
    }

    fn eval_field_ref(&mut self, root: Option<&str>, path: &[PathSegment]) -> Value {
        let local = self
            .let_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(root.unwrap_or("$")));
        let value = if let Some(local) = local {
            self.access_path(local.clone(), path)
        } else {
            match self.env.resolve_field(root, path) {
                Ok(value) => {
                    // A projection materializes an array at the host boundary. Plain field
                    // lookups and a trailing wildcard only read an existing value.
                    if self.budget.max_alloc_bytes < u64::MAX
                        && matches!(value, Value::Array(_))
                        && path
                            .iter()
                            .rev()
                            .skip(1)
                            .any(|segment| matches!(segment, PathSegment::Wildcard))
                    {
                        self.track_alloc(value_size_estimate(&value));
                    }
                    if self.alloc_limit_breached() {
                        Value::Null
                    } else {
                        value
                    }
                }
                Err(error) => {
                    self.diag(error.to_string());
                    Value::Null
                }
            }
        };
        if self.tracing() {
            self.trace_step(TraceStep::FieldResolved {
                path: render_field_path(root, path),
                value: fel_to_json(&value),
            });
        }
        value
    }

    fn access_path(&mut self, current: Value, path: &[PathSegment]) -> Value {
        if path.is_empty() {
            return current;
        }
        access_value_path(&current, path, &mut |event| match event {
            PathAccess::Array(values) => self.make_array(values),
            PathAccess::Error(error) => {
                self.diag(error.to_string());
                Value::Null
            }
        })
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
                    let values: Vec<Value> =
                        arr.iter().map(|v| self.eval_unary(op, v.clone())).collect();
                    self.make_array(values)
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
                return {
                    let values: Vec<Value> = la
                        .iter()
                        .zip(ra.iter())
                        .map(|(l, r)| self.apply_binary(op, l, r))
                        .collect();
                    self.make_array(values)
                };
            }
            (Value::Array(la), _) => {
                return {
                    let values: Vec<Value> = la
                        .iter()
                        .map(|l| self.apply_binary(op, l, &right))
                        .collect();
                    self.make_array(values)
                };
            }
            (_, Value::Array(ra)) => {
                let values: Vec<Value> =
                    ra.iter().map(|r| self.apply_binary(op, &left, r)).collect();
                return self.make_array(values);
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
            BinaryOp::Add => self.num_op(left, right, "+"),
            BinaryOp::Sub => self.num_op(left, right, "-"),
            BinaryOp::Mul => self.num_op(left, right, "*"),
            BinaryOp::Div => {
                if let (Value::Number(a), Value::Number(b)) = (left, right) {
                    if b.is_zero() {
                        self.diag("division by zero");
                        Value::Null
                    } else {
                        match a.checked_div(*b) {
                            Some(q) => Value::Number(q),
                            None => {
                                self.diag("division overflow");
                                Value::Null
                            }
                        }
                    }
                } else if let (Value::Money(m), Value::Number(n)) = (left, right) {
                    if n.is_zero() {
                        self.diag("division by zero");
                        Value::Null
                    } else {
                        match m.amount.checked_div(*n) {
                            Some(amount) => Value::Money(Money {
                                amount,
                                currency: m.currency,
                            }),
                            None => {
                                self.diag("division overflow");
                                Value::Null
                            }
                        }
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
                        match a.amount.checked_div(b.amount) {
                            Some(q) => Value::Number(q),
                            None => {
                                self.diag("division overflow");
                                Value::Null
                            }
                        }
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
                        match a.checked_rem(*b) {
                            Some(r) => Value::Number(r),
                            None => {
                                self.diag("modulo overflow");
                                Value::Null
                            }
                        }
                    }
                } else if let (Value::Money(m), Value::Number(n)) = (left, right) {
                    if n.is_zero() {
                        self.diag("modulo by zero");
                        Value::Null
                    } else {
                        match m.amount.checked_rem(*n) {
                            Some(amount) => Value::Money(Money {
                                amount,
                                currency: m.currency,
                            }),
                            None => {
                                self.diag("modulo overflow");
                                Value::Null
                            }
                        }
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
                    self.make_string(format!("{a}{b}"))
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

    fn num_op(&mut self, left: &Value, right: &Value, sym: &str) -> Value {
        let combine = |a: Decimal, b: Decimal| -> Option<Decimal> {
            match sym {
                "+" => a.checked_add(b),
                "-" => a.checked_sub(b),
                "*" => a.checked_mul(b),
                _ => None,
            }
        };

        match (left, right) {
            (Value::Number(a), Value::Number(b)) => match combine(*a, *b) {
                Some(v) => Value::Number(v),
                None => {
                    self.diag(format!("arithmetic overflow ({sym})"));
                    Value::Null
                }
            },
            (Value::Money(a), Value::Money(b)) if sym == "+" || sym == "-" => {
                if a.currency != b.currency {
                    self.diag(format!(
                        "currency mismatch: {} vs {}",
                        a.currency, b.currency
                    ));
                    Value::Null
                } else {
                    match combine(a.amount, b.amount) {
                        Some(amount) => Value::Money(Money {
                            amount,
                            currency: a.currency,
                        }),
                        None => {
                            self.diag(format!("arithmetic overflow ({sym})"));
                            Value::Null
                        }
                    }
                }
            }
            (Value::Money(m), Value::Number(n)) if sym == "+" || sym == "-" => {
                match combine(m.amount, *n) {
                    Some(amount) => Value::Money(Money {
                        amount,
                        currency: m.currency,
                    }),
                    None => {
                        self.diag(format!("arithmetic overflow ({sym})"));
                        Value::Null
                    }
                }
            }
            (Value::Money(m), Value::Number(n)) if sym == "*" => match m.amount.checked_mul(*n) {
                Some(amount) => Value::Money(Money {
                    amount,
                    currency: m.currency,
                }),
                None => {
                    self.diag("arithmetic overflow (*)");
                    Value::Null
                }
            },
            (Value::Number(n), Value::Money(m)) if sym == "*" => match n.checked_mul(m.amount) {
                Some(amount) => Value::Money(Money {
                    amount,
                    currency: m.currency,
                }),
                None => {
                    self.diag("arithmetic overflow (*)");
                    Value::Null
                }
            },
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
        // Uniform catalog-driven arity gate (FUT-7). Every built-in's signature
        // — `(min, max)` derived from `BuiltinFunctionCatalogEntry::arity` — is
        // enforced before dispatch so handlers cannot silently accept too-few
        // args (defaulting missing positions to null) or too-many args (ignoring
        // tails). Unknown names skip this gate and fall through to the
        // extension registry / undefined-function diagnostic at the bottom.
        if let Some((min, max)) = crate::extensions::builtin_arity(name) {
            let got = args.len();
            let out_of_bounds = got < min || max.is_some_and(|m| got > m);
            if out_of_bounds {
                self.diagnostics
                    .push(Diagnostic::arity_mismatch(name, min, max, got));
                return Value::Null;
            }
        }
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
            "upper" => self.fn_str1(args, "upper", |s| s.to_uppercase()),
            "lower" => self.fn_str1(args, "lower", |s| s.to_lowercase()),
            "trim" => self.fn_str1(args, "trim", |s| s.trim().to_string()),
            "matches" => self.fn_matches(args),
            "format" => self.fn_format(args),

            // Numeric
            "round" => self.fn_round(args),
            "floor" => self.fn_num1(args, "floor", |n| n.floor()),
            "ceil" => self.fn_num1(args, "ceil", |n| n.ceil()),
            "abs" => self.fn_num1(args, "abs", |n| n.abs()),
            "power" => self.fn_power(args),

            // Date
            "today" => self.fn_today(),
            "now" => self.fn_now(),
            "year" => self.fn_date_part(args, "year", |d| dec(d.year() as i64)),
            "month" => self.fn_date_part(args, "month", |d| dec(d.month() as i64)),
            "day" => self.fn_date_part(args, "day", |d| dec(d.day() as i64)),
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
            "present" => self.fn_present(args),
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
                self.make_string(v.type_name().to_string())
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
                    Value::Null => Value::Null,
                    other => self.reject_expected_type("moneyAmount", "money", &other),
                }
            }
            "moneyCurrency" => {
                let v = self.eval_arg(args, 0);
                match v {
                    Value::Money(m) => self.make_string(m.currency.to_string()),
                    Value::Null => Value::Null,
                    other => self.reject_expected_type("moneyCurrency", "money", &other),
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
            "formatNumber" => self.fn_format_number(args),
            "formatDate" => self.fn_format_date(args),
            "runtimeMeta" => self.fn_runtime_meta(args),
            "subject" => self.fn_subject(args),
            "pluralCategory" => self.fn_plural_category(args),

            _ => {
                let evaluated_args: Vec<Value> = args.iter().map(|arg| self.eval(arg)).collect();
                if let Some(extensions) = self.extensions {
                    match call_extension(extensions, name, &evaluated_args) {
                        ExtensionCallOutcome::Ok(result) => {
                            let result = match result {
                                Value::String(s) => self.make_string(s),
                                Value::Array(arr) => self.make_array(arr),
                                Value::Object(obj) => self.make_object(obj),
                                other => {
                                    self.track_alloc(value_size_estimate(&other));
                                    if self.alloc_limit_breached() {
                                        return Value::Null;
                                    }
                                    other
                                }
                            };
                            if self.tracing() {
                                self.trace_step(TraceStep::FunctionCalled {
                                    name: name.to_string(),
                                    args: evaluated_args.iter().map(fel_to_json).collect(),
                                    result: fel_to_json(&result),
                                });
                            }
                            return result;
                        }
                        ExtensionCallOutcome::ArityMismatch {
                            name,
                            min_args,
                            max_args,
                            got,
                        } => {
                            self.diagnostics
                                .push(Diagnostic::arity_mismatch(name, min_args, max_args, got));
                            return Value::Null;
                        }
                        ExtensionCallOutcome::Failed { name, message } => {
                            self.diagnostics
                                .push(Diagnostic::extension_failed(name, message));
                            return Value::Null;
                        }
                        ExtensionCallOutcome::NotFound => {}
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
                self.diag_expected_type(fn_name, "array", val.type_name());
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
        let Some(mut cache) = self.call_arg_cache_stack.pop() else {
            self.diag("internal: missing call arg cache frame");
            return args.iter().map(|e| fel_to_json(&self.eval(e))).collect();
        };
        if cache.args_ptr != args.as_ptr() {
            self.diag("internal: call arg cache pointer mismatch");
            return args.iter().map(|e| fel_to_json(&self.eval(e))).collect();
        }
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
    ///
    /// Arity is enforced upstream by the uniform pre-dispatch gate in `eval_function`;
    /// callers are guaranteed `args.len() >= 2` and may index `args[0]`/`args[1]` directly.
    pub(super) fn filter_where(&mut self, args: &[Expr], fn_name: &str) -> Option<Vec<Value>> {
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

/// Read a resolved value with 1-based indices and wildcard projection.
///
/// Borrows the input and copies only the returned value or projected elements.
///
/// # Errors
///
/// Returns the first invalid access in the path. Missing values yield null.
pub fn resolve_value_segments(value: &Value, path: &[PathSegment]) -> Result<Value, FieldError> {
    let mut first_error = None;
    let value = access_value_path(value, path, &mut |event| match event {
        PathAccess::Array(values) => Value::Array(values),
        PathAccess::Error(error) => {
            first_error.get_or_insert(error);
            Value::Null
        }
    });
    first_error.map_or(Ok(value), Err)
}

enum PathAccess {
    Array(Vec<Value>),
    Error(FieldError),
}

fn access_value_path(
    mut current: &Value,
    path: &[PathSegment],
    handle: &mut impl FnMut(PathAccess) -> Value,
) -> Value {
    for (i, seg) in path.iter().enumerate() {
        match seg {
            PathSegment::Dot(name) => match current {
                Value::Object(entries) => {
                    current = entries.get(name.as_str()).unwrap_or(&Value::Null);
                }
                Value::Null => return Value::Null,
                _ => {
                    return handle(PathAccess::Error(FieldError::new(format!(
                        "cannot access '{name}' on {}",
                        current.type_name()
                    ))));
                }
            },
            PathSegment::Index(idx) => match current {
                Value::Array(arr) => {
                    // FEL indices are 1-based; guard above rejects 0 and OOB before subtracting.
                    if *idx == 0 || *idx > arr.len() {
                        return handle(PathAccess::Error(FieldError::new(format!(
                            "index {idx} out of bounds (len {})",
                            arr.len()
                        ))));
                    }
                    current = &arr[*idx - 1];
                }
                Value::Null => return Value::Null,
                _ => {
                    return handle(PathAccess::Error(FieldError::new(format!(
                        "cannot index into {}",
                        current.type_name()
                    ))));
                }
            },
            PathSegment::Wildcard => match current {
                Value::Array(arr) => {
                    let remaining = &path[i + 1..];
                    if remaining.is_empty() {
                        break;
                    }
                    let elems: Vec<Value> = arr
                        .iter()
                        .map(|e| access_value_path(e, remaining, handle))
                        .collect();
                    return handle(PathAccess::Array(elems));
                }
                Value::Null => return Value::Null,
                _ => {
                    return handle(PathAccess::Error(FieldError::new(format!(
                        "cannot wildcard on {}",
                        current.type_name()
                    ))));
                }
            },
        }
    }
    current.clone()
}
