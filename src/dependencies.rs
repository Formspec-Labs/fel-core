//! Static dependency extraction — field refs, context refs, and MIP dependencies.
//!
//! Walks the AST without evaluation to find field references,
//! context references, MIP dependencies, and structural flags.
//!
//! The `walk` helper and related functions recurse the AST to populate [`Dependencies`].
#![allow(clippy::missing_docs_in_private_items)]
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::{self, Write as _};
use std::rc::Rc;

use serde::{Serialize, Serializer};

use crate::ast::{Expr, PathDotJoin, PathSegment};

/// One typed step in a statically resolved dependency read path.
///
/// Keeps literal object keys (such as `instance('source', 'rows[1].code')`, which reads the
/// literal property key `"rows[1]"` followed by `"code"`) distinct from 1-based FEL array
/// subscripts (`@instance('source').rows[1].code`) and wildcard element projections (`[*]`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReadPathSegment {
    /// A literal object property key (never re-parsed as bracket syntax).
    Key(String),
    /// A 1-based FEL array subscript (`[1]`, `[2]`, ...).
    Index(usize),
    /// A wildcard element projection (`[*]`).
    Wildcard,
}

/// A sequence of typed read path segments.
///
/// An empty segment list represents the whole root value (for example, a whole instance or
/// an instance read whose path depends on runtime data).
///
/// Canonical serialization ([`ReadPath::to_canonical_string`]) is strictly injective and
/// round-trips via [`ReadPath::parse_canonical`]:
/// - Empty path → `""`
/// - Bare [`ReadPathSegment::Key`] matching `[A-Za-z0-9_-]+` → `key` (or `.key` after a segment)
/// - Any other [`ReadPathSegment::Key`] → `["<json-escaped-key>"]` (no leading `.`)
/// - [`ReadPathSegment::Index`] → `[N]` (no leading `.`)
/// - [`ReadPathSegment::Wildcard`] → `[*]` (no leading `.`)
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ReadPath {
    segments: Vec<ReadPathSegment>,
}

impl ReadPath {
    /// Construct a typed read path from segments.
    pub fn new(segments: Vec<ReadPathSegment>) -> Self {
        Self { segments }
    }

    /// Parse a two-argument `instance(name, path)` literal path string into typed literal keys.
    ///
    /// Matches FEL's runtime `instance()` evaluator: an empty string reads the whole instance,
    /// and a non-empty string splits strictly on `.` where each component is a literal property
    /// key (brackets such as `rows[1]` or `rows[*]` are never interpreted as array subscripts).
    pub fn from_dotted_literal_keys(path: &str) -> Self {
        if path.is_empty() {
            Self::default()
        } else {
            Self {
                segments: path
                    .split('.')
                    .map(|segment| ReadPathSegment::Key(segment.to_owned()))
                    .collect(),
            }
        }
    }

    /// Borrow the typed segments in this read path.
    pub fn segments(&self) -> &[ReadPathSegment] {
        &self.segments
    }

    /// Consume this read path into its typed segments.
    pub fn into_segments(self) -> Vec<ReadPathSegment> {
        self.segments
    }

    /// Whether this path represents the whole root value (no child segments).
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Append a typed segment to this read path.
    pub fn push(&mut self, segment: ReadPathSegment) {
        self.segments.push(segment);
    }

    /// Append all segments from an iterator to this read path.
    pub fn extend_segments(&mut self, segments: impl IntoIterator<Item = ReadPathSegment>) {
        self.segments.extend(segments);
    }

    /// Serialize to the unambiguous, injective canonical path string.
    pub fn to_canonical_string(&self) -> String {
        let mut out = String::new();
        for (idx, segment) in self.segments.iter().enumerate() {
            match segment {
                ReadPathSegment::Key(key) if is_bare_read_key(key) => {
                    if idx > 0 {
                        out.push('.');
                    }
                    out.push_str(key);
                }
                ReadPathSegment::Key(key) => {
                    out.push('[');
                    let quoted =
                        serde_json::to_string(key).expect("string key serializes to JSON");
                    out.push_str(&quoted);
                    out.push(']');
                }
                ReadPathSegment::Index(i) => {
                    let _ = write!(out, "[{i}]");
                }
                ReadPathSegment::Wildcard => out.push_str("[*]"),
            }
        }
        out
    }

