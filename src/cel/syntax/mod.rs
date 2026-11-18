//! Reversible source notation for nodes outside the native translation profile.
// Rust guideline compliant 2026-02-21

mod cel;
mod depth;
mod fel;

use std::fmt::Write;

use super::Language;

impl Language {
    /// Parse source in this language before interpreting versioned editing notation.
    pub(super) fn parse(self, source: &str, marked: bool) -> Result<Form, String> {
        super::limits::check(source, self).map_err(|e| e.to_string())?;
        match self {
            Self::Cel => cel::parse(source, marked),
            Self::Fel => fel::parse(source, marked),
        }
    }
    /// Quote a string using the destination grammar.
    fn quote(self, s: &str) -> String {
        match self {
            Self::Cel => serde_json::Value::String(s.to_owned()).to_string(),
            Self::Fel => crate::print_expr(&crate::Expr::String(s.to_owned())),
        }
    }

    /// Identify an original-source literal for this language.
    fn source_tag(self) -> &'static str {
        match self {
            Self::Cel => "bridge_cel_source",
            Self::Fel => "bridge_fel_source",
        }
    }
}

/// Shared syntax and named representations for language-specific constructs.
#[derive(Debug, PartialEq, Clone)]
pub(super) enum Form {
    /// A null literal.
    Null,
    /// A Boolean literal.
    Bool(bool),
    /// A signed integer literal.
    Int(i64),
    /// A string literal.
    String(String),
    /// A qualified field or variable name.
    Path(String),
    /// An ordered list of expressions.
    List(Vec<Form>),
    /// An object with required string keys.
    Object(Vec<(String, Form)>),
    /// An ordinary call or a versioned notation helper.
    Call(String, Vec<Form>),
    /// An operator with two operands.
    Binary(&'static str, Box<Form>, Box<Form>),
    /// A logical negation.
    Not(Box<Form>),
    /// A conditional with both branches.
    Choice(Box<Form>, Box<Form>, Box<Form>),
}

/// Check whether both grammars accept the unquoted name.
fn portable(name: &str) -> bool {
    crate::lexer::is_valid_fel_identifier(name)
        && !matches!(
            name,
            "as" | "break"
                | "const"
                | "continue"
                | "for"
                | "function"
                | "import"
                | "loop"
                | "package"
                | "namespace"
                | "return"
                | "var"
                | "void"
                | "while"
        )
}

/// Escape author-defined names that collide with editing notation.
fn ordinary_call(name: &str, marked: bool) -> bool {
    portable(name) && (marked || !name.starts_with("bridge_"))
}

/// Represent a language-specific literal by its spelling.
fn atom(tag: &str, source: String) -> Form {
    Form::Call(tag.to_owned(), vec![Form::String(source)])
}

/// Represent a language-specific operation with editable operands.
fn operation(tag: &str, children: Vec<Form>) -> Form {
    Form::Call(tag.to_owned(), children)
}

/// Return the delimiters around a fixed-arity operation.
fn fixed_parts(tag: &str) -> Option<&'static [&'static str]> {
    Some(match tag {
        "bridge_cel_div" | "bridge_fel_div" => &["(", " / ", ")"],
        "bridge_fel_concat" => &["(", " & ", ")"],
        "bridge_fel_coalesce" => &["(", " ?? ", ")"],
        "bridge_fel_if" => &["(if ", " then ", " else ", ")"],
        "bridge_fel_not_in" => &["(", " not in ", ")"],
        "bridge_fel_negate" => &["(-", ")"],
        "bridge_fel_not" => &["(not ", ")"],
        "bridge_cel_index" => &["", "[", "]"],
        "bridge_cel_optional_index" => &["", "[?", "]"],
        _ => return None,
    })
}

/// Preserve a function name even when it coincides with notation.
fn function(name: &str, mut args: Vec<Form>, marked: bool, tag: &str) -> Form {
    if ordinary_call(name, marked) {
        return Form::Call(name.to_owned(), args);
    }
    args.insert(0, Form::String(name.to_owned()));
    operation(tag, args)
}

