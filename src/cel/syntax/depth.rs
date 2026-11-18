//! Store a deep expression as a shallow sequence of editable nodes for FEL's parser.
// Rust guideline compliant 2026-02-21

use super::Form;

/// Transform each immediate child while preserving the parent node.
pub(super) fn map_children(
    node: &Form,
    mut visit: impl FnMut(&Form) -> Result<Form, String>,
) -> Result<Form, String> {
    let mut one = |x: &Form| visit(x);
    Ok(match node {
        Form::List(xs) => Form::List(xs.iter().map(one).collect::<Result<_, _>>()?),
        Form::Object(xs) => Form::Object(
            xs.iter()
                .map(|(k, v)| Ok((k.clone(), one(v)?)))
                .collect::<Result<_, String>>()?,
        ),
        Form::Call(name, args) => Form::Call(
            name.clone(),
            args.iter().map(one).collect::<Result<_, _>>()?,
        ),
        Form::Binary(op, a, b) => Form::Binary(op, Box::new(one(a)?), Box::new(one(b)?)),
        Form::Choice(c, t, f) => {
            Form::Choice(Box::new(one(c)?), Box::new(one(t)?), Box::new(one(f)?))
        }
        Form::Not(v) => Form::Not(Box::new(one(v)?)),
        leaf => leaf.clone(),
    })
}

/// Store a deep tree in postorder using references to earlier rows.
pub(super) fn pack(root: &Form) -> Result<Form, String> {
    /// Append the children and parent, then return the parent reference.
    fn append(node: &Form, rows: &mut Vec<Form>) -> Result<Form, String> {
        let row = map_children(node, |child| append(child, rows))?;
        let index = i64::try_from(rows.len()).map_err(|_| "too many expression nodes")?;
        rows.push(row);
        Ok(Form::Call("bridge_ref".into(), vec![Form::Int(index)]))
    }
    let mut rows = Vec::new();
    append(root, &mut rows)?;
    Ok(Form::Call("bridge_tree".into(), vec![Form::List(rows)]))
}

/// Restore a flat record, checking depth and single ownership of each node.
pub(super) fn unpack(node: Form) -> Result<Form, String> {
    let Form::Call(tag, args) = &node else {
        return Ok(node);
    };
    if tag != "bridge_tree" {
        return Ok(node);
    }
    let [Form::List(rows)] = args.as_slice() else {
        return Err("expected a list of syntax nodes".into());
    };
    let mut done: Vec<Option<(Form, usize)>> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut height = 1;
        let node = map_children(row, |r| {
            if let Form::Call(tag, args) = r
                && tag == "bridge_ref"
                && let [Form::Int(i)] = args.as_slice()
                && let Ok(i) = usize::try_from(*i)
                && let Some((value, child_height)) = done.get_mut(i).and_then(Option::take)
            {
                height = height.max(child_height + 1);
                if height > super::super::limits::MAX_TREE_HEIGHT {
                    return Err("restored syntax tree exceeds the depth limit".into());
                }
                return Ok(value);
            }
            Err("a syntax reference must point to one earlier, unused node".into())
        })?;
        done.push(Some((node, height)));
    }
    let (root, _) = done.pop().flatten().ok_or("empty syntax tree")?;
    if done.iter().any(Option::is_some) {
        return Err("unused syntax nodes".into());
    }
    Ok(root)
}
