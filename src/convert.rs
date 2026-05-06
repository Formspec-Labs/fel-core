//! Canonical conversion between serde_json::Value and TypeValue.
//!
//! These are the single source of truth for JSON↔FEL value conversion.
//! All crates should use these instead of rolling their own.

use std::collections::HashMap;

use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde_json::Value;

use crate::types::{Money as TypeMoney, Value as TypeValue};

/// JSON object → flat field map for FEL `MapEnvironment` (`{}` / empty → empty map).
pub fn json_object_to_field_map(val: &Value) -> HashMap<String, TypeValue> {
    let mut map = HashMap::new();
    if let Some(obj) = val.as_object() {
        for (k, v) in obj {
            map.insert(k.clone(), json_to_fel(v));
        }
    }
    map
}

/// Parse a JSON object string into a field map (empty or `"{}"` → empty map).
pub fn field_map_from_json_str(fields_json: &str) -> Result<HashMap<String, TypeValue>, String> {
    if fields_json.is_empty() || fields_json == "{}" {
        return Ok(HashMap::new());
    }
    let json_val: Value =
        serde_json::from_str(fields_json).map_err(|e| format!("invalid fields JSON: {e}"))?;
    Ok(json_object_to_field_map(&json_val))
}

/// Convert a `serde_json::Value` to a `TypeValue`.
///
/// Conversion rules:
/// - `Null` → `TypeValue::Null`
/// - `Bool(b)` → `TypeValue::Boolean(b)`
/// - `Number(n)` → `TypeValue::Number` (tries i64, then u64, then f64)
/// - `String(s)` → `TypeValue::String(s)` — no silent date coercion
/// - `Array(arr)` → `TypeValue::Array` (recursive)
/// - `Object` with `"$type": "money"` + `"amount"` + `"currency"` → `TypeValue::Money`
/// - `Object` otherwise → `TypeValue::Object` (recursive)
///
/// Money detection requires an explicit `"$type": "money"` marker. Objects that
/// happen to have `amount` and `currency` fields but lack the marker are treated
/// as regular objects — no heuristic guessing.
///
/// The `amount` field accepts either a JSON number or a JSON string that parses
/// as a Decimal.
pub fn json_to_fel(val: &Value) -> TypeValue {
    match val {
        Value::Null => TypeValue::Null,
        Value::Bool(b) => TypeValue::Boolean(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                TypeValue::Number(Decimal::from(i))
            } else if let Some(u) = n.as_u64() {
                TypeValue::Number(Decimal::from(u))
            } else if let Some(f) = n.as_f64() {
                TypeValue::Number(Decimal::from_f64(f).unwrap_or(Decimal::ZERO))
            } else {
                TypeValue::Null
            }
        }
        Value::String(s) => TypeValue::String(s.clone()),
        Value::Array(arr) => TypeValue::Array(arr.iter().map(json_to_fel).collect()),
        Value::Object(map) => {
            let is_money_type = map
                .get("$type")
                .and_then(|v| v.as_str())
                .map(|s| s == "money")
                .unwrap_or(false);
            if is_money_type
                && let Some(currency) = map.get("currency").and_then(|v| v.as_str())
                && let Some(amount) = map.get("amount")
            {
                let maybe_decimal = match amount {
                    Value::Number(n) => n
                        .as_i64()
                        .map(Decimal::from)
                        .or_else(|| n.as_u64().map(Decimal::from))
                        .or_else(|| n.as_f64().and_then(Decimal::from_f64)),
                    Value::String(s) => Decimal::from_str_exact(s).ok(),
                    _ => None,
                };
                if let Some(amount_decimal) = maybe_decimal {
                    return TypeValue::Money(TypeMoney {
                        amount: amount_decimal,
                        currency: currency.to_string(),
                    });
                }
            }
            TypeValue::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), json_to_fel(v)))
                    .collect(),
            )
        }
    }
}

