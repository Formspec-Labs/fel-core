//! JSON values with exact numeric tokens and no object-shaped numeric markers.
//!
//! Serde owns JSON syntax and escaping. This type keeps validated numeric text
//! until decimal arithmetic or serialization, without changing serde_json's
//! feature behavior for other crates. Use text input to retain original digits;
//! conversion from an already parsed serde_json value cannot recover lost digits.

use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;
use std::{fmt, ops, str::FromStr};

/// An insertion-ordered JSON object.
pub type Map<K, V> = indexmap::IndexMap<K, V>;

/// A validated JSON number, retaining its original decimal digits.
#[derive(Clone, Debug)]
pub struct Number(Box<RawValue>);

impl Number {
    /// The original JSON number token.
    pub fn as_str(&self) -> &str {
        self.0.get()
    }

    /// An exact decimal, or `None` outside the evaluator's precision and range.
    pub fn as_decimal(&self) -> Option<Decimal> {
        let text = self.as_str();
        let (base, exponent) = text.split_once(['e', 'E']).unwrap_or((text, "0"));
        let negative = base.starts_with('-');
        let fractional = base
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len());
        let digits: String = base.chars().filter(char::is_ascii_digit).collect();
        let significant = digits.trim_start_matches('0');
        // Zero is representable regardless of its exponent spelling.
        if significant.is_empty() {
            return Some(Decimal::ZERO);
        }
        let coefficient = significant.trim_end_matches('0');
        if coefficient.len() > 29 {
            return None;
        }
        let power = exponent
            .parse::<i64>()
            .ok()?
            .checked_sub(i64::try_from(fractional).ok()?)?
            .checked_add(i64::try_from(significant.len() - coefficient.len()).ok()?)?;
        // Bound expansion before allocating. Decimal supports 96-bit coefficients and scale <= 28.
        let normalized = if power >= 0 {
            if power > 29 || coefficient.len() + power as usize > 29 {
                return None;
            }
            format!("{}{}", coefficient, "0".repeat(power as usize))
        } else {
            let scale = power.checked_neg()?;
            if scale > 28 {
                return None;
            }
            let scale = scale as usize;
            if scale >= coefficient.len() {
                format!("0.{}{}", "0".repeat(scale - coefficient.len()), coefficient)
            } else {
                let split = coefficient.len() - scale;
                format!("{}.{}", &coefficient[..split], &coefficient[split..])
            }
        };
        let signed = if negative {
            format!("-{normalized}")
        } else {
            normalized
        };
        Decimal::from_str_exact(&signed).ok()
    }

    /// An exact signed integer when it fits in i64.
    pub fn as_i64(&self) -> Option<i64> {
        self.as_decimal().filter(|v| v.fract().is_zero())?.to_i64()
    }
    /// An exact unsigned integer when it fits in u64.
    pub fn as_u64(&self) -> Option<u64> {
        self.as_decimal().filter(|v| v.fract().is_zero())?.to_u64()
    }
    /// A finite binary floating-point approximation for display or legacy callers.
    pub fn as_f64(&self) -> Option<f64> {
        self.as_str().parse::<f64>().ok().filter(|v| v.is_finite())
    }
    /// Whether this number is an i64.
    pub fn is_i64(&self) -> bool {
        self.as_i64().is_some()
    }
    /// Whether this number is a u64.
    pub fn is_u64(&self) -> bool {
        self.as_u64().is_some()
    }
}

impl FromStr for Number {
    type Err = serde_json::Error;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let raw: Box<RawValue> = serde_json::from_str(text)?;
        if !matches!(raw.get().as_bytes().first(), Some(b'-' | b'0'..=b'9')) {
            return Err(<serde_json::Error as serde::de::Error>::custom(
                "expected a JSON number",
            ));
        }
        Ok(Self(raw))
    }
}
impl From<Decimal> for Number {
    fn from(value: Decimal) -> Self {
        value
            .normalize()
            .to_string()
            .parse()
            .expect("Decimal emits a valid JSON number")
    }
}
impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_decimal(), other.as_decimal()) {
            (Some(a), Some(b)) => a == b,
            _ => self.as_str() == other.as_str(),
        }
    }
}
impl Eq for Number {}
impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl Serialize for Number {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// JSON's six value kinds; a number is never an ordinary object or string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Value {
    /// JSON null.
    #[default]
    Null,
    /// JSON boolean.
    Bool(bool),
    /// An exact JSON number.
    Number(Number),
    /// JSON string.
    String(String),
    /// JSON array.
    Array(Vec<Value>),
    /// JSON object, including objects that resemble implementation markers.
    Object(Map<String, Value>),
}