    /// Parse a canonical read path string back into typed segments.
    ///
    /// # Errors
    ///
    /// Returns an error description if `input` is not a valid canonical encoding produced by
    /// [`ReadPath::to_canonical_string`].
    pub fn parse_canonical(input: &str) -> Result<Self, String> {
        if input.is_empty() {
            return Ok(Self::default());
        }
        let bytes = input.as_bytes();
        let mut pos = 0;
        let mut segments = Vec::new();
        while pos < bytes.len() {
            match bytes[pos] {
                b'.' => {
                    if segments.is_empty() {
                        return Err("unexpected leading '.' in canonical read path".to_owned());
                    }
                    pos += 1;
                    let key = parse_bare_key(input, &mut pos)?;
                    segments.push(ReadPathSegment::Key(key));
                }
                b'[' => {
                    segments.push(parse_bracket_segment(input, &mut pos)?);
                }
                _ if segments.is_empty() => {
                    let key = parse_bare_key(input, &mut pos)?;
                    segments.push(ReadPathSegment::Key(key));
                }
                ch => {
                    return Err(format!(
                        "unexpected character {:?} at byte {pos} in canonical read path",
                        ch as char
                    ));
                }
            }
        }
        Ok(Self { segments })
    }
}

impl From<Vec<ReadPathSegment>> for ReadPath {
    fn from(segments: Vec<ReadPathSegment>) -> Self {
        Self { segments }
    }
}

impl fmt::Display for ReadPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_canonical_string())
    }
}

impl Ord for ReadPath {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_canonical_string().cmp(&other.to_canonical_string())
    }
}

impl PartialOrd for ReadPath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Serialize for ReadPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_canonical_string())
    }
}

fn is_bare_read_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn parse_bare_key(input: &str, pos: &mut usize) -> Result<String, String> {
    let start = *pos;
    let bytes = input.as_bytes();
    while *pos < bytes.len() {
        let b = bytes[*pos];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
            *pos += 1;
        } else {
            break;
        }
    }
    if *pos == start {
        return Err(format!(
            "expected key segment at byte {start} in canonical read path"
        ));
    }
    Ok(input[start..*pos].to_owned())
}

fn parse_bracket_segment(input: &str, pos: &mut usize) -> Result<ReadPathSegment, String> {
    let bytes = input.as_bytes();
    debug_assert_eq!(bytes[*pos], b'[');
    *pos += 1;
    if *pos >= bytes.len() {
        return Err("unterminated '[' in canonical read path".to_owned());
    }
    match bytes[*pos] {
        b'"' => {
            let str_start = *pos;
            *pos += 1;
            let mut escaped = false;
            while *pos < bytes.len() {
                let b = bytes[*pos];
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    *pos += 1;
                    let key: String = serde_json::from_str(&input[str_start..*pos])
                        .map_err(|e| format!("invalid JSON key in canonical read path: {e}"))?;
                    if *pos >= bytes.len() || bytes[*pos] != b']' {
                        return Err("expected ']' after quoted key in canonical read path".to_owned());
                    }
                    *pos += 1;
                    return Ok(ReadPathSegment::Key(key));
                }
                *pos += 1;
            }
            Err("unterminated quoted key in canonical read path".to_owned())
        }
        b'*' => {
            *pos += 1;
            if *pos >= bytes.len() || bytes[*pos] != b']' {
                return Err("expected ']' after '*' in canonical read path".to_owned());
            }
            *pos += 1;
            Ok(ReadPathSegment::Wildcard)
        }
        b'0'..=b'9' => {
            let start = *pos;
            while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
                *pos += 1;
            }
            let digits = &input[start..*pos];
            if digits.len() > 1 && digits.starts_with('0') {
                return Err(format!("non-canonical leading zero index [{digits}]"));
            }
            if *pos >= bytes.len() || bytes[*pos] != b']' {
                return Err("expected ']' after numeric index in canonical read path".to_owned());
            }
            *pos += 1;
            let index = digits
                .parse::<usize>()
                .map_err(|e| format!("invalid index [{digits}]: {e}"))?;
            Ok(ReadPathSegment::Index(index))
        }
        ch => Err(format!(
            "unexpected bracket content starting with {:?} in canonical read path",
            ch as char
        )),
    }
}

