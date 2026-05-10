//! Build [`FormspecEnvironment`] from JSON-shaped evaluation context.
//!
//! `push_repeat_context` recursively walks nested repeat JSON into environment state.
#![allow(clippy::missing_docs_in_private_items)]

use serde_json::{Map, Value};

use crate::convert::json_to_fel;
use crate::types::Value as CoreValue;
use crate::{FormspecEnvironment, MipState};

fn push_repeat_context(env: &mut FormspecEnvironment, repeat: &Value, depth: u8) {
    if depth > 32 {
        return;
    }
    let Some(obj) = repeat.as_object() else {
        return;
    };

    if let Some(parent) = obj.get("parent") {
        push_repeat_context(env, parent, depth + 1);
    }

    let current = obj
        .get("current")
        .map(json_to_fel)
        .unwrap_or(CoreValue::Null);
    let index = obj.get("index").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
    let count = obj.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let collection = obj
        .get("collection")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(json_to_fel).collect())
        .unwrap_or_default();
    env.push_repeat(current, index, count, collection);
}

/// Populate a [`FormspecEnvironment`] from a JSON object (e.g. WASM `evalFELWithContext` payload).
///
/// Recognized keys: `nowIso` / `now_iso`, `fields`, `variables`, `mipStates` / `mip_states`,
/// `repeatContext` / `repeat_context`, `instances`, `locale`, `meta`.
pub fn formspec_environment_from_json_map(ctx: &Map<String, Value>) -> FormspecEnvironment {
    let mut env = FormspecEnvironment::new();

    if let Some(now_iso) = ctx
        .get("nowIso")
        .or_else(|| ctx.get("now_iso"))
        .and_then(|v| v.as_str())
    {
        env.set_now_from_iso(now_iso);
    }

    if let Some(fields) = ctx.get("fields").and_then(|v| v.as_object()) {
        for (k, v) in fields {
            env.set_field(k, json_to_fel(v));
        }
    }

    if let Some(vars) = ctx.get("variables").and_then(|v| v.as_object()) {
        for (k, v) in vars {
            env.set_variable(k, json_to_fel(v));
        }
    }

    if let Some(mips) = ctx
        .get("mipStates")
        .or_else(|| ctx.get("mip_states"))
        .and_then(|v| v.as_object())
    {
        for (k, v) in mips {
            if let Some(mip_obj) = v.as_object() {
                env.set_mip(
                    k,
                    MipState {
                        valid: mip_obj
                            .get("valid")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true),
                        relevant: mip_obj
                            .get("relevant")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true),
                        readonly: mip_obj
                            .get("readonly")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        required: mip_obj
                            .get("required")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                    },
                );
            }
        }
    }

    if let Some(repeat) = ctx
        .get("repeatContext")
        .or_else(|| ctx.get("repeat_context"))
    {
        push_repeat_context(&mut env, repeat, 0);
    }

    if let Some(instances) = ctx.get("instances").and_then(|v| v.as_object()) {
        for (k, v) in instances {
            env.set_instance(k, json_to_fel(v));
        }
    }

    if let Some(locale) = ctx.get("locale").and_then(|v| v.as_str()) {
        env.set_locale(locale);
    }

    if let Some(meta) = ctx.get("meta").and_then(|v| v.as_object()) {
        for (k, v) in meta {
            env.set_meta(k, json_to_fel(v));
        }
    }

    env
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_docs_in_private_items)]
    use indexmap::IndexMap;
    use rust_decimal::Decimal;
    use serde_json::json;

    use crate::types::Value as CoreValue;

    use super::*;

    #[test]
    fn builds_env_from_fields_and_now_iso() {
        let ctx = json!({
            "nowIso": "2020-01-01T00:00:00Z",
            "fields": { "n": 3 }
        });
        let obj = ctx.as_object().unwrap();
        let env = formspec_environment_from_json_map(obj);
        assert_eq!(
            env.data.get("n"),
            Some(&CoreValue::Number(Decimal::from(3)))
        );
        assert!(env.current_datetime.is_some());
    }

    #[test]
    fn builds_env_from_snake_case_context_keys() {
        let ctx = json!({
            "now_iso": "2020-01-02T03:04:05Z",
            "fields": { "a": 1 },
            "variables": { "v": "x" },
            "instances": { "i": { "k": 2 } },
            "locale": "en-US",
            "meta": { "flag": true },
            "mip_states": {
                "a": {
                    "valid": false,
                    "relevant": true,
                    "readonly": true,
                    "required": false
                }
            }
        });
        let env = formspec_environment_from_json_map(ctx.as_object().unwrap());

        assert_eq!(
            env.data.get("a"),
            Some(&CoreValue::Number(Decimal::from(1)))
        );
        assert_eq!(
            env.variables.get("v"),
            Some(&CoreValue::String("x".to_string()))
        );
        assert_eq!(
            env.instances.get("i"),
            Some(&CoreValue::Object(IndexMap::from([(
                "k".to_string(),
                CoreValue::Number(Decimal::from(2)),
            )])))
        );
        assert_eq!(env.locale.as_deref(), Some("en-US"));
        assert_eq!(env.meta.get("flag"), Some(&CoreValue::Boolean(true)));
        assert!(env.current_datetime.is_some());

        let mip = env.mip_states.get("a").expect("mip state for a");
        assert!(!mip.valid);
        assert!(mip.relevant);
        assert!(mip.readonly);
        assert!(!mip.required);
    }

    #[test]
    fn repeat_context_builds_parent_chain_and_defaults() {
        let ctx = json!({
            "repeat_context": {
                "current": "child",
                "index": 2,
                "count": 3,
                "collection": ["child", "other"],
                "parent": {
                    "current": "parent"
                }
            }
        });
        let env = formspec_environment_from_json_map(ctx.as_object().unwrap());
        let repeat = env.repeat_context.as_ref().expect("repeat context");
        assert_eq!(repeat.current, CoreValue::String("child".to_string()));
        assert_eq!(repeat.index, 2);
        assert_eq!(repeat.count, 3);
        assert_eq!(
            repeat.collection,
            vec![
                CoreValue::String("child".to_string()),
                CoreValue::String("other".to_string())
            ]
        );

        let parent = repeat.parent.as_ref().expect("parent repeat context");
        assert_eq!(parent.current, CoreValue::String("parent".to_string()));
        // Defaults apply when omitted.
        assert_eq!(parent.index, 1);
        assert_eq!(parent.count, 0);
        assert!(parent.collection.is_empty());
    }

    #[test]
    fn mip_defaults_apply_for_partial_entries() {
        let ctx = json!({
            "mipStates": {
                "q": { "readonly": true }
            }
        });
        let env = formspec_environment_from_json_map(ctx.as_object().unwrap());
        let mip = env.mip_states.get("q").expect("mip state q");
        assert!(mip.valid);
        assert!(mip.relevant);
        assert!(mip.readonly);
        assert!(!mip.required);
    }
}