/// Combine operands under a shared operator.
fn binary(op: &'static str, left: Form, right: Form) -> Form {
    Form::Binary(op, Box::new(left), Box::new(right))
}

/// Identify precedence groups whose left operand can omit parentheses in FEL.
fn fel_group(node: &Form) -> Option<u8> {
    let op = match node {
        Form::Binary(op, ..) => *op,
        Form::Call(tag, _) => tag.as_str(),
        Form::Not(_) => "!",
        _ => return None,
    };
    Some(match op {
        "||" => 1,
        "&&" => 2,
        "bridge_fel_coalesce" => 3,
        "+" | "-" | "bridge_fel_concat" => 4,
        "*" | "%" | "bridge_fel_div" => 5,
        "!" | "bridge_fel_not" | "bridge_fel_negate" => 6,
        _ => return None,
    })
}

/// Group a negative receiver before applying a CEL selection or index.
fn cel_receiver(text: String) -> String {
    if text.starts_with('-') {
        format!("({text})")
    } else {
        text
    }
}

/// Extract the optional marker from a CEL list element or field.
fn cel_entry(form: &Form) -> (&str, &Form) {
    if let Form::Call(tag, args) = form
        && tag == "bridge_cel_optional"
        && let [value] = args.as_slice()
    {
        ("?", value)
    } else {
        ("", form)
    }
}

/// Render the keys and values of a CEL map or message.
fn cel_fields(entries: &[Form], message: bool) -> Result<String, String> {
    let mut printed = Vec::with_capacity(entries.len());
    for entry in entries {
        let (prefix, raw) = cel_entry(entry);
        let Form::List(pair) = raw else {
            return Err("expected a key and a value".into());
        };
        let [key, value] = pair.as_slice() else {
            return Err("expected two elements in a field".into());
        };
        let key = if message {
            let Form::String(name) = key else {
                return Err("expected a message field name".into());
            };
            cel::field_name(name)
        } else {
            render(key, Language::Cel, true)?
        };
        printed.push(format!(
            "{prefix}{key}: {}",
            render(value, Language::Cel, true)?
        ));
    }
    Ok(format!("{{{}}}", printed.join(", ")))
}