/// Convert a `TypeValue` to a `serde_json::Value`.
///
/// Conversion rules:
/// - `Null` → `Value::Null`
/// - `Boolean(b)` → `Value::Bool(b)`
/// - `Number(n)` → integer JSON number when whole, decimal string otherwise (precision-safe)
/// - `String(s)` → `Value::String(s)`
/// - `Date(d)` → `Value::String(d.format_iso())`
/// - `Money { amount, currency }` → `{"$type": "money", "amount": <number>, "currency": <string>}`
/// - `Array(arr)` → `Value::Array` (recursive)
/// - `Object(entries)` → `Value::Object` (recursive)
pub fn fel_to_json(val: &TypeValue) -> Value {
    match val {
        TypeValue::Null => Value::Null,
        TypeValue::Boolean(b) => Value::Bool(*b),
        TypeValue::Number(n) => {
            if n.fract().is_zero()
                && let Some(i) = n.to_i64()
            {
                return Value::Number(serde_json::Number::from(i));
            }
            Value::String(n.normalize().to_string())
        }
        TypeValue::String(s) => Value::String(s.clone()),
        TypeValue::Date(d) => Value::String(d.format_iso()),
        TypeValue::Array(arr) => Value::Array(arr.iter().map(fel_to_json).collect()),
        TypeValue::Object(entries) => {
            let map: serde_json::Map<String, Value> = entries
                .iter()
                .map(|(k, v)| (k.clone(), fel_to_json(v)))
                .collect();
            Value::Object(map)
        }
        TypeValue::Money(m) => {
            let mut map = serde_json::Map::new();
            map.insert("$type".to_string(), Value::String("money".to_string()));
            map.insert(
                "amount".to_string(),
                fel_to_json(&TypeValue::Number(m.amount)),
            );
            map.insert("currency".to_string(), Value::String(m.currency.clone()));
            Value::Object(map)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_docs_in_private_items)]
    use super::*;
    use serde_json::json;

    #[test]
    fn null_roundtrip() {
        let val = json_to_fel(&json!(null));
        assert!(matches!(val, TypeValue::Null));
        assert_eq!(fel_to_json(&val), json!(null));
    }

    #[test]
    fn boolean_roundtrip() {
        assert!(matches!(json_to_fel(&json!(true)), TypeValue::Boolean(true)));
        assert!(matches!(
            json_to_fel(&json!(false)),
            TypeValue::Boolean(false)
        ));
        assert_eq!(fel_to_json(&TypeValue::Boolean(true)), json!(true));
        assert_eq!(fel_to_json(&TypeValue::Boolean(false)), json!(false));
    }

    #[test]
    fn integer_roundtrip() {
        let val = json_to_fel(&json!(42));
        assert_eq!(fel_to_json(&val), json!(42));
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn float_roundtrip() {
        let val = json_to_fel(&json!(3.14));
        let back = fel_to_json(&val);
        assert_eq!(back, json!("3.14"));
    }

    #[test]
    fn string_roundtrip() {
        let val = json_to_fel(&json!("hello"));
        assert!(matches!(val, TypeValue::String(ref s) if s == "hello"));
        assert_eq!(fel_to_json(&val), json!("hello"));
    }

    #[test]
    fn string_no_date_coercion() {
        // ISO date strings must NOT be silently coerced to TypeValue::Date
        let val = json_to_fel(&json!("2024-06-15"));
        assert!(
            matches!(val, TypeValue::String(ref s) if s == "2024-06-15"),
            "expected String, got {val:?}"
        );

        let val = json_to_fel(&json!("2024-06-15T10:30:00"));
        assert!(
            matches!(val, TypeValue::String(ref s) if s == "2024-06-15T10:30:00"),
            "expected String, got {val:?}"
        );
    }

    #[test]
    fn array_roundtrip() {
        let val = json_to_fel(&json!([1, "two", null]));
        let back = fel_to_json(&val);
        assert_eq!(back, json!([1, "two", null]));
    }

    #[test]
    fn object_roundtrip() {
        let val = json_to_fel(&json!({"a": 1, "b": "two"}));
        let back = fel_to_json(&val);
        assert_eq!(back["a"], json!(1));
        assert_eq!(back["b"], json!("two"));
    }

    #[test]
    fn money_numeric_amount() {
        let val = json_to_fel(&json!({"$type": "money", "amount": 99.99, "currency": "USD"}));
        match &val {
            TypeValue::Money(m) => {
                assert_eq!(m.currency, "USD");
                let f = m.amount.to_f64().unwrap();
                assert!((f - 99.99).abs() < 0.01, "amount: {f}");
            }
            other => panic!("expected Money, got {other:?}"),
        }
    }

    #[test]
    fn money_string_amount() {
        let val = json_to_fel(&json!({"$type": "money", "amount": "99.99", "currency": "USD"}));
        match &val {
            TypeValue::Money(m) => {
                assert_eq!(m.currency, "USD");
                // String amount parsed as exact Decimal
                assert_eq!(m.amount, Decimal::from_str_exact("99.99").unwrap());
            }
            other => panic!("expected Money, got {other:?}"),
        }
    }

    #[test]
    fn money_integer_amount() {
        let val = json_to_fel(&json!({"$type": "money", "amount": 100, "currency": "EUR"}));
        match &val {
            TypeValue::Money(m) => {
                assert_eq!(m.currency, "EUR");
                assert_eq!(m.amount, Decimal::from(100));
            }
            other => panic!("expected Money, got {other:?}"),
        }
    }

    #[test]
    fn money_without_type_marker_is_object() {
        // Object with "amount" + "currency" but no "$type": "money" must NOT become Money
        let val = json_to_fel(&json!({"amount": 99.99, "currency": "USD"}));
        assert!(
            matches!(val, TypeValue::Object(_)),
            "expected Object, got {val:?}"
        );
    }

    #[test]
    fn money_roundtrip() {
        let money = TypeValue::Money(TypeMoney {
            amount: Decimal::from_str_exact("99.99").unwrap(),
            currency: "USD".to_string(),
        });
        let json = fel_to_json(&money);
        assert_eq!(json.get("$type"), Some(&json!("money")));
        assert_eq!(json.get("currency"), Some(&json!("USD")));
        assert_eq!(json.get("amount"), Some(&json!("99.99")));
    }

    #[test]
    fn money_missing_currency_becomes_object() {
        // Object with "amount" but no "currency" should NOT become Money
        let val = json_to_fel(&json!({"$type": "money", "amount": 100}));
        assert!(
            matches!(val, TypeValue::Object(_)),
            "expected Object, got {val:?}"
        );
    }

    #[test]
    fn money_non_numeric_amount_becomes_object() {
        // "amount" that isn't numeric or parseable as Decimal → plain Object
        let val = json_to_fel(&json!({"$type": "money", "amount": true, "currency": "USD"}));
        assert!(
            matches!(val, TypeValue::Object(_)),
            "expected Object, got {val:?}"
        );
    }

    #[test]
    fn date_to_json_iso_string() {
        use crate::types::Date;
        let date = TypeValue::Date(Date::Date {
            year: 2025,
            month: 6,
            day: 15,
        });
        assert_eq!(fel_to_json(&date), json!("2025-06-15"));
    }

    #[test]
    fn datetime_to_json_iso_string() {
        use crate::types::Date;
        let dt = TypeValue::Date(Date::DateTime {
            year: 2025,
            month: 6,
            day: 15,
            hour: 10,
            minute: 30,
            second: 0,
        });
        assert_eq!(fel_to_json(&dt), json!("2025-06-15T10:30:00"));
    }

    #[test]
    fn decimal_max_produces_number() {
        let val = TypeValue::Number(Decimal::MAX);
        let json = fel_to_json(&val);
        assert!(json.is_string(), "Decimal::MAX should produce a JSON string");
    }

    #[test]
    fn non_integer_decimal_keeps_full_precision_as_json_string() {
        let dec = Decimal::from_str_exact("0.1234567890123456789012345678").unwrap();
        let json = fel_to_json(&TypeValue::Number(dec));
        assert_eq!(json, json!("0.1234567890123456789012345678"));
    }

    #[test]
    fn nested_object_roundtrip() {
        let val = json_to_fel(&json!({"outer": {"inner": 42}}));
        let back = fel_to_json(&val);
        assert_eq!(back["outer"]["inner"], json!(42));
    }

    #[test]
    fn u64_large_number() {
        // A number larger than i64::MAX but within u64 range
        let big = (i64::MAX as u64) + 1;
        let val = json_to_fel(&json!(big));
        match &val {
            TypeValue::Number(n) => assert_eq!(*n, Decimal::from(big)),
            other => panic!("expected Number, got {other:?}"),
        }
    }
}
