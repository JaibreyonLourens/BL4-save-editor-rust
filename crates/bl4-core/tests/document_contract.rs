//! Supported-document requirements with literal expected values and types.
use bl4_core::save::document::{DocumentError, SaveDocument};
use serde_yaml_ng::Value;

fn round_trip(source: &str) -> SaveDocument {
    let doc = SaveDocument::parse(source.as_bytes()).expect("supported document");
    let reopened = SaveDocument::parse(&doc.to_yaml_bytes().unwrap()).expect("reopen document");
    assert_eq!(reopened, doc);
    reopened
}

#[test]
fn core_tags_retain_their_defined_types_and_values() {
    let doc = round_trip(concat!(
        "text: !!str 42\nint: !!int '42'\nfloat: !!float '1.5'\n",
        "boolean: !!bool 'true'\nempty: !!null 'null'\n",
        "sequence: !!seq [1, '2']\nmapping: !!map {x: 3}\n",
        "verbatim: !<tag:yaml.org,2002:str> true\n",
    ));
    assert_eq!(doc.root()["text"], Value::String("42".into()));
    assert_eq!(doc.root()["int"].as_u64(), Some(42));
    assert_eq!(doc.root()["float"].as_f64(), Some(1.5));
    assert_eq!(doc.root()["boolean"], Value::Bool(true));
    assert!(doc.root()["empty"].is_null());
    assert_eq!(doc.root()["sequence"][1], Value::String("2".into()));
    assert_eq!(doc.root()["mapping"]["x"].as_u64(), Some(3));
    assert_eq!(doc.root()["verbatim"], Value::String("true".into()));
}

#[test]
fn tags_that_cannot_be_preserved_and_wrong_node_kinds_are_rejected() {
    for source in [
        "value: !<tag:example.org,2026:Future> 42",
        "value: !!timestamp 2026-01-01",
        "value: !!binary SGVsbG8=",
        "value: !!set {a: null}",
        "%TAG !e! tag:example.org,2026:\n---\nvalue: !e!Future {x: 1}",
        "value: !!str [1, 2]",
        "value: !!int {x: 1}",
        "value: !!seq {x: 1}",
        "value: !!map [1, 2]",
        "value: ! 42",
    ] {
        assert!(
            matches!(
                SaveDocument::parse(source.as_bytes()),
                Err(DocumentError::UnsupportedStructure)
            ),
            "unsupported tag accepted: {source}"
        );
    }
}

#[test]
fn merge_semantics_are_rejected_but_literal_merge_characters_are_preserved() {
    for source in [
        "value: {<<: {x: 1}, y: 2}",
        "value: {<<: [{x: 1}, {y: 2}]}",
        "value: {!!merge '<<': {x: 1}}",
        "'<<': literal key",
        "value: {!!str '<<': literal key}",
    ] {
        assert!(matches!(
            SaveDocument::parse(source.as_bytes()),
            Err(DocumentError::UnsupportedStructure)
        ));
    }
    let doc = round_trip("value: <<\nlist: ['<<', '&anchor', '*alias']");
    assert_eq!(doc.root()["value"], Value::String("<<".into()));
}

#[test]
fn integer_boundaries_are_exact_and_quoted_numbers_stay_strings() {
    let doc = round_trip(concat!(
        "min: -9223372036854775808\nmax: 18446744073709551615\n",
        "hex: 0xffffffffffffffff\noctal: 0o17\n",
        "quoted: '18446744073709551616'\nlarge_exact: 9007199254740993\n",
    ));
    assert_eq!(doc.root()["min"].as_i64(), Some(i64::MIN));
    assert_eq!(doc.root()["max"].as_u64(), Some(u64::MAX));
    assert_eq!(doc.root()["hex"].as_u64(), Some(u64::MAX));
    assert_eq!(doc.root()["octal"].as_u64(), Some(15));
    assert_eq!(
        doc.root()["large_exact"].as_u64(),
        Some(9_007_199_254_740_993)
    );
    assert_eq!(
        doc.root()["quoted"],
        Value::String("18446744073709551616".into())
    );
}

#[test]
fn unrepresentable_numbers_are_rejected_instead_of_rounded_or_retyped() {
    for value in [
        "18446744073709551616",
        "-9223372036854775809",
        "340282366920938463463374607431768211456",
        "0x10000000000000000",
        "-0x8000000000000001",
        "1e9999",
        "1e-9999",
        "!!float 1e9999",
        "!Future 340282366920938463463374607431768211456",
        "-1e-9999",
        "!!int 18446744073709551616",
        "!!float '1e-9999'",
    ] {
        let source = format!("value: {value}");
        assert!(
            SaveDocument::parse(source.as_bytes()).is_err(),
            "unrepresentable number accepted: {value}"
        );
    }
}