/// Render an editable form or restore its original language construct.
fn render(form: &Form, dialect: Language, restore: bool) -> Result<String, String> {
    let child = |x: &Form| render(x, dialect, restore);
    let first = |operand: &Form| -> Result<String, String> {
        let result = child(operand)?;
        if dialect == Language::Fel
            && let Some(group) = fel_group(form)
            && fel_group(operand) == Some(group)
            && let Some(inner) = result.strip_prefix('(').and_then(|s| s.strip_suffix(')'))
        {
            return Ok(inner.to_owned());
        }
        Ok(result)
    };
    let joined = |xs: &[Form]| {
        xs.iter()
            .map(child)
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join(", "))
    };
    Ok(match form {
        Form::Null => "null".to_owned(),
        Form::Bool(v) => v.to_string(),
        Form::Int(v) => v.to_string(),
        Form::String(v) => dialect.quote(v),
        Form::Path(v) => {
            if dialect == Language::Fel {
                format!("${v}")
            } else {
                v.clone()
            }
        }
        Form::List(xs) => format!("[{}]", joined(xs)?),
        Form::Object(xs) => {
            let entries = xs
                .iter()
                .map(|(k, v)| Ok(format!("{}: {}", dialect.quote(k), child(v)?)))
                .collect::<Result<Vec<_>, String>>()?;
            format!("{{{}}}", entries.join(", "))
        }
        Form::Binary(op, a, b) => {
            let glyph = match (dialect, *op) {
                (Language::Fel, "==") => "=",
                (Language::Fel, "&&") => "and",
                (Language::Fel, "||") => "or",
                _ => op,
            };
            format!("({} {glyph} {})", first(a)?, child(b)?)
        }
        Form::Not(v) => format!("(!{})", first(v)?),
        Form::Choice(c, t, f) => format!("({} ? {} : {})", child(c)?, child(t)?, child(f)?),
        Form::Call(name, args) if restore && name.starts_with("bridge_") => {
            let prefix = if dialect == Language::Cel {
                "bridge_cel_"
            } else {
                "bridge_fel_"
            };
            if !name.starts_with(prefix) {
                return Err(format!("unexpected {name} in a {} rule", dialect.label()));
            }
            if let Some(parts) = fixed_parts(name) {
                if parts.len() != args.len() + 1 {
                    return Err(format!("invalid number of operands for {name}"));
                }
                let mut result = parts[0].to_owned();
                for (i, (v, tail)) in args.iter().zip(&parts[1..]).enumerate() {
                    let mut operand = if i == 0 { first(v)? } else { child(v)? };
                    if i == 0
                        && matches!(
                            name.as_str(),
                            "bridge_cel_index" | "bridge_cel_optional_index"
                        )
                    {
                        operand = cel_receiver(operand);
                    }
                    result.push_str(&operand);
                    result.push_str(tail);
                }
                return Ok(result);
            }
            match (name.as_str(), args.as_slice()) {
                ("bridge_cel_and" | "bridge_cel_or", args) if args.len() >= 2 => {
                    let glyph = if name == "bridge_cel_and" {
                        " && "
                    } else {
                        " || "
                    };
                    format!(
                        "({})",
                        args.iter()
                            .map(child)
                            .collect::<Result<Vec<_>, _>>()?
                            .join(glyph)
                    )
                }
                ("bridge_cel_prefix", [Form::String(ops), v])
                    if ops.contains(['!', '-'])
                        && ops.bytes().all(|c| matches!(c, b'!' | b'-' | b'(')) =>
                {
                    format!(
                        "({ops}{}{}",
                        child(v)?,
                        ")".repeat(1 + ops.matches('(').count())
                    )
                }
                ("bridge_cel_list", args) => {
                    let xs = args
                        .iter()
                        .map(|arg| {
                            let (prefix, value) = cel_entry(arg);
                            Ok(format!("{prefix}{}", child(value)?))
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    format!("[{}]", xs.join(", "))
                }
                ("bridge_cel_map", entries) => cel_fields(entries, false)?,
                ("bridge_cel_message", [message, fields @ ..]) => {
                    format!("{}{}", child(message)?, cel_fields(fields, true)?)
                }
                ("bridge_fel_call" | "bridge_cel_call", [Form::String(name), operands @ ..]) => {
                    format!("{name}({})", joined(operands)?)
                }
                ("bridge_cel_call", [callee, operands @ ..]) => {
                    format!("{}({})", child(callee)?, joined(operands)?)
                }
                ("bridge_cel_method", [receiver, Form::String(method), operands @ ..]) => format!(
                    "{}.{}({})",
                    cel_receiver(child(receiver)?),
                    method,
                    joined(operands)?
                ),
                (
                    "bridge_cel_select" | "bridge_cel_optional_select",
                    [receiver, Form::String(field)],
                ) => format!(
                    "{}.{}{}",
                    cel_receiver(child(receiver)?),
                    if name == "bridge_cel_optional_select" {
                        "?"
                    } else {
                        ""
                    },
                    cel::field_name(field)
                ),
                ("bridge_fel_let", [Form::String(id), value, body]) => {
                    format!("(let {id} = ({}) in {})", child(value)?, child(body)?)
                }
                ("bridge_fel_lookup", [source, Form::String(key), value]) => {
                    format!("({})[{key} = {}]", child(source)?, child(value)?)
                }
                ("bridge_fel_access", [source, Form::String(path)]) => {
                    format!("({}){path}", child(source)?)
                }
                ("bridge_cel_bytes", [Form::String(hex)]) => {
                    if hex.len() % 2 != 0 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
                        return Err("CEL bytes require pairs of hexadecimal digits".into());
                    }
                    let mut result = String::from("b\"");
                    for i in (0..hex.len()).step_by(2) {
                        write!(result, "\\x{}", &hex[i..i + 2]).expect("writing a string");
                    }
                    result.push('"');
                    result
                }
                (
                    "bridge_cel_double"
                    | "bridge_cel_uint"
                    | "bridge_cel_root"
                    | "bridge_cel_name"
                    | "bridge_fel_decimal"
                    | "bridge_fel_date"
                    | "bridge_fel_datetime"
                    | "bridge_fel_field"
                    | "bridge_fel_variable"
                    | "bridge_fel_context",
                    [Form::String(token)],
                ) => token.clone(),
                _ => return Err(format!("invalid arguments for {name}")),
            }
        }
        Form::Call(name, args) => format!("{name}({})", joined(args)?),
    })
}

