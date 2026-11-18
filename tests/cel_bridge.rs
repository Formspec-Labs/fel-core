//! Exercises the public source-conversion API and command.
// Rust guideline compliant 2026-02-21
#![cfg(feature = "cel-bridge")]

use fel_core::cel::{InputType, Language, NativeOptions, convert, convert_literal, convert_native};
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn editing_source_round_trips_form_and_cel_only_constructs() {
    let source = "money($amount, 'USD')";
    let cel = convert(source, Language::Fel).unwrap();
    assert!(cel.starts_with("// cel-fel-syntax v1 from FEL\n"));
    let edited = convert(&cel.replace("USD", "GBP"), Language::Cel).unwrap();
    assert_eq!(
        fel_core::parse(&edited).unwrap(),
        fel_core::parse("money($amount, 'GBP')").unwrap()
    );
    let fel = convert(r#"b"\x00\xff""#, Language::Cel).unwrap();
    assert_eq!(convert(&fel, Language::Fel).unwrap(), r#"b"\x00\xff""#);
    let marker = "// cel-fel-syntax v1 from CEL\ntrue";
    assert!(convert(marker, Language::Cel).is_err());
    assert!(convert_literal(marker, Language::Cel).is_ok());
}

#[test]
fn reopens_v1_operator_chains_and_rejects_invalid_edits() {
    let restore = |body| {
        convert(
            &format!("// cel-fel-syntax v1 from CEL\n{body}"),
            Language::Fel,
        )
    };
    let old = "bridge_cel_and($a, bridge_cel_or($b, $c, $d), bridge_cel_prefix('!!', $ready))";
    let same = "a && (b || c || d) && !!ready";
    assert_eq!(
        convert(&restore(old).unwrap(), Language::Cel),
        convert(same, Language::Cel)
    );
    for bad in [
        "bridge_cel_and($a)",
        "bridge_cel_prefix('', $a)",
        "bridge_cel_and(bridge_cel_double('true'), $b, $c)",
        "bridge_cel_prefix('--', bridge_cel_uint('1'))",
    ] {
        assert!(restore(bad).is_err(), "{bad}");
    }
}

#[test]
fn native_conversion_preserves_both_meanings_of_count() {
    let bindings = NativeOptions::default().with_binding(
        "names",
        InputType::List,
        "$medications[*].medDrugName",
    );
    let formula = "count($medications[*].medDrugName)";
    let cel = convert_native(formula, Language::Fel, &bindings).unwrap();
    assert_eq!(cel, "names.filter(item, dyn(item) != null).size()");
    assert_eq!(
        convert_native(&cel, Language::Cel, &bindings).unwrap(),
        formula
    );
    assert_eq!(
        convert_native("size(names)", Language::Cel, &bindings).unwrap(),
        "length($medications[*].medDrugName)"
    );
    assert!(convert_native("count(null)", Language::Fel, &bindings).is_err());
    assert_eq!(
        convert(
            &convert("count(null)", Language::Fel).unwrap(),
            Language::Cel
        )
        .unwrap(),
        "count(null)"
    );
}

#[test]
fn source_limits_and_deep_editing_are_reversible() {
    let deep = format!("{}x{}", "f(".repeat(50), ")".repeat(50));
    let converted = convert(&deep, Language::Cel).unwrap();
    assert_eq!(convert(&converted, Language::Fel).unwrap(), deep);

    // Flat FEL operator chains can require more parentheses in generated CEL.
    let flat = format!("{}1", "1 + ".repeat(90));
    let converted = convert(&flat, Language::Fel).unwrap();
    let returned = convert(&converted, Language::Cel).unwrap();
    assert_eq!(
        fel_core::parse(&returned).unwrap(),
        fel_core::parse(&flat).unwrap()
    );
    let oversized = " ".repeat(fel_core::cel::MAX_SOURCE_BYTES + 1);
    assert!(matches!(
        convert(&oversized, Language::Cel),
        Err(fel_core::cel::ConversionError::Limit(_))
    ));
}

#[test]
fn full_size_cli_output_can_be_piped_back() {
    let header = "// cel-fel-syntax v1 from FEL\n";
    let text = "a".repeat(fel_core::cel::MAX_SOURCE_BYTES - header.len() - 2);
    let original = format!("'{text}'");
    let converted = convert(&original, Language::Fel).unwrap();
    assert_eq!(converted.len(), fel_core::cel::MAX_SOURCE_BYTES);
    let mut child = Command::new(env!("CARGO_BIN_EXE_fel-convert"))
        .arg("to-fel")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(child.stdin.take().unwrap(), "{converted}").unwrap();
    let restored = child.wait_with_output().unwrap();
    assert!(restored.status.success());
    assert_eq!(
        String::from_utf8(restored.stdout).unwrap().trim_end(),
        original
    );
}

#[test]
fn cli_reads_stdin_and_keeps_errors_off_stdout() {
    let bin = env!("CARGO_BIN_EXE_fel-convert");
    let mut child = Command::new(bin)
        .arg("to-cel")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"$age > 18").unwrap();
    let converted = child.wait_with_output().unwrap();
    assert!(converted.status.success());
    let cel = String::from_utf8(converted.stdout).unwrap();
    assert_eq!(convert(&cel, Language::Cel).unwrap(), "($age > 18)");
    for args in [
        vec!["to-cel", "--native", "count(null)"],
        vec!["to-fel", "true", "{}"],
        vec!["to-fel", "// cel-fel-syntax v9 from FEL\ntrue"],
        vec![
            "to-cel",
            "--native",
            "--bind",
            "n:int=$n",
            "--bind",
            "n:list=$xs",
            "$n",
        ],
    ] {
        let failed = Command::new(bin).args(args).output().unwrap();
        assert!(!failed.status.success());
        assert!(failed.stdout.is_empty());
        assert!(!failed.stderr.is_empty());
    }
}