/// Dependencies extracted from a FEL expression.
#[derive(Debug, Clone, Default)]
pub struct Dependencies {
    /// Field paths referenced (e.g., `["firstName", "address.city"]`).
    pub fields: HashSet<String>,
    /// Context references (e.g., `["@current", "@index"]`).
    pub context_refs: HashSet<String>,
    /// Typed paths read per statically named instance. An empty [`ReadPath`] covers the whole
    /// instance, including a read whose path depends on runtime data. Computed names are unresolved.
    pub instance_reads: BTreeMap<String, BTreeSet<ReadPath>>,
    /// Function calls in traversal order, including their argument counts.
    pub function_calls: Vec<(String, usize)>,
    /// An instance lookup computes its source name at evaluation time.
    pub has_dynamic_instance: bool,
    /// Typed paths relative to the current field, excluding dollars rebound by predicates.
    pub self_reads: BTreeSet<ReadPath>,
    /// Typed paths relative to `@current` (the enclosing repeat row), retaining postfix
    /// subscripts, wildcards, aliases, and row-predicate column reads.
    pub current_reads: BTreeSet<ReadPath>,
    /// MIP dependencies: fields used in valid/relevant/readonly/required.
    pub mip_deps: HashSet<String>,
    /// Whether bare `$` (self-reference) appears.
    pub has_self_ref: bool,
    /// Whether any `[*]` wildcard appears.
    pub has_wildcard: bool,
    /// Whether prev() or next() is called.
    pub uses_prev_next: bool,
}

impl Dependencies {
    /// Iterate over the functions called in this expression.
    pub fn functions(&self) -> impl Iterator<Item = &str> {
        self.function_calls.iter().map(|(name, _)| name.as_str())
    }

    /// Iterate over context names without their call arguments or property paths.
    pub fn context_names(&self) -> impl Iterator<Item = &str> {
        self.context_refs
            .iter()
            .filter_map(|reference| reference.strip_prefix('@')?.split(['.', '(', '[']).next())
    }
}

/// Extract dependencies from an AST expression.
pub fn extract_dependencies(expr: &Expr) -> Dependencies {
    let mut deps = Dependencies::default();
    visit(expr, &mut deps, &mut Vec::new());
    deps
}

#[derive(Clone, PartialEq)]
enum Root {
    Field,
    Instance(String),
    CurrentField,
    CurrentRow,
}

// Describe where a value came from only when syntax determines the answer.
// Constructors retain their actual member names and indices. A projected array
// has one set of possible element origins (e.g. rows[*].child).
#[derive(Clone, PartialEq)]
enum Origin {
    Path(Root, ReadPath),
    Object(Rc<BTreeMap<String, Origins>>),
    Array(Rc<Vec<Origins>>),
    Projection(Rc<Origins>),
}

type Origins = Vec<Origin>;
type Bindings = Vec<(String, Origins)>;

fn record(origins: &Origins, deps: &mut Dependencies) {
    for origin in origins {
        match origin {
            Origin::Path(Root::Instance(name), path) => {
                deps.instance_reads
                    .entry(name.clone())
                    .or_default()
                    .insert(path.clone());
            }
            Origin::Path(Root::Field, path) => {
                deps.fields.insert(path.to_canonical_string());
            }
            Origin::Path(Root::CurrentField, path) => {
                deps.self_reads.insert(path.clone());
            }
            Origin::Path(Root::CurrentRow, path) => {
                deps.current_reads.insert(path.clone());
            }
            Origin::Projection(rows) => record(rows, deps),
            // Each constructor member was visited when the value was built.
            Origin::Array(_) | Origin::Object(_) => {}
        }
    }
}

fn union(mut left: Origins, right: Origins) -> Origins {
    for origin in right {
        if !left.contains(&origin) {
            left.push(origin);
        }
    }
    left
}

fn elements(origins: Origins) -> Origins {
    origins.into_iter().fold(Vec::new(), |rows, origin| {
        let values = match origin {
            Origin::Path(root, mut path) => {
                path.push(ReadPathSegment::Wildcard);
                vec![Origin::Path(root, path)]
            }
            Origin::Array(values) => values.iter().cloned().fold(Vec::new(), union),
            Origin::Projection(values) => (*values).clone(),
            Origin::Object(_) => Vec::new(),
        };
        union(rows, values)
    })
}