impl Value {
    /// A named object member.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_object()?.get(key)
    }
    /// A mutable named object member.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.as_object_mut()?.get_mut(key)
    }
    /// The object members, if this is an object.
    pub fn as_object(&self) -> Option<&Map<String, Value>> {
        if let Self::Object(v) = self {
            Some(v)
        } else {
            None
        }
    }
    /// The mutable object members, if this is an object.
    pub fn as_object_mut(&mut self) -> Option<&mut Map<String, Value>> {
        if let Self::Object(v) = self {
            Some(v)
        } else {
            None
        }
    }
    /// The array elements, if this is an array.
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        if let Self::Array(v) = self {
            Some(v)
        } else {
            None
        }
    }
    /// The mutable array elements, if this is an array.
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        if let Self::Array(v) = self {
            Some(v)
        } else {
            None
        }
    }
    /// The string contents, if this is a string.
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(v) = self {
            Some(v)
        } else {
            None
        }
    }
    /// The boolean, if this is a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    /// The exact decimal, if supported by the evaluator.
    pub fn as_decimal(&self) -> Option<Decimal> {
        if let Self::Number(v) = self {
            v.as_decimal()
        } else {
            None
        }
    }
    /// An exact signed integer, if available.
    pub fn as_i64(&self) -> Option<i64> {
        if let Self::Number(v) = self {
            v.as_i64()
        } else {
            None
        }
    }
    /// An exact unsigned integer, if available.
    pub fn as_u64(&self) -> Option<u64> {
        if let Self::Number(v) = self {
            v.as_u64()
        } else {
            None
        }
    }
    /// A finite binary approximation, if this is a number.
    pub fn as_f64(&self) -> Option<f64> {
        if let Self::Number(v) = self {
            v.as_f64()
        } else {
            None
        }
    }
    /// Whether this is null.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
    /// Whether this is an object.
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }
    /// Whether this is an array.
    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }
    /// Whether this is a string.
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }
    /// Whether this is a number.
    pub fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }
    /// Whether this value fits a signed integer.
    pub fn is_i64(&self) -> bool {
        self.as_i64().is_some()
    }
    /// Whether this value fits an unsigned integer.
    pub fn is_u64(&self) -> bool {
        self.as_u64().is_some()
    }
    /// Whether this is a boolean.
    pub fn is_boolean(&self) -> bool {
        matches!(self, Self::Bool(_))
    }
    /// Find a mutable value by RFC 6901 JSON Pointer.
    pub fn pointer_mut(&mut self, pointer: &str) -> Option<&mut Self> {
        if pointer.is_empty() {
            return Some(self);
        }
        if !pointer.starts_with('/') {
            return None;
        }
        pointer.split('/').skip(1).try_fold(self, |value, token| {
            let key = token.replace("~1", "/").replace("~0", "~");
            match value {
                Self::Object(map) => map.get_mut(&key),
                Self::Array(array)
                    if !key.starts_with('+') && (key == "0" || !key.starts_with('0')) =>
                {
                    array.get_mut(key.parse::<usize>().ok()?)
                }
                _ => None,
            }
        })
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(v) => v.serialize(serializer),
            Self::Number(v) => v.serialize(serializer),
            Self::String(v) => v.serialize(serializer),
            Self::Array(v) => v.serialize(serializer),
            Self::Object(v) => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(v.len()))?;
                for (key, value) in v {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Box::<RawValue>::deserialize(deserializer)?;
        parse_raw(&raw, 0).map_err(serde::de::Error::custom)
    }
}

/// Parse JSON through serde, preserving number tokens and object identity.
///
/// # Errors
/// Rejects malformed JSON and nesting deeper than serde_json's default limit.
pub fn from_str(text: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str(text)
}

fn parse_raw(raw: &RawValue, depth: usize) -> Result<Value, serde_json::Error> {
    if depth > 128 {
        return Err(<serde_json::Error as serde::de::Error>::custom(
            "JSON nesting limit exceeded",
        ));
    }
    Ok(match raw.get().as_bytes()[0] {
        b'{' => {
            let members: Map<String, Box<RawValue>> = serde_json::from_str(raw.get())?;
            Value::Object(
                members
                    .into_iter()
                    .map(|(key, raw)| Ok((key, parse_raw(&raw, depth + 1)?)))
                    .collect::<Result<_, serde_json::Error>>()?,
            )
        }
        b'[' => {
            let elements: Vec<Box<RawValue>> = serde_json::from_str(raw.get())?;
            Value::Array(
                elements
                    .iter()
                    .map(|raw| parse_raw(raw, depth + 1))
                    .collect::<Result<_, _>>()?,
            )
        }
        b'"' => Value::String(serde_json::from_str(raw.get())?),
        b't' | b'f' => Value::Bool(serde_json::from_str(raw.get())?),
        b'n' => Value::Null,
        _ => Value::Number(raw.get().parse()?),
    })
}

