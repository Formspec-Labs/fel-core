//! Bound recursive parsing, printing and field declaration work.
// Rust guideline compliant 2026-02-21

use super::{ConversionError, Declaration, Language, MAX_SOURCE_BYTES};

/// Bounds the recursive CEL parser and the editable tree printer.
pub(super) const MAX_DEPTH: usize = 64;
/// A flattened editing record can describe a deeper tree than its source text.
pub(super) const MAX_TREE_HEIGHT: usize = 128;
/// Includes postfix calls, infix operators, and keyword operators.
const MAX_OPERATIONS: usize = 256;

/// Reserve stack for the bounded recursive parsers, even on small host threads.
pub(super) fn with_parser_stack<T>(work: impl FnOnce() -> T) -> T {
    const MIB: usize = 1024 * 1024;
    stacker::maybe_grow(8 * MIB, 16 * MIB, work)
}

/// Find the end of a quoted token, skipping escaped delimiters.
fn quoted(bytes: &[u8], start: usize, lang: Language, raw: bool) -> usize {
    let quote = bytes[start];
    let width = if lang == Language::Cel
        && quote != b'`'
        && bytes.get(start..start + 3) == Some(&[quote; 3])
    {
        3
    } else {
        1
    };
    let delimiter = &bytes[start..start + width];
    let mut at = start + width;
    while at < bytes.len() {
        if bytes[at..].starts_with(delimiter) {
            return at + width;
        }
        if !raw && bytes[at] == b'\\' {
            at += 1;
        }
        at += 1;
    }
    bytes.len()
}

/// Check source before entering recursive code, including generated source.
pub(super) fn check(source: &str, lang: Language) -> Result<(), ConversionError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(ConversionError::Limit(format!(
            "source exceeds {MAX_SOURCE_BYTES} UTF-8 bytes"
        )));
    }
    let bytes = source.as_bytes();
    let mut at = 0;
    let mut nesting = 0usize;
    let mut operations = 0;
    while at < bytes.len() {
        let rest = &bytes[at..];
        match rest[0] {
            b'/' if rest.starts_with(b"//") => {
                at += rest.iter().position(|c| *c == b'\n').unwrap_or(rest.len());
            }
            b'/' if lang == Language::Fel && rest.starts_with(b"/*") => {
                at += rest[2..]
                    .windows(2)
                    .position(|s| s == b"*/")
                    .map_or(rest.len(), |i| i + 4);
            }
            b'\'' | b'"' => at = quoted(bytes, at, lang, false),
            b'`' if lang == Language::Cel => at = quoted(bytes, at, lang, true),
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = at;
                at += rest
                    .iter()
                    .take_while(|c| c.is_ascii_alphanumeric() || **c == b'_')
                    .count();
                let word = &source[start..at];
                if lang == Language::Cel
                    && ["r", "br", "rb"]
                        .iter()
                        .any(|prefix| word.eq_ignore_ascii_case(prefix))
                    && matches!(bytes.get(at), Some(b'\'' | b'"'))
                {
                    at = quoted(bytes, at, lang, true);
                } else if matches!(word, "in" | "and" | "or" | "not" | "let" | "if") {
                    operations += 1;
                }
            }
            b'(' | b'[' | b'{' => {
                nesting += 1;
                operations += 1;
                at += 1;
            }
            b')' | b']' | b'}' => {
                nesting = nesting.saturating_sub(1);
                at += 1;
            }
            c => {
                if b".+-*/%&|!<>=?".contains(&c) {
                    operations += 1;
                }
                at += 1;
            }
        }
        if nesting > MAX_DEPTH || operations > MAX_OPERATIONS {
            return Err(ConversionError::Limit(format!(
                "expression exceeds {MAX_DEPTH} nested delimiters or {MAX_OPERATIONS} structural operations"
            )));
        }
    }
    Ok(())
}

/// Validate all user-supplied text before the parsers inspect a binding.
pub(super) fn check_bindings(bindings: &[Declaration]) -> Result<(), ConversionError> {
    if bindings.len() > MAX_OPERATIONS {
        return Err(ConversionError::Limit(format!(
            "at most {MAX_OPERATIONS} field bindings are supported"
        )));
    }
    let mut total = 0usize;
    for binding in bindings {
        total = total
            .saturating_add(binding.name.len())
            .saturating_add(binding.field.len());
        if total > MAX_SOURCE_BYTES {
            return Err(ConversionError::Limit(
                "field bindings exceed the source byte limit".into(),
            ));
        }
        check(&binding.name, Language::Cel)?;
        check(&binding.field, Language::Fel)?;
    }
    Ok(())
}
