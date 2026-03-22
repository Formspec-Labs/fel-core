**fel_core > types**

# Module: types

## Contents

**Structs**

- [`FelMoney`](#felmoney) - Monetary value with ISO currency code.

**Enums**

- [`FelDate`](#feldate) - Calendar date or date-time (no timezone model; used by date functions).
- [`FelValue`](#felvalue) - Runtime value for FEL evaluation (mirrors JSON + dates + money).

**Functions**

- [`civil_from_days_pub`](#civil_from_days_pub) - Convert days since the internal FEL epoch (2000-01-01) to a [`FelDate`] (date-only).
- [`date_add_days`](#date_add_days) - Add days to a date.
- [`days_in_month`](#days_in_month) - Gregorian days in `month` for `year` (validates `month` in debug only).
- [`format_number`](#format_number) - Format a Decimal: strip trailing zeros, show as integer when possible.
- [`parse_date_literal`](#parse_date_literal) - Parse "@YYYY-MM-DD" into FelDate.
- [`parse_datetime_literal`](#parse_datetime_literal) - Parse "@YYYY-MM-DDTHH:MM:SS..." into FelDate.

---

## fel_core::types::FelDate

*Enum*

Calendar date or date-time (no timezone model; used by date functions).

**Variants:**
- `Date{ year: i32, month: u32, day: u32 }`
- `DateTime{ year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32 }`

**Methods:**

- `fn year(self: &Self) -> i32` - Calendar year component.
- `fn month(self: &Self) -> u32` - Month 1–12.
- `fn day(self: &Self) -> u32` - Day of month.
- `fn to_naive_date(self: &Self) -> (i32, u32, u32)` - `(year, month, day)` tuple.
- `fn ordinal_days(self: &Self) -> i64` - Days since epoch (2000-01-01) for ordering.
- `fn ordinal(self: &Self) -> i64` - Full ordinal including time (seconds from epoch) for DateTime ordering.
- `fn format_iso(self: &Self) -> String` - `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS` (no timezone suffix).

**Traits:** Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> FelDate`
- **PartialEq**
  - `fn eq(self: &Self, other: &FelDate) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::types::FelMoney

*Struct*

Monetary value with ISO currency code.

**Fields:**
- `amount: rust_decimal::Decimal` - Decimal amount (base-10).
- `currency: String` - ISO 4217 currency code (e.g. `USD`).

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Self) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> FelMoney`



## fel_core::types::FelValue

*Enum*

Runtime value for FEL evaluation (mirrors JSON + dates + money).

**Variants:**
- `Null`
- `Boolean(bool)`
- `Number(rust_decimal::Decimal)`
- `String(String)`
- `Date(FelDate)`
- `Array(Vec<FelValue>)`
- `Object(Vec<(String, FelValue)>)`
- `Money(FelMoney)`

**Methods:**

- `fn type_name(self: &Self) -> &'static str` - Lowercase FEL type name for error messages.
- `fn is_null(self: &Self) -> bool` - True only for [`FelValue::Null`].
- `fn is_truthy(self: &Self) -> bool` - Loose truth test (not FEL `and`/`or` typing — used by some builtins).
- `fn as_number(self: &Self) -> Option<Decimal>` - Extract number or `None`.
- `fn as_string(self: &Self) -> Option<&str>` - Borrow string or `None`.
- `fn as_bool(self: &Self) -> Option<bool>` - Extract boolean or `None`.
- `fn as_date(self: &Self) -> Option<&FelDate>` - Borrow date/datetime or `None`.
- `fn as_array(self: &Self) -> Option<&Vec<FelValue>>` - Borrow array or `None`.
- `fn as_money(self: &Self) -> Option<&FelMoney>` - Borrow money or `None`.

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Self) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> FelValue`



## fel_core::types::civil_from_days_pub

*Function*

Convert days since the internal FEL epoch (2000-01-01) to a [`FelDate`] (date-only).

```rust
fn civil_from_days_pub(z: i64) -> FelDate
```



## fel_core::types::date_add_days

*Function*

Add days to a date.

```rust
fn date_add_days(d: &FelDate, n: i64) -> FelDate
```



## fel_core::types::days_in_month

*Function*

Gregorian days in `month` for `year` (validates `month` in debug only).

```rust
fn days_in_month(year: i32, month: u32) -> u32
```



## fel_core::types::format_number

*Function*

Format a Decimal: strip trailing zeros, show as integer when possible.

```rust
fn format_number(n: rust_decimal::Decimal) -> String
```



## fel_core::types::parse_date_literal

*Function*

Parse "@YYYY-MM-DD" into FelDate.

```rust
fn parse_date_literal(s: &str) -> Option<FelDate>
```



## fel_core::types::parse_datetime_literal

*Function*

Parse "@YYYY-MM-DDTHH:MM:SS..." into FelDate.

```rust
fn parse_datetime_literal(s: &str) -> Option<FelDate>
```