/// Convert an already parsed value, preserving only the digits it still contains.
impl From<serde_json::Value> for Value {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(v) => Self::Bool(v),
            serde_json::Value::Number(v) => {
                Self::Number(v.to_string().parse().expect("serde emits valid numbers"))
            }
            serde_json::Value::String(v) => Self::String(v),
            serde_json::Value::Array(v) => Self::Array(v.into_iter().map(Self::from).collect()),
            serde_json::Value::Object(v) => {
                Self::Object(v.into_iter().map(|(k, v)| (k, Self::from(v))).collect())
            }
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&serde_json::to_string(self).map_err(|_| fmt::Error)?)
    }
}
impl ops::Index<&str> for Value {
    type Output = Value;
    fn index(&self, key: &str) -> &Self::Output {
        self.get(key).unwrap_or(&Value::Null)
    }
}
impl ops::Index<usize> for Value {
    type Output = Value;
    fn index(&self, key: usize) -> &Self::Output {
        self.as_array()
            .and_then(|v| v.get(key))
            .unwrap_or(&Value::Null)
    }
}
impl ops::Index<&String> for Value {
    type Output = Value;
    fn index(&self, key: &String) -> &Self::Output {
        &self[key.as_str()]
    }
}
impl ops::IndexMut<&str> for Value {
    fn index_mut(&mut self, key: &str) -> &mut Self::Output {
        if self.is_null() {
            *self = Value::Object(Map::new());
        }
        self.as_object_mut()
            .expect("indexed value must be an object")
            .entry(key.to_string())
            .or_default()
    }
}
impl ops::IndexMut<usize> for Value {
    fn index_mut(&mut self, key: usize) -> &mut Self::Output {
        &mut self.as_array_mut().expect("indexed value must be an array")[key]
    }
}
impl PartialEq<serde_json::Value> for Value {
    fn eq(&self, other: &serde_json::Value) -> bool {
        self == &Self::from(other.clone())
    }
}
impl PartialEq<&str> for Value {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == Some(*other)
    }
}
impl PartialEq<Value> for serde_json::Value {
    fn eq(&self, other: &Value) -> bool {
        other == self
    }
}

/// Serialize a value directly into an exact JSON tree.
///
/// # Errors
/// Returns serialization errors, including unsupported map keys.
pub fn to_value<T: Serialize + ?Sized>(value: &T) -> Result<Value, serde_json::Error> {
    from_str(&serde_json::to_string(value)?)
}

/// Deserialize an exact tree through JSON text, preserving numeric tokens.
///
/// # Errors
/// Returns errors when the target cannot represent the JSON value.
pub fn from_value<T: serde::de::DeserializeOwned>(
    value: impl Into<Value>,
) -> Result<T, serde_json::Error> {
    serde_json::from_str(&value.into().to_string())
}

/// Build exact JSON from ordinary Rust JSON literals.
///
/// Numeric Rust literals have already been interpreted by Rust. Use `from_str`
/// for source JSON and `to_value` for values containing exact numeric leaves.
#[macro_export]
macro_rules! native_json {
    ($($token:tt)+) => { $crate::exact_json::Value::from(serde_json::json!($($token)+)) };
}
pub use crate::native_json;

/// Encode FEL values as schema-compliant JSON, with exact numbers and string money amounts.
pub fn from_fel(value: &crate::Value) -> Value {
    match value {
        crate::Value::Null => Value::Null,
        crate::Value::Boolean(v) => Value::Bool(*v),
        crate::Value::Number(v) => Value::Number((*v).into()),
        crate::Value::String(v) => Value::String(v.clone()),
        crate::Value::Date(v) => Value::String(v.format_iso()),
        crate::Value::Array(v) => Value::Array(v.iter().map(from_fel).collect()),
        crate::Value::Object(v) => {
            Value::Object(v.iter().map(|(k, v)| (k.clone(), from_fel(v))).collect())
        }
        crate::Value::Money(v) => Value::Object(
            [
                (
                    "amount".into(),
                    Value::String(v.amount.normalize().to_string()),
                ),
                ("currency".into(), Value::String(v.currency.to_string())),
            ]
            .into_iter()
            .collect(),
        ),
    }
}