/// Stable version prefix for text passed between editing tools.
const MARK: &str = "// cel-fel-syntax v1 from ";

/// Recognize the reserved opening comment, including unknown versions.
pub(super) fn has_marker(source: &str) -> bool {
    source.trim_start().starts_with("// cel-fel-syntax ")
}

/// Reopen v1 operators written before the CEL parser normalized chains and prefixes.
fn canonical_v1_cel(node: &Form) -> Result<Form, String> {
    let node = depth::map_children(node, canonical_v1_cel)?;
    if let Form::Call(name, args) = &node
        && matches!(
            name.as_str(),
            "bridge_cel_and" | "bridge_cel_or" | "bridge_cel_prefix"
        )
    {
        // A legacy operator may normalize its grouping. Each operand must still
        // express the same node, including any edited type-specific literal.
        let prefix_argument = usize::from(name == "bridge_cel_prefix");
        for operand in args.iter().skip(prefix_argument) {
            if Language::Cel.parse(&render(operand, Language::Cel, true)?, false)? != *operand {
                return Err("edited CEL operand changed its source construct".into());
            }
        }
        return Language::Cel.parse(&render(&node, Language::Cel, true)?, false);
    }
    Ok(node)
}

/// Convert an original expression or restore its versioned text.
pub(super) fn convert(source: &str, from: Language, literal: bool) -> Result<String, String> {
    let to = from.other();
    if !literal && has_marker(source) {
        let (header, body) = source
            .trim_start()
            .split_once('\n')
            .ok_or("missing expression after syntax marker")?;
        if header.trim_end() != format!("{MARK}{}", to.label()) {
            return Err(format!("expected a v1 syntax marker from {}", to.label()));
        }
        let tree = depth::unpack(from.parse(body, true)?)?;
        if let Form::Call(tag, args) = &tree
            && tag == to.source_tag()
            && let [Form::String(original)] = args.as_slice()
        {
            to.parse(original, false)?;
            return Ok(original.clone());
        }
        let restored = render(&tree, to, true)?;
        let parsed = to.parse(&restored, false)?;
        if parsed != tree && !(to == Language::Cel && canonical_v1_cel(&tree)? == parsed) {
            return Err("edited syntax no longer describes the indicated source construct".into());
        }
        return Ok(restored);
    }
    let tree = from.parse(source, false)?;
    let printed = match structured_body(source, &tree, from, to) {
        Ok(body) => body,
        Err(_) => {
            // Keep the original spelling when the other grammar cannot carry this tree.
            let form = atom(from.source_tag(), source.to_owned());
            let body = render(&form, to, false)?;
            if to.parse(&body, true)? != form {
                return Err("could not preserve the original source as a string literal".into());
            }
            body
        }
    };
    Ok(format!("{MARK}{}\n{printed}", from.label()))
}

/// Verify that both parsers recover the same editable structure.
fn structured_body(
    source: &str,
    tree: &Form,
    from: Language,
    to: Language,
) -> Result<String, String> {
    let mut printed = render(tree, to, false)?;
    let reparsed = match to.parse(&printed, true) {
        Err(e) if e.contains("maximum depth") => {
            printed = render(&depth::pack(tree)?, to, false)?;
            depth::unpack(to.parse(&printed, true)?)?
        }
        other => other?,
    };
    let restored = render(tree, from, true)?;
    if reparsed != *tree
        || from.parse(&restored, false)? != *tree
        || (from == Language::Fel && crate::parse(source).ok() != crate::parse(&restored).ok())
    {
        return Err("source structure changed while printing the converted expression".into());
    }
    Ok(printed)
}
