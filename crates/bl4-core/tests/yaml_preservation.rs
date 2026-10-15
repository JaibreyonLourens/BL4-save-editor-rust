//! BL4-007 candidate-parser probes. These are not the production document API.
use bl4_core::save::document::SaveDocument;
use serde_yaml_ng::{Value, value::Tag};

fn parse(source: &str) -> Value {
    serde_yaml_ng::from_str(source).expect("Parse synthetic YAML")
}

fn round_trip(value: &Value) {
    let serialized = serde_yaml_ng::to_string(value).expect("Serialize synthetic tree");
    let recovered = parse(&serialized);
    assert_eq!(&recovered, value);
}

#[test]
fn unknown_fields_and_nested_collections_survive() {
    let tree =
        parse("future: {nested: [3, {unknown: keep}, [], {}]}\norder: [third, first, second]\n");
    assert_eq!(
        tree["future"]["nested"][1]["unknown"],
        Value::String("keep".into())
    );
    assert_eq!(tree["order"], parse("[third, first, second]"));
    round_trip(&tree);
}

#[test]
fn scalar_types_and_string_contents_survive() {
    let tree = parse("[null, true, false, 42, -7, 1.5, '42', 'true', 'null', 'héllo 🌍', [], {}]");
    let values = tree.as_sequence().unwrap();
    assert!(values[0].is_null());
    assert_eq!(values[1], Value::Bool(true));
    assert_eq!(values[2], Value::Bool(false));
    assert_eq!(values[3].as_u64(), Some(42));
    assert_eq!(values[4].as_i64(), Some(-7));
    assert_eq!(values[5].as_f64(), Some(1.5));
    for (index, expected) in [(6, "42"), (7, "true"), (8, "null"), (9, "héllo 🌍")] {
        assert_eq!(values[index], Value::String(expected.into()));
    }
    assert!(values[10].as_sequence().unwrap().is_empty());
    assert!(values[11].as_mapping().unwrap().is_empty());
    round_trip(&tree);
    let multiline = parse("text: |\n  first\n  second\n");
    assert_eq!(multiline["text"], Value::String("first\nsecond\n".into()));
    round_trip(&multiline);
}

#[test]
fn local_tags_on_scalars_sequences_and_mappings_survive() {
    let tree =
        parse("scalar: !Future '42'\nsequence: !Items [1, !Inner x]\nmapping: !Record {x: 2}\n");
    for (key, name) in [
        ("scalar", "Future"),
        ("sequence", "Items"),
        ("mapping", "Record"),
    ] {
        let Value::Tagged(tagged) = &tree[key] else {
            panic!("Expected tagged node")
        };
        assert_eq!(tagged.tag, Tag::new(name));
    }
    let Value::Tagged(tagged) = &tree["scalar"] else {
        unreachable!()
    };
    assert_eq!(tagged.value, Value::String("42".into()));
    round_trip(&tree);
}

#[test]
fn non_string_and_collection_mapping_keys_survive() {
    let tree = parse("42: integer\ntrue: boolean\n? [a, b]\n: sequence\n? {x: 1}\n: mapping\n");
    let mapping = tree.as_mapping().unwrap();
    assert_eq!(mapping.len(), 4);
    for (key, expected) in [
        ("42", "integer"),
        ("true", "boolean"),
        ("[a, b]", "sequence"),
        ("{x: 1}", "mapping"),
    ] {
        assert_eq!(
            mapping.get(parse(key)),
            Some(&Value::String(expected.into()))
        );
    }
    round_trip(&tree);
}

#[test]
fn duplicate_keys_are_rejected_including_nested_and_complex_keys() {
    for source in [
        "x: 1\nx: 2",
        "outer: {x: 1, x: 2}",
        "? [a, b]\n: 1\n? [a, b]\n: 2",
    ] {
        assert!(serde_yaml_ng::from_str::<Value>(source).is_err());
    }
}