/// Decode ordinary JSON without interpreting any object as a typed wire marker.
///
/// # Errors
/// Rejects numbers outside the evaluator's decimal precision or range.
pub fn to_fel(value: &Value) -> Result<crate::Value, String> {
    Ok(match value {
        Value::Null => crate::Value::Null,
        Value::Bool(v) => crate::Value::Boolean(*v),
        Value::Number(v) => crate::Value::Number(
            v.as_decimal()
                .ok_or_else(|| format!("JSON number {v} exceeds FEL decimal precision or range"))?,
        ),
        Value::String(v) => crate::Value::String(v.clone()),
        Value::Array(v) => crate::Value::Array(v.iter().map(to_fel).collect::<Result<_, _>>()?),
        Value::Object(v) => crate::Value::Object(
            v.iter()
                .map(|(k, v)| Ok((k.clone(), to_fel(v)?)))
                .collect::<Result<_, String>>()?,
        ),
    })
}

/// Check arithmetic range without allocating a second tree of FEL values.
///
/// # Errors
/// Rejects any numeric token outside FEL's decimal precision or range.
pub fn check_decimal_range(value: &Value) -> Result<(), String> {
    match value {
        Value::Number(number) if number.as_decimal().is_none() => Err(format!(
            "JSON number {number} exceeds FEL decimal precision or range"
        )),
        Value::Array(values) => values.iter().try_for_each(check_decimal_range),
        Value::Object(values) => values.values().try_for_each(check_decimal_range),
        _ => Ok(()),
    }
}

/// Adapt exact tokens to FEL's explicitly tagged host protocol.
///
/// Ordinary response data must use [`to_fel`] instead: this protocol deliberately
/// recognizes FEL number, date, and money markers supplied by a host.
pub fn to_fel_wire(value: &Value) -> crate::Value {
    fn wire(value: &Value) -> serde_json::Value {
        match value {
            Value::Number(number) => {
                serde_json::json!({"$type": "number", "value": number.as_decimal().map(|n| n.to_string()).unwrap_or_else(|| number.to_string())})
            }
            Value::Array(values) => serde_json::Value::Array(values.iter().map(wire).collect()),
            Value::Object(values) => {
                let tag = values.get("$type").and_then(Value::as_str);
                serde_json::Value::Object(
                    values
                        .iter()
                        .map(|(key, value)| {
                            let numeric_slot = matches!(
                                (tag, key.as_str()),
                                (Some("number"), "value") | (Some("money"), "amount")
                            );
                            let value = if numeric_slot {
                                value
                                    .as_decimal()
                                    .map(|n| serde_json::Value::String(n.to_string()))
                                    .unwrap_or_else(|| wire(value))
                            } else {
                                wire(value)
                            };
                            (key.clone(), value)
                        })
                        .collect(),
                )
            }
            Value::Null => serde_json::Value::Null,
            Value::Bool(v) => serde_json::Value::Bool(*v),
            Value::String(v) => serde_json::Value::String(v.clone()),
        }
    }
    crate::json_to_fel(&wire(value))
}

/// Structural JSON access shared by definition walkers and bind merging.
///
/// This keeps those algorithms independent of numeric storage. The exact and
/// serde_json trees share traversal, without converting their numeric leaves.
pub trait JsonTree: Clone {
    /// The object's mutable collection type.
    type Object: Default + Extend<(String, Self)>;
    /// A named member of an object.
    fn get(&self, key: &str) -> Option<&Self>;
    /// A string's contents.
    fn as_str(&self) -> Option<&str>;
    /// An array's elements.
    fn as_array(&self) -> Option<&Vec<Self>>;
    /// Whether this value is an object.
    fn is_object(&self) -> bool;
    /// Object members in document order; empty for other kinds.
    fn members(&self) -> impl Iterator<Item = (&String, &Self)>;
}

impl JsonTree for Value {
    type Object = Map<String, Self>;
    fn get(&self, key: &str) -> Option<&Self> {
        self.get(key)
    }
    fn as_str(&self) -> Option<&str> {
        self.as_str()
    }
    fn as_array(&self) -> Option<&Vec<Self>> {
        self.as_array()
    }
    fn is_object(&self) -> bool {
        self.is_object()
    }
    fn members(&self) -> impl Iterator<Item = (&String, &Self)> {
        self.as_object().into_iter().flatten()
    }
}
impl JsonTree for serde_json::Value {
    type Object = serde_json::Map<String, Self>;
    fn get(&self, key: &str) -> Option<&Self> {
        self.get(key)
    }
    fn as_str(&self) -> Option<&str> {
        self.as_str()
    }
    fn as_array(&self) -> Option<&Vec<Self>> {
        self.as_array()
    }
    fn is_object(&self) -> bool {
        self.is_object()
    }
    fn members(&self) -> impl Iterator<Item = (&String, &Self)> {
        self.as_object().into_iter().flatten()
    }
}