fn follow(mut origins: Origins, path: &[PathSegment]) -> Origins {
    for (index, segment) in path.iter().enumerate() {
        if origins.is_empty() {
            break;
        }
        if *segment == PathSegment::Wildcard {
            let rows = follow(elements(origins), &path[index + 1..]);
            return vec![Origin::Projection(Rc::new(rows))];
        }
        origins = origins.into_iter().fold(Vec::new(), |result, origin| {
            let values = match (origin, segment) {
                (Origin::Path(root, mut base), PathSegment::Dot(key)) => {
                    base.push(ReadPathSegment::Key(key.clone()));
                    vec![Origin::Path(root, base)]
                }
                (Origin::Path(root, mut base), PathSegment::Index(i)) => {
                    base.push(ReadPathSegment::Index(*i));
                    vec![Origin::Path(root, base)]
                }
                (Origin::Path(root, mut base), PathSegment::Wildcard) => {
                    base.push(ReadPathSegment::Wildcard);
                    vec![Origin::Path(root, base)]
                }
                (Origin::Object(entries), PathSegment::Dot(key)) => {
                    entries.get(key).cloned().unwrap_or_default()
                }
                (Origin::Array(rows), PathSegment::Index(index)) => index
                    .checked_sub(1)
                    .and_then(|i| rows.get(i))
                    .cloned()
                    .unwrap_or_default(),
                (Origin::Projection(rows), PathSegment::Index(_)) => (*rows).clone(),
                _ => Vec::new(),
            };
            union(result, values)
        });
    }
    origins
}

fn visit(expr: &Expr, deps: &mut Dependencies, bindings: &mut Bindings) -> Origins {
    let paths = walk(expr, deps, bindings);
    // A lookup already records its source collection and key column.
    let bare_dollar = matches!(expr, Expr::FieldRef { name: None, path } if path.is_empty())
        && bindings.iter().any(|(name, _)| name == "$");
    if !bare_dollar && !matches!(expr, Expr::RowLookup { .. }) {
        record(&paths, deps);
    }
    paths
}

