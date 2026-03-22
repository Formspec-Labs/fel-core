**fel_core > convert**

# Module: convert

## Contents

**Functions**

- [`fel_to_json`](#fel_to_json) - Convert a `FelValue` to a `serde_json::Value`.
- [`field_map_from_json_str`](#field_map_from_json_str) - Parse a JSON object string into a field map (empty or `"{}"` → empty map).
- [`json_object_to_field_map`](#json_object_to_field_map) - JSON object → flat field map for FEL `MapEnvironment` (`{}` / empty → empty map).
- [`json_to_fel`](#json_to_fel) - Convert a `serde_json::Value` to a `FelValue`.

---

## fel_core::convert::fel_to_json

*Function*

Convert a `FelValue` to a `serde_json::Value`.

Conversion rules:
- `Null` → `Value::Null`
- `Boolean(b)` → `Value::Bool(b)`
- `Number(n)` → `Value::Number` (integer when whole, f64 otherwise)
- `String(s)` → `Value::String(s)`
- `Date(d)` → `Value::String(d.format_iso())`
- `Money { amount, currency }` → `{"$type": "money", "amount": <number>, "currency": <string>}`
- `Array(arr)` → `Value::Array` (recursive)
- `Object(entries)` → `Value::Object` (recursive)

```rust
fn fel_to_json(val: &crate::types::FelValue) -> serde_json::Value
```



## fel_core::convert::field_map_from_json_str

*Function*

Parse a JSON object string into a field map (empty or `"{}"` → empty map).

```rust
fn field_map_from_json_str(fields_json: &str) -> Result<std::collections::HashMap<String, crate::types::FelValue>, String>
```



## fel_core::convert::json_object_to_field_map

*Function*

JSON object → flat field map for FEL `MapEnvironment` (`{}` / empty → empty map).

```rust
fn json_object_to_field_map(val: &serde_json::Value) -> std::collections::HashMap<String, crate::types::FelValue>
```



## fel_core::convert::json_to_fel

*Function*

Convert a `serde_json::Value` to a `FelValue`.

Conversion rules:
- `Null` → `FelValue::Null`
- `Bool(b)` → `FelValue::Boolean(b)`
- `Number(n)` → `FelValue::Number` (tries i64, then u64, then f64)
- `String(s)` → `FelValue::String(s)` — no silent date coercion
- `Array(arr)` → `FelValue::Array` (recursive)
- `Object` with `"$type": "money"` + `"amount"` + `"currency"` → `FelValue::Money`
- `Object` otherwise → `FelValue::Object` (recursive)

Money detection requires an explicit `"$type": "money"` marker. Objects that
happen to have `amount` and `currency` fields but lack the marker are treated
as regular objects — no heuristic guessing.

The `amount` field accepts either a JSON number or a JSON string that parses
as a Decimal.

```rust
fn json_to_fel(val: &serde_json::Value) -> crate::types::FelValue
```



