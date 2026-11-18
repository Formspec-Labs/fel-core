//! Recognize the CEL filter and size expression for a non-null value count.
// Rust guideline compliant 2026-02-21

use super::syntax::Form;

/// Return the list and whether its item was cast to CEL's dynamic type.
pub(super) fn non_null_count(node: &Form) -> Option<(&Form, bool)> {
    let Form::Call(name, args) = node else {
        return None;
    };
    let filtered = match (name.as_str(), args.as_slice()) {
        ("size", [list]) => list,
        ("bridge_cel_method", [list, Form::String(method)]) if method == "size" => list,
        _ => return None,
    };
    let Form::Call(tag, args) = filtered else {
        return None;
    };
    let [
        list,
        Form::String(method),
        Form::Path(bound),
        Form::Binary("!=", left, right),
    ] = args.as_slice()
    else {
        return None;
    };
    if tag != "bridge_cel_method" || method != "filter" || bound.contains('.') {
        return None;
    }
    let value = match (left.as_ref(), right.as_ref()) {
        (item, Form::Null) | (Form::Null, item) => item,
        _ => return None,
    };
    match value {
        Form::Path(item) if item == bound => Some((list, false)),
        Form::Call(f, args)
            if f == "dyn" && matches!(args.as_slice(), [Form::Path(item)] if item == bound) =>
        {
            Some((list, true))
        }
        _ => None,
    }
}