fn walk(expr: &Expr, deps: &mut Dependencies, bindings: &mut Bindings) -> Origins {
    match expr {
        Expr::FieldRef { name, path } => {
            return reference(name.as_deref(), path, deps, bindings);
        }
        Expr::VarRef { name, path } => {
            return reference(Some(name), path, deps, bindings);
        }
        Expr::ContextRef {
            name,
            called,
            arg,
            tail,
        } => {
            let mut context = format!("@{name}");
            if *called {
                context.push_str(
                    &arg.as_ref()
                        .map_or_else(|| "()".into(), |a| format!("('{a}')")),
                );
            }
            for segment in tail {
                context.push('.');
                context.push_str(segment);
            }
            deps.context_refs.insert(context);
            if name == "instance"
                && let Some(instance) = arg
            {
                let path = ReadPath::new(
                    tail.iter()
                        .cloned()
                        .map(ReadPathSegment::Key)
                        .collect(),
                );
                return vec![Origin::Path(Root::Instance(instance.clone()), path)];
            }
            if name == "current" && !*called && arg.is_none() {
                let path = ReadPath::new(
                    tail.iter()
                        .cloned()
                        .map(ReadPathSegment::Key)
                        .collect(),
                );
                return vec![Origin::Path(Root::CurrentRow, path)];
            }
        }
        Expr::FunctionCall { name, args } => {
            deps.function_calls.push((name.clone(), args.len()));
            match name.as_str() {
                "valid" | "relevant" | "readonly" | "required" => {
                    if let Some(first) = args.first() {
                        let path = extract_field_path_str(first);
                        if !path.is_empty() {
                            deps.mip_deps.insert(path);
                        }
                    }
                }
                "coalesce" => {
                    return args
                        .iter()
                        .map(|arg| visit(arg, deps, bindings))
                        .fold(Vec::new(), union);
                }
                "if" if args.len() == 3 => {
                    visit(&args[0], deps, bindings);
                    return union(
                        visit(&args[1], deps, bindings),
                        visit(&args[2], deps, bindings),
                    );
                }
                "prev" | "next" | "parent" => deps.uses_prev_next = true,
                "countWhere" | "every" | "some" | "sumWhere" | "avgWhere" | "minWhere"
                | "maxWhere" | "moneySumWhere"
                    if args.len() == 2 =>
                {
                    let rows = elements(visit(&args[0], deps, bindings));
                    bindings.push(("$".into(), rows));
                    visit(&args[1], deps, bindings);
                    bindings.pop();
                    return Vec::new();
                }
                _ => {}
            }
            for arg in args {
                visit(arg, deps, bindings);
            }
            if name == "instance"
                && args
                    .first()
                    .is_some_and(|arg| !matches!(arg, Expr::String(_)))
            {
                deps.has_dynamic_instance = true;
            }
            if name == "instance"
                && let Some(Expr::String(instance)) = args.first()
            {
                let path = match args.get(1) {
                    None => ReadPath::default(),
                    Some(Expr::String(path)) => ReadPath::from_dotted_literal_keys(path),
                    // The instance is known but its path depends on runtime data.
                    Some(_) => {
                        deps.instance_reads
                            .entry(instance.clone())
                            .or_default()
                            .insert(ReadPath::default());
                        return Vec::new();
                    }
                };
                return vec![Origin::Path(Root::Instance(instance.clone()), path)];
            }
        }
        Expr::LetBinding { name, value, body } => {
            let value_paths = visit(value, deps, bindings);
            bindings.push((name.clone(), value_paths));
            let result = walk(body, deps, bindings);
            bindings.pop();
            return result;
        }
        Expr::BinaryOp { left, right, .. } => {
            visit(left, deps, bindings);
            visit(right, deps, bindings);
        }
        Expr::UnaryOp { operand, .. } => {
            visit(operand, deps, bindings);
        }
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
            visit(condition, deps, bindings);
            return union(
                visit(then_branch, deps, bindings),
                visit(else_branch, deps, bindings),
            );
        }
        Expr::NullCoalesce { left, right } => {
            return union(visit(left, deps, bindings), visit(right, deps, bindings));
        }
        Expr::Membership {
            value, container, ..
        } => {
            visit(value, deps, bindings);
            visit(container, deps, bindings);
        }
        Expr::RowLookup {
            source,
            key_path,
            key,
        } => {
            let source_origins = visit(source, deps, bindings);
            visit(key, deps, bindings);
            let rows = field_read_path_from_root(expr, |_, name| {
                !bindings.iter().any(|(bound, _)| bound == name)
            })
            .map_or_else(
                || elements(source_origins),
                |path| vec![Origin::Path(Root::Field, path)],
            );
            let columns = key_path
                .iter()
                .cloned()
                .map(PathSegment::Dot)
                .collect::<Vec<_>>();
            record(&follow(rows.clone(), &columns), deps);
            deps.has_wildcard = true;
            return rows;
        }
        Expr::PostfixAccess { .. } => {
            // As in evaluation, adjacent postfix nodes form a single access path.
            let mut base = expr;
            let mut suffixes = Vec::new();
            while let Expr::PostfixAccess { expr, path } = base {
                suffixes.push(path.as_slice());
                base = expr;
            }
            let path: Vec<_> = suffixes.into_iter().rev().flatten().cloned().collect();
            deps.has_wildcard |= path.contains(&PathSegment::Wildcard);
            let origins = walk(base, deps, bindings);
            let reads = follow(origins.clone(), &path);
            if reads.is_empty() {
                record(&origins, deps);
            }
            return reads;
        }
        Expr::Array(elems) => {
            return vec![Origin::Array(Rc::new(
                elems
                    .iter()
                    .map(|value| visit(value, deps, bindings))
                    .collect(),
            ))];
        }
        Expr::Object(entries) => {
            return vec![Origin::Object(Rc::new(
                entries
                    .iter()
                    .map(|(key, value)| (key.clone(), visit(value, deps, bindings)))
                    .collect(),
            ))];
        }
        Expr::Null
        | Expr::Boolean(_)
        | Expr::Number(_)
        | Expr::String(_)
        | Expr::DateLiteral(_)
        | Expr::DateTimeLiteral(_) => {}
    }
    Vec::new()
}

