/// FEL locale-aware function tests.
///
/// Covers the five FEL built-in functions for locale-aware expressions,
/// including cardinal plural rules for Arabic, Polish, French, and English.
///
/// These functions read from the FormspecEnvironment's locale and meta fields.
use fel_core::*;
use rust_decimal::Decimal;

fn num(n: i64) -> Value {
    Value::Number(Decimal::from(n))
}

fn s(v: &str) -> Value {
    Value::String(v.to_string())
}

fn eval_with_env(input: &str, env: &FormspecEnvironment) -> EvalResult {
    let expr = parse(input).unwrap();
    evaluate(&expr, env)
}

fn eval_value(input: &str, env: &FormspecEnvironment) -> Value {
    eval_with_env(input, env).value
}

// ── locale() ──────────────────────────────────────────────────────

#[test]
fn locale_returns_active_locale_string() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("fr-CA");
    assert_eq!(eval_value("locale()", &env), s("fr-CA"));
}

#[test]
fn locale_returns_null_when_not_set() {
    let env = FormspecEnvironment::new();
    assert_eq!(eval_value("locale()", &env), Value::Null);
}

#[test]
fn locale_returns_empty_string_when_set_empty() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("");
    // Empty string is a valid locale value (means "no locale selected")
    assert_eq!(eval_value("locale()", &env), s(""));
}

// ── runtimeMeta(key) ──────────────────────────────────────────────

#[test]
fn runtime_meta_returns_string_value() {
    let mut env = FormspecEnvironment::new();
    env.set_meta("gender", s("feminine"));
    assert_eq!(eval_value("runtimeMeta('gender')", &env), s("feminine"));
}

#[test]
fn runtime_meta_returns_number_value() {
    let mut env = FormspecEnvironment::new();
    env.set_meta("maxRetries", num(3));
    assert_eq!(eval_value("runtimeMeta('maxRetries')", &env), num(3));
}

#[test]
fn runtime_meta_returns_boolean_value() {
    let mut env = FormspecEnvironment::new();
    env.set_meta("isAdmin", Value::Boolean(true));
    assert_eq!(
        eval_value("runtimeMeta('isAdmin')", &env),
        Value::Boolean(true)
    );
}

#[test]
fn runtime_meta_returns_null_for_missing_key() {
    let env = FormspecEnvironment::new();
    assert_eq!(eval_value("runtimeMeta('missing')", &env), Value::Null);
}

#[test]
fn runtime_meta_null_propagation_on_null_key() {
    let env = FormspecEnvironment::new();
    // runtimeMeta(null) should return null
    assert_eq!(eval_value("runtimeMeta(null)", &env), Value::Null);
}

// ── pluralCategory(count, locale?) ────────────────────────────────

#[test]
fn plural_category_english_one() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("en");
    assert_eq!(eval_value("pluralCategory(1)", &env), s("one"));
}

#[test]
fn plural_category_english_other() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("en");
    assert_eq!(eval_value("pluralCategory(0)", &env), s("other"));
    assert_eq!(eval_value("pluralCategory(2)", &env), s("other"));
    assert_eq!(eval_value("pluralCategory(5)", &env), s("other"));
}

#[test]
fn plural_category_with_explicit_locale() {
    let env = FormspecEnvironment::new();
    // Explicit locale overrides the environment locale
    assert_eq!(eval_value("pluralCategory(1, 'en')", &env), s("one"));
    assert_eq!(eval_value("pluralCategory(2, 'en')", &env), s("other"));
}

#[test]
fn plural_category_arabic_zero() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("ar");
    assert_eq!(eval_value("pluralCategory(0)", &env), s("zero"));
}

#[test]
fn plural_category_arabic_one() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("ar");
    assert_eq!(eval_value("pluralCategory(1)", &env), s("one"));
}

#[test]
fn plural_category_arabic_two() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("ar");
    assert_eq!(eval_value("pluralCategory(2)", &env), s("two"));
}

#[test]
fn plural_category_arabic_few() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("ar");
    // Arabic "few" = 3-10
    assert_eq!(eval_value("pluralCategory(5)", &env), s("few"));
}

#[test]
fn plural_category_arabic_many() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("ar");
    // Arabic "many" = 11-99
    assert_eq!(eval_value("pluralCategory(15)", &env), s("many"));
}

#[test]
fn plural_category_polish_one() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("pl");
    assert_eq!(eval_value("pluralCategory(1)", &env), s("one"));
}