#[test]
fn aliases_expand_to_values_and_anchor_identity_is_lost() {
    let tree = parse("original: &base {x: [1, 2]}\ncopy: *base\n");
    assert_eq!(tree["original"], parse("{x: [1, 2]}"));
    assert_eq!(tree["copy"], tree["original"]);
    let serialized = serde_yaml_ng::to_string(&tree).unwrap();
    assert!(!serialized.contains("&base"));
    assert!(!serialized.contains("*base"));
    round_trip(&tree);
    // The candidate cannot preserve shared-node identity for later edits.
    assert!(serde_yaml_ng::from_str::<Value>("x: &self [*self]").is_err());
}

#[test]
fn merge_keys_remain_literal_without_automatic_merge() {
    let tree = parse("base: &base {x: 1}\ncopy: {<<: *base, y: 2}\n");
    assert_eq!(tree["copy"]["<<"], parse("{x: 1}"));
    assert!(tree["copy"].get("x").is_none());
    round_trip(&tree);
}

#[test]
fn invalid_text_malformed_yaml_and_multiple_documents_are_rejected() {
    assert!(serde_yaml_ng::from_slice::<Value>(&[0xff, 0xfe, 0xff]).is_err());
    for source in ["x: [", "---\nx: 1\n---\nx: 2"] {
        assert!(serde_yaml_ng::from_str::<Value>(source).is_err());
    }
}

#[test]
fn empty_input_is_accepted_as_null_by_candidate() {
    for source in ["", "# comment only\n", "---\n"] {
        assert_eq!(parse(source), Value::Null);
    }
    // Save-document policy must distinguish absent content before calling this parser.
}

#[test]
fn deeply_nested_input_hits_candidate_recursion_limit() {
    let source = format!("{}0{}", "[".repeat(256), "]".repeat(256));
    assert!(serde_yaml_ng::from_str::<Value>(&source).is_err());
}

#[test]
#[ignore = "requires private exported YAML; does not print document contents"]
fn private_yaml_semantic_round_trip() {
    let path = std::env::var("BL4_YAML_PATH")
        .or_else(|_| std::env::var("BL4_DECODED_YAML_PATH"))
        .expect("Set BL4_YAML_PATH to a private export");
    let original = std::fs::read(&path).expect("Read private YAML");
    // Check UTF-8 explicitly; a YAML parser may accept other text encodings.
    assert!(
        std::str::from_utf8(&original).is_ok(),
        "Private YAML is not UTF-8"
    );
    let parsed = serde_yaml_ng::from_slice::<Value>(&original)
        .unwrap_or_else(|_| panic!("Private YAML parsing failed; contents withheld"));
    let serialized = serde_yaml_ng::to_string(&parsed)
        .unwrap_or_else(|_| panic!("Private YAML serialization failed; contents withheld"));
    let recovered = serde_yaml_ng::from_str::<Value>(&serialized)
        .unwrap_or_else(|_| panic!("Private YAML reparsing failed; contents withheld"));
    assert!(
        parsed == recovered,
        "Private full-tree comparison failed; contents withheld"
    );
    assert!(
        std::fs::read(&path).expect("Reread private YAML") == original,
        "Source changed"
    );
    println!(
        "Private YAML full-tree round trip passed ({} input bytes).",
        original.len()
    );
}

#[test]
#[ignore = "requires a private YAML export"]
fn production_document_round_trips_private_yaml() {
    let path = std::env::var("BL4_YAML_PATH")
        .or_else(|_| std::env::var("BL4_DECODED_YAML_PATH"))
        .expect("Set BL4_YAML_PATH to a private YAML export");

    let source = std::fs::read(&path).expect("read private YAML");

    let original = SaveDocument::parse(&source).expect("parse private YAML");

    let serialized = original.to_yaml_bytes().expect("serialize private YAML");

    let recovered = SaveDocument::parse(&serialized).expect("reparse private YAML");

    assert!(
        recovered == original,
        "private document tree changed; contents withheld"
    );
    assert!(
        std::fs::read(&path).expect("reread private YAML") == source,
        "private source changed; contents withheld"
    );
}