fn reference(
    name: Option<&str>,
    path: &[PathSegment],
    deps: &mut Dependencies,
    bindings: &Bindings,
) -> Origins {
    deps.has_wildcard |= path.contains(&PathSegment::Wildcard);
    let roots = if let Some((_, paths)) = bindings
        .iter()
        .rev()
        .find(|(key, _)| key == name.unwrap_or("$"))
    {
        paths.clone()
    } else if let Some(name) = name {
        vec![Origin::Path(
            Root::Field,
            ReadPath::new(vec![ReadPathSegment::Key(name.to_owned())]),
        )]
    } else {
        deps.has_self_ref = true;
        vec![Origin::Path(Root::CurrentField, ReadPath::default())]
    };
    follow(roots, path)
}

fn extract_field_path_str(expr: &Expr) -> String {
    match expr {
        Expr::FieldRef { name, path } => {
            let mut result = name.as_deref().unwrap_or("").to_string();
            for seg in path {
                seg.append_to_path(&mut result, PathDotJoin::PrefixWhenNonempty);
            }
            result
        }
        Expr::VarRef { name, path } => {
            let mut result = name.clone();
            for seg in path {
                seg.append_to_path(&mut result, PathDotJoin::PrefixWhenNonempty);
            }
            result
        }
        _ => String::new(),
    }
}

/// Return a path only when the source names a field with `$` and no enclosing `let` binds it.
pub(crate) fn explicit_field_path(expr: &Expr, let_vars: &[String]) -> Option<String> {
    field_path_from_root(expr, |root, name| {
        matches!(root, Expr::FieldRef { .. }) && !let_vars.iter().any(|bound| bound == name)
    })
}

fn push_ast_segments(read_path: &mut ReadPath, segments: &[PathSegment]) {
    for segment in segments {
        match segment {
            PathSegment::Dot(key) => read_path.push(ReadPathSegment::Key(key.clone())),
            PathSegment::Index(i) => read_path.push(ReadPathSegment::Index(*i)),
            PathSegment::Wildcard => read_path.push(ReadPathSegment::Wildcard),
        }
    }
}

/// Postfix chains are parsed iteratively. Walk back to their root without recursion.
fn field_read_path_from_root(
    mut expr: &Expr,
    accept: impl Fn(&Expr, &str) -> bool,
) -> Option<ReadPath> {
    let mut suffixes = Vec::new();
    let mut read_path = loop {
        match expr {
            Expr::PostfixAccess { expr: base, path } => {
                suffixes.push(path.as_slice());
                expr = base;
            }
            Expr::RowLookup { source, .. } => {
                suffixes.push(std::slice::from_ref(&PathSegment::Wildcard));
                expr = source;
            }
            Expr::FieldRef {
                name: Some(name),
                path,
            }
            | Expr::VarRef { name, path }
                if accept(expr, name) =>
            {
                let mut base = ReadPath::new(vec![ReadPathSegment::Key(name.clone())]);
                push_ast_segments(&mut base, path);
                break base;
            }
            _ => return None,
        }
    };
    for suffix in suffixes.into_iter().rev() {
        push_ast_segments(&mut read_path, suffix);
    }
    Some(read_path)
}

/// Postfix chains are parsed iteratively. Walk back to their root without recursion.
fn field_path_from_root(expr: &Expr, accept: impl Fn(&Expr, &str) -> bool) -> Option<String> {
    field_read_path_from_root(expr, accept).map(|path| path.to_canonical_string())
}

use crate::wire_style::JsonWireStyle;

/// Serialize [`Dependencies`] for WASM / JSON FFI (camelCase keys).
pub fn dependencies_to_json_value(deps: &Dependencies) -> serde_json::Value {
    dependencies_to_json_value_styled(deps, JsonWireStyle::JsCamel)
}