#[test]
fn plural_category_polish_few() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("pl");
    // Polish "few" = 2-4, 22-24, 32-34, ...
    assert_eq!(eval_value("pluralCategory(2)", &env), s("few"));
    assert_eq!(eval_value("pluralCategory(3)", &env), s("few"));
    assert_eq!(eval_value("pluralCategory(4)", &env), s("few"));
    assert_eq!(eval_value("pluralCategory(22)", &env), s("few"));
}

#[test]
fn plural_category_polish_many() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("pl");
    // Polish "many" = 0, 5-21, 25-31, ...
    assert_eq!(eval_value("pluralCategory(0)", &env), s("many"));
    assert_eq!(eval_value("pluralCategory(5)", &env), s("many"));
    assert_eq!(eval_value("pluralCategory(12)", &env), s("many"));
}

#[test]
fn plural_category_french_one() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("fr");
    // French: 0 and 1 are "one"
    assert_eq!(eval_value("pluralCategory(0)", &env), s("one"));
    assert_eq!(eval_value("pluralCategory(1)", &env), s("one"));
}

#[test]
fn plural_category_french_other() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("fr");
    assert_eq!(eval_value("pluralCategory(2)", &env), s("other"));
}

/// `pt-PT` and `pt-BR`: per `intl_pluralrules` CLDR cardinal data, 0 is `other`; only `1` is `one`.
#[test]
fn plural_category_portuguese_portugal_vs_brazil() {
    let env = FormspecEnvironment::new();
    assert_eq!(eval_value("pluralCategory(0, 'pt-PT')", &env), s("other"));
    assert_eq!(eval_value("pluralCategory(1, 'pt-PT')", &env), s("one"));
    assert_eq!(eval_value("pluralCategory(2, 'pt-PT')", &env), s("other"));

    assert_eq!(eval_value("pluralCategory(0, 'pt-BR')", &env), s("other"));
    assert_eq!(eval_value("pluralCategory(1, 'pt-BR')", &env), s("one"));
    assert_eq!(eval_value("pluralCategory(2, 'pt-BR')", &env), s("other"));
}

/// Bare `pt` uses CLDR rules for the language subtag alone (0–1 → `one` in current data).
#[test]
fn plural_category_portuguese_language_tag_without_region() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("pt");
    assert_eq!(eval_value("pluralCategory(0)", &env), s("one"));
    assert_eq!(eval_value("pluralCategory(1)", &env), s("one"));
}

#[test]
fn plural_category_turkish_one_and_other() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("tr");
    assert_eq!(eval_value("pluralCategory(1)", &env), s("one"));
    assert_eq!(eval_value("pluralCategory(2)", &env), s("other"));
}

/// Fractional counts must not fall through `to_i64()` as 0 (would mis-classify, e.g. Arabic "zero").
#[test]
fn plural_category_truncates_fractional_count() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("ar");
    assert_eq!(eval_value("pluralCategory(1.5)", &env), s("one"));
    assert_eq!(eval_value("pluralCategory(2.9)", &env), s("two"));

    env.set_locale("en");
    assert_eq!(eval_value("pluralCategory(1.5)", &env), s("one"));
    assert_eq!(eval_value("pluralCategory(2.1)", &env), s("other"));
}

#[test]
fn plural_category_null_propagation() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("en");
    assert_eq!(eval_value("pluralCategory(null)", &env), Value::Null);
}

#[test]
fn plural_category_no_locale_returns_null() {
    // No locale set and no explicit locale param — return null
    let env = FormspecEnvironment::new();
    assert_eq!(eval_value("pluralCategory(1)", &env), Value::Null);
}

// ── formatNumber() ────────────────────────────────────────────────

#[test]
fn format_number_en_grouping() {
    let mut env = FormspecEnvironment::new();
    env.set_locale("en");
    assert_eq!(eval_value("formatNumber(1234.5)", &env), s("1,234.5"));
    assert_eq!(eval_value("formatNumber(1234.5, 'en')", &env), s("1,234.5"));
}

#[test]
fn format_number_fr_differs_from_en() {
    let env = FormspecEnvironment::new();
    let en = eval_value("formatNumber(1234.5, 'en')", &env);
    let fr = eval_value("formatNumber(1234.5, 'fr')", &env);
    assert_eq!(en, s("1,234.5"));
    assert_eq!(fr, s("1 234,5"));
    assert_ne!(en, fr);
}

#[test]
fn format_number_null_propagation() {
    let env = FormspecEnvironment::new();
    assert_eq!(eval_value("formatNumber(null)", &env), Value::Null);
}

