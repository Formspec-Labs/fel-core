#![allow(dead_code)]

use fel_core::*;
use indexmap::IndexMap;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

pub fn eval(input: &str) -> Value {
    let expr = parse(input).unwrap();
    let env = MapEnvironment::new();
    evaluate(&expr, &env).value
}

pub fn eval_fields(input: &str, fields: Vec<(&str, Value)>) -> Value {
    eval_fields_result(input, fields).value
}

pub fn eval_fields_result(input: &str, fields: Vec<(&str, Value)>) -> EvalResult {
    let expr = parse(input).unwrap();
    let env = MapEnvironment::with_fields(
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    );
    evaluate(&expr, &env)
}

pub fn num(n: impl Into<Decimal>) -> Value {
    Value::Number(n.into())
}

pub fn dec(v: &str) -> Value {
    Value::Number(Decimal::from_str(v).unwrap())
}

pub fn s(v: &str) -> Value {
    Value::String(v.to_string())
}

pub fn arr(vals: Vec<Value>) -> Value {
    Value::Array(vals)
}

/// Build an object value with insertion order preserved.
pub fn obj(pairs: Vec<(String, Value)>) -> Value {
    Value::Object(pairs.into_iter().collect::<IndexMap<_, _>>())
}