/// Serialize [`Dependencies`] with explicit host key style.
pub fn dependencies_to_json_value_styled(
    deps: &Dependencies,
    style: JsonWireStyle,
) -> serde_json::Value {
    match style {
        JsonWireStyle::JsCamel => serde_json::json!({
            "fields": deps.fields.iter().collect::<Vec<_>>(),
            "contextRefs": deps.context_refs.iter().collect::<Vec<_>>(),
            "instanceReads": deps.instance_reads,
            "selfReads": deps.self_reads,
            "mipDeps": deps.mip_deps.iter().collect::<Vec<_>>(),
            "hasSelfRef": deps.has_self_ref,
            "hasWildcard": deps.has_wildcard,
            "usesPrevNext": deps.uses_prev_next,
        }),
        JsonWireStyle::PythonSnake => serde_json::json!({
            "fields": deps.fields.iter().collect::<Vec<_>>(),
            "context_refs": deps.context_refs.iter().collect::<Vec<_>>(),
            "instance_reads": deps.instance_reads,
            "self_reads": deps.self_reads,
            "mip_deps": deps.mip_deps.iter().collect::<Vec<_>>(),
            "has_self_ref": deps.has_self_ref,
            "has_wildcard": deps.has_wildcard,
            "uses_prev_next": deps.uses_prev_next,
        }),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_docs_in_private_items)]
    use super::*;
    use crate::parser;

    fn deps(input: &str) -> Dependencies {
        let expr = parser::parse(input).unwrap();
        extract_dependencies(&expr)
    }

    #[test]
    fn distinguishes_literal_and_computed_instance_names() {
        assert!(deps("instance($sourceName, 'value')").has_dynamic_instance);
        assert!(!deps("instance('record', $field)").has_dynamic_instance);
        assert!(!deps("@instance('record').value").has_dynamic_instance);
        assert!(!deps("instance()").has_dynamic_instance);
    }

    #[test]
    fn test_simple_field_ref() {
        let d = deps("$name");
        assert!(d.fields.contains("name"));
    }

    #[test]
    fn test_nested_field_ref() {
        let d = deps("$address.city");
        assert!(d.fields.contains("address.city"));
    }

    #[test]
    fn test_wildcard_detection() {
        let d = deps("$items[*].qty");
        assert!(d.has_wildcard);
        assert!(d.fields.contains("items[*].qty"));
    }

    #[test]
    fn postfix_on_projection_reads_the_inner_field_path() {
        let d = deps("($rows[*].x)[1]");
        assert!(d.has_wildcard);
        assert_eq!(d.fields.into_iter().collect::<Vec<_>>(), ["rows[*].x"]);
    }

    #[test]
    fn test_bare_dollar() {
        let d = deps("$");
        assert!(d.has_self_ref);
    }

    #[test]
    fn test_context_ref() {
        let d = deps("@index");
        assert!(d.context_refs.contains("@index"));
    }

    #[test]
    fn test_instance_ref() {
        let d = deps("@instance('priorYear').total");
        assert!(d.instance_reads.contains_key("priorYear"));
    }

    #[test]
    fn test_let_binding_excludes_bound_var() {
        let d = deps("let x = $a in x + $b");
        assert!(d.fields.contains("a"));
        assert!(d.fields.contains("b"));
        assert!(!d.fields.contains("x"));
    }

    #[test]
    fn test_mip_deps() {
        let d = deps("valid($name)");
        assert!(d.mip_deps.contains("name"));
    }

    #[test]
    fn test_prev_next_detection() {
        let d = deps("prev()");
        assert!(d.uses_prev_next);
    }

    #[test]
    fn test_multiple_fields() {
        let d = deps("$a + $b * $c");
        assert_eq!(d.fields.len(), 3);
        assert!(d.fields.contains("a"));
        assert!(d.fields.contains("b"));
        assert!(d.fields.contains("c"));
    }

    /// Spec: core/spec.md §3.6.1, fel-grammar.md §4 precedence 7 —
    /// NullCoalesce (`??`) dep extraction must walk both sides.
    #[test]
    fn null_coalesce_extracts_deps_from_both_sides() {
        let d = deps("$fieldA ?? $fieldB");
        assert!(
            d.fields.contains("fieldA"),
            "left side of ?? must produce a dep"
        );
        assert!(
            d.fields.contains("fieldB"),
            "right side of ?? must produce a dep"
        );
        assert_eq!(d.fields.len(), 2);
    }

    /// Spec: core/spec.md §3.6.1, fel-grammar.md §4 precedence 1 —
    /// Ternary (`? :`) dep extraction must walk condition, true-branch, and false-branch.
    #[test]
    fn ternary_extracts_deps_from_all_three_branches() {
        let d = deps("$cond ? $yes : $no");
        assert!(d.fields.contains("cond"), "condition must produce a dep");
        assert!(d.fields.contains("yes"), "true-branch must produce a dep");
        assert!(d.fields.contains("no"), "false-branch must produce a dep");
        assert_eq!(d.fields.len(), 3);
    }

    /// Spec: core/spec.md §3.6.1, fel-grammar.md §4.2-4.3 —
    /// Field refs nested inside object and array literals must be found.
    #[test]
    fn object_and_array_literal_dep_extraction() {
        let d = deps("{total: $price, items: [$qty, $tax]}");
        assert!(
            d.fields.contains("price"),
            "object value must produce a dep"
        );
        assert!(d.fields.contains("qty"), "array element must produce a dep");
        assert!(d.fields.contains("tax"), "array element must produce a dep");
        assert_eq!(d.fields.len(), 3);
    }

    /// Spec: core/spec.md §3.5.1 — countWhere's predicate rebinds `$` to the
    /// current element. Bare `$` inside the predicate is NOT a field self-ref.
    #[test]
    fn count_where_rebinds_bare_dollar() {
        let d = deps("countWhere($items, $ > 3)");
        assert!(
            d.fields.contains("items"),
            "first arg field ref must be found"
        );
        assert!(
            !d.has_self_ref,
            "bare $ in countWhere predicate is rebound, not a self-ref"
        );
        // The predicate's `$` should NOT appear in fields
        assert_eq!(d.fields.len(), 1);
    }

    /// Spec: core/spec.md §3.5.1 — `every` rebinds `$` like `countWhere`.
    #[test]
    fn every_rebinds_bare_dollar() {
        let d = deps("every($items, $ > 0)");
        assert!(d.fields.contains("items"));
        assert!(!d.has_self_ref);
        assert_eq!(d.fields.len(), 1);
    }

    /// Spec: core/spec.md §3.5.1 — `sumWhere` rebinds `$` like `countWhere` (via `filter_where`).
    #[test]
    fn sum_where_rebinds_bare_dollar() {
        let d = deps("sumWhere($items, $ > 0)");
        assert!(d.fields.contains("items"));
        assert!(!d.has_self_ref);
        assert_eq!(d.fields.len(), 1);
    }

    /// Spec: core/spec.md §3.6.1 — deeply nested function calls (3+ levels)
    /// must recursively extract all deps.
    #[test]
    fn deeply_nested_function_calls_extract_all_deps() {
        let d = deps("sum(round($a + $b) + every($items, $ > $threshold))");
        assert!(d.fields.contains("a"));
        assert!(d.fields.contains("b"));
        assert!(d.fields.contains("items"));
        assert!(d.fields.contains("threshold"));
        assert_eq!(d.fields.len(), 4);
    }

    mod path_segment_append {
        use super::deps;
        use crate::ast::{PathDotJoin, PathSegment};

        #[test]
        fn walk_style_always_prefixes_dot_segments() {
            let mut path = "items".to_string();
            PathSegment::Dot("qty".into())
                .append_to_path(&mut path, PathDotJoin::PrefixWhenNonempty);
            assert_eq!(path, "items.qty");
        }

        #[test]
        fn extract_style_skips_dot_on_empty_base() {
            let mut path = String::new();
            PathSegment::Dot("a".into()).append_to_path(&mut path, PathDotJoin::PrefixWhenNonempty);
            assert_eq!(path, "a");
        }

        #[test]
        fn extract_style_prefixes_dot_when_base_nonempty() {
            let mut path = "items".to_string();
            PathSegment::Dot("qty".into())
                .append_to_path(&mut path, PathDotJoin::PrefixWhenNonempty);
            assert_eq!(path, "items.qty");
        }

        #[test]
        fn index_and_wildcard_rendering() {
            let mut path = "items".to_string();
            PathSegment::Index(2).append_to_path(&mut path, PathDotJoin::PrefixWhenNonempty);
            PathSegment::Wildcard.append_to_path(&mut path, PathDotJoin::PrefixWhenNonempty);
            PathSegment::Dot("qty".into())
                .append_to_path(&mut path, PathDotJoin::PrefixWhenNonempty);
            assert_eq!(path, "items[2][*].qty");
        }

        #[test]
        fn mip_empty_base_wildcard_path() {
            let d = deps("valid($[*])");
            assert!(d.mip_deps.contains("[*]"));
        }

        #[test]
        fn indexed_field_path_round_trip() {
            let d = deps("$rows[2].id");
            assert!(d.fields.contains("rows[2].id"));
        }
    }
}