#[test]
fn binary64_special_values_and_negative_zero_survive() {
    let doc = round_trip("positive: .inf\nnegative: -.inf\nnan: .nan\nzero: -0.0\nsmall: 5e-324\n");
    assert_eq!(doc.root()["positive"].as_f64(), Some(f64::INFINITY));
    assert_eq!(doc.root()["negative"].as_f64(), Some(f64::NEG_INFINITY));
    assert!(doc.root()["nan"].as_f64().unwrap().is_nan());
    assert!(doc.root()["zero"].as_f64().unwrap().is_sign_negative());
    assert_eq!(doc.root()["small"].as_f64().unwrap().to_bits(), 1);
}

#[test]
fn duplicate_keys_aliases_and_multiple_documents_fail_in_production() {
    for source in [
        "x: 1\nx: 2",
        "outer: {x: 1, x: 2}",
        "? [a, b]\n: 1\n? [a, b]\n: 2",
        "x: &shared [1]\ny: *shared",
        "x: &self [*self]",
        "x: 1\n---\ny: 2",
        "x: 1\n...\n---\ny: [",
    ] {
        assert!(SaveDocument::parse(source.as_bytes()).is_err());
    }
}

#[test]
fn resource_limits_stop_before_a_later_syntax_error() {
    // Limits must abort event parsing, not just reject after traversing all input.
    let source = format!("value: {}0{}\nlater: [", "[".repeat(128), "]".repeat(128));
    assert!(matches!(
        SaveDocument::parse(source.as_bytes()),
        Err(DocumentError::TooDeep)
    ));
    let source = format!("value: [{}]\nlater: [", "0,".repeat(100_000));
    assert!(matches!(
        SaveDocument::parse(source.as_bytes()),
        Err(DocumentError::TooManyNodes)
    ));
}

#[test]
fn exact_depth_and_node_budgets_are_accepted() {
    // Root map + 127 nested sequences = 128 collections.
    let source = format!("value: {}0{}", "[".repeat(127), "]".repeat(127));
    round_trip(&source);
    // Root map, key, sequence + 99,997 scalar items = 100,000 nodes.
    let source = format!("value: [{}]", "0,".repeat(99_997));
    let doc = round_trip(&source);
    assert_eq!(doc.root()["value"].as_sequence().unwrap().len(), 99_997);
}

#[test]
fn local_tags_keep_identity_and_numeric_looking_strings_keep_their_types() {
    let doc = round_trip(concat!(
        "scalar: !Future '18446744073709551616'\n",
        "sequence: !Items [!Inner 'true']\nmapping: !Record {x: 3}\n",
        "looks_numeric: ['1e9999', '1e-9999', inf, NaN, '0123']\n",
    ));
    for (key, tag) in [
        ("scalar", "Future"),
        ("sequence", "Items"),
        ("mapping", "Record"),
    ] {
        let Value::Tagged(value) = &doc.root()[key] else {
            panic!("tag lost")
        };
        assert_eq!(value.tag, serde_yaml_ng::value::Tag::new(tag));
    }
    for (index, expected) in ["1e9999", "1e-9999", "inf", "NaN", "0123"]
        .iter()
        .enumerate()
    {
        assert_eq!(
            doc.root()["looks_numeric"][index],
            Value::String((*expected).into())
        );
    }
}

#[test]
fn numeric_text_reconstruction_handles_unicode_keys_tags_and_multiple_occurrences() {
    let doc = round_trip(concat!(
        "'h\u{00e9}llo \u{1f30d}': '1e9999'\n",
        "next: !Future '1e-9999'\n",
        "nested: {'340282366920938463463374607431768211456': '1e9999'}\n",
        "multiline: |\n  1e9999\n  \u{00e9}\n",
    ));
    assert_eq!(
        doc.root()["h\u{00e9}llo \u{1f30d}"],
        Value::String("1e9999".into())
    );
    assert_eq!(
        doc.root()["nested"]["340282366920938463463374607431768211456"],
        Value::String("1e9999".into())
    );
    assert_eq!(
        doc.root()["multiline"],
        Value::String("1e9999\n\u{00e9}\n".into())
    );
    let Value::Tagged(tagged) = &doc.root()["next"] else {
        panic!("tag lost")
    };
    assert_eq!(tagged.value, Value::String("1e-9999".into()));
}
