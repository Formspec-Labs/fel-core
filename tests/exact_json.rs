//! Exact JSON transport preserves numeric tokens and distinguishes ordinary objects.
use fel_core::exact_json::{self, Value};

#[test]
fn numeric_tokens_round_trip_without_float_conversion() {
    for token in [
        "0.3333333333333333333333333333",
        "-0.1234567890123456789012345678",
        "1152921504606846976",
        "1e-28",
        "7.9228162514264337593543950335e28",
    ] {
        let value = exact_json::from_str(token).unwrap();
        assert!(value.is_number());
        assert_eq!(value.to_string(), token);
        let fel = exact_json::to_fel(&value).unwrap();
        assert_eq!(exact_json::from_fel(&fel).as_decimal(), value.as_decimal());
    }
}

#[test]
fn strings_and_marker_objects_remain_their_original_json_kinds() {
    for text in [
        r#""0.3333333333333333333333333333""#,
        r#"{"$type":"number","value":"1.2"}"#,
        r#"{"rawJSON":"1.2"}"#,
        r#"{"$serde_json::private::Number":"1.2"}"#,
        r#"{"$serde_json::private::RawValue":"1.2"}"#,
    ] {
        let value = exact_json::from_str(text).unwrap();
        assert!(!value.is_number());
        assert_eq!(value.to_string(), text);
        assert!(!matches!(
            exact_json::to_fel(&value).unwrap(),
            fel_core::Value::Number(_)
        ));
    }
}

#[test]
fn unsupported_decimal_arithmetic_is_explicit_and_never_rounded() {
    for token in [
        "1e100",
        "1e-100",
        "0.123456789012345678901234567891",
        "1.123456789012345678901234567891e0",
    ] {
        let value = exact_json::from_str(token).unwrap();
        assert_eq!(value.to_string(), token);
        assert!(
            exact_json::check_decimal_range(&value)
                .unwrap_err()
                .contains("precision or range")
        );
        assert!(exact_json::to_fel(&value).is_err());
    }
    for invalid in ["NaN", "Infinity", "01", "1.", "+1", "[", "true false"] {
        assert!(exact_json::from_str(invalid).is_err());
    }
}

#[test]
fn exact_values_serialize_inside_typed_serde_structs() {
    #[derive(serde::Serialize)]
    struct Response {
        data: Value,
    }
    let response = Response {
        data: exact_json::from_str(r#"{"ratio":0.3333333333333333333333333333}"#).unwrap(),
    };
    assert_eq!(
        serde_json::to_string(&response).unwrap(),
        r#"{"data":{"ratio":0.3333333333333333333333333333}}"#
    );
}

#[test]
fn exponent_notation_is_admitted_by_its_exact_value() {
    for (token, canonical) in [
        ("100000000000000000000000000000e-1", "1e28"),
        ("0.00000000000000000000000000001e1", "1e-28"),
        ("10e-29", "1e-28"),
        ("1.2300e2", "123"),
        ("-0e9999999999999999999999999", "0"),
        ("1e000000000000000000000000001", "10"),
    ] {
        let value = exact_json::from_str(token).unwrap();
        let expected = exact_json::from_str(canonical)
            .unwrap()
            .as_decimal()
            .unwrap();
        assert_eq!(value.as_decimal(), Some(expected), "{token}");
    }
    assert!(
        exact_json::from_str("79228162514264337593543950336")
            .unwrap()
            .as_decimal()
            .is_none()
    );
}