// ── formatDate() ──────────────────────────────────────────────────

#[test]
fn format_date_medium_en() {
    let env = FormspecEnvironment::new();
    assert_eq!(
        eval_value("formatDate('2026-05-17', 'medium', 'en')", &env),
        s("May 17, 2026")
    );
}

#[test]
fn format_date_short_fr_differs_from_en() {
    let env = FormspecEnvironment::new();
    let en = eval_value("formatDate('2026-05-17', 'short', 'en')", &env);
    let fr = eval_value("formatDate('2026-05-17', 'short', 'fr')", &env);
    assert_eq!(en, s("5/17/26"));
    assert_eq!(fr, s("17/05/26"));
    assert_ne!(en, fr);
}

#[test]
fn format_date_accepts_fel_date_literal() {
    let env = FormspecEnvironment::new();
    assert_eq!(
        eval_value("formatDate(@2026-05-17, 'medium', 'en')", &env),
        s("May 17, 2026")
    );
}

#[test]
fn format_date_null_propagation() {
    let env = FormspecEnvironment::new();
    assert_eq!(eval_value("formatDate(null)", &env), Value::Null);
}

#[test]
fn format_date_medium_fr_differs_from_en() {
    let env = FormspecEnvironment::new();
    let en = eval_value("formatDate('@2026-05-17', 'medium', 'en')", &env);
    let fr = eval_value("formatDate('@2026-05-17', 'medium', 'fr')", &env);
    assert_eq!(en, s("May 17, 2026"));
    assert_eq!(fr, s("mai 17, 2026"));
    assert_ne!(en, fr);
}

#[test]
fn format_number_rejects_non_number() {
    let env = FormspecEnvironment::new();
    let result = eval_with_env("formatNumber('x')", &env);
    assert_eq!(result.value, Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("number"))
    );
}

#[test]
fn format_number_negative_en() {
    let env = FormspecEnvironment::new();
    assert_eq!(
        eval_value("formatNumber(-1234.5, 'en')", &env),
        s("-1,234.5")
    );
}

#[test]
fn format_date_rejects_invalid_string() {
    let env = FormspecEnvironment::new();
    let result = eval_with_env("formatDate('not-a-date')", &env);
    assert_eq!(result.value, Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("invalid"))
    );
}

#[test]
fn format_date_two_arg_locale_only() {
    let env = FormspecEnvironment::new();
    assert_eq!(
        eval_value("formatDate('@2026-05-17', 'fr')", &env),
        s("mai 17, 2026")
    );
}

#[test]
fn builtin_catalog_includes_format_number() {
    let catalog = builtin_function_catalog();
    assert!(
        catalog.iter().any(|e| e.name == "formatNumber"),
        "builtin catalog should include formatNumber()"
    );
}

#[test]
fn builtin_catalog_includes_format_date() {
    let catalog = builtin_function_catalog();
    assert!(
        catalog.iter().any(|e| e.name == "formatDate"),
        "builtin catalog should include formatDate()"
    );
}

// ── context_json: locale and meta from JSON ───────────────────────

#[test]
fn context_json_parses_locale() {
    let ctx = serde_json::json!({
        "locale": "fr-CA",
        "fields": {}
    });
    let env = formspec_environment_from_json_map(ctx.as_object().unwrap());
    assert_eq!(env.locale.as_deref(), Some("fr-CA"));
}

#[test]
fn context_json_parses_meta() {
    let ctx = serde_json::json!({
        "meta": { "gender": "feminine", "retries": 3 },
        "fields": {}
    });
    let env = formspec_environment_from_json_map(ctx.as_object().unwrap());
    assert_eq!(env.meta.get("gender"), Some(&s("feminine")));
    assert_eq!(env.meta.get("retries"), Some(&num(3)));
}

// ── builtin catalog includes new functions ────────────────────────

#[test]
fn builtin_catalog_includes_locale() {
    let catalog = builtin_function_catalog();
    assert!(
        catalog.iter().any(|e| e.name == "locale"),
        "builtin catalog should include locale()"
    );
}

#[test]
fn builtin_catalog_includes_runtime_meta() {
    let catalog = builtin_function_catalog();
    assert!(
        catalog.iter().any(|e| e.name == "runtimeMeta"),
        "builtin catalog should include runtimeMeta()"
    );
}

#[test]
fn builtin_catalog_includes_plural_category() {
    let catalog = builtin_function_catalog();
    assert!(
        catalog.iter().any(|e| e.name == "pluralCategory"),
        "builtin catalog should include pluralCategory()"
    );
}
