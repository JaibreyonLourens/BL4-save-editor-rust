use bl4_core::save::{
    container::{ContainerContext, Platform, encode},
    document::SaveDocument,
    inspection::DocumentKind,
    session::SaveSession,
};

fn document(state: &str) -> SaveDocument {
    SaveDocument::parse(
        format!("state:\n{state}\nfuture: !Unknown {{nested: [true, '42']}}\n").as_bytes(),
    )
    .expect("valid synthetic document")
}

#[test]
fn equipped_slots_accept_single_records_and_lists_without_changing_the_document() {
    let doc = document(concat!(
        "  char_guid: synthetic\n  class: hunter\n",
        "  inventory:\n    equipped_inventory:\n      equipped:\n",
        "        weapon_1: [{serial: 'first'}, {serial: 'second'}]\n",
        "        shield: {serial: '  exact serial  '}\n",
        "        empty: []\n        malformed: null\n",
    ));
    let original = doc.clone();
    let entries = doc.character_summary().unwrap().equipped.unwrap();
    assert_eq!(entries.len(), 4);
    assert_eq!(entries[0].slot, Some("weapon_1"));
    assert_eq!(entries[1].slot, Some("weapon_1"));
    assert_eq!(entries[0].serial, Some("first"));
    assert_eq!(entries[1].serial, Some("second"));
    assert_eq!(entries[2].slot, Some("shield"));
    assert_eq!(entries[2].serial, Some("  exact serial  "));
    assert_eq!(entries[3].serial, None);
    assert_eq!(doc, original);
}

#[test]
fn equipped_distinguishes_empty_unavailable_and_unsupported_values() {
    let empty = document(
        "  char_guid: id\n  class: hunter\n  inventory: {equipped_inventory: {equipped: {}}}",
    );
    assert_eq!(empty.character_summary().unwrap().equipped, Some(vec![]));
    for inventory in [
        "{}",
        "{items: {equipped: {slot_0: {serial: wrong_path}}}}",
        "{equipped_inventory: {equipped: []}}",
        "{equipped_inventory: {equipped: !Future {}}}",
    ] {
        let doc = document(&format!(
            "  char_guid: id\n  class: hunter\n  inventory: {inventory}"
        ));
        assert!(doc.character_summary().unwrap().equipped.is_none());
    }
}

#[test]
fn observed_character_identity_allows_independent_optional_summaries() {
    let doc = document(concat!(
        "  char_guid: synthetic-guid\n",
        "  class: synthetic-class\n",
        "  char_name: 'Hero'\n",
        "  currencies: {cash: 1234, eridium: 57}\n",
        "  inventory:\n",
        "    items:\n",
        "      backpack:\n",
        "        slot_9: {serial: 'unsupported serial', unknown: [1, 2]}\n",
        "        slot_2: {serial: 'another serial'}\n",
    ));
    assert_eq!(doc.kind(), DocumentKind::Character);
    let summary = doc.character_summary().expect("character summary");
    assert_eq!(summary.name, Some("Hero"));
    assert_eq!(summary.class, "synthetic-class");
    assert_eq!(summary.cash, Some(1234));
    assert_eq!(summary.eridium, Some(57));
    let inventory = summary.backpack.expect("backpack available");
    assert_eq!(inventory.len(), 2);
    assert_eq!(inventory[0].slot, Some("slot_9"));
    assert_eq!(inventory[0].serial, Some("unsupported serial"));
    assert_eq!(inventory[1].slot, Some("slot_2"));
    assert_eq!(inventory[1].serial, Some("another serial"));
}

#[test]
fn character_level_uses_the_unique_character_record_regardless_of_order() {
    for records in [
        "[{type: Specialization, level: 9}, {type: Character, level: 42}]",
        "[{type: Character, level: 42}, {type: Specialization, level: 9}]",
        "[{type: Future, level: 99}, {type: Character, level: 42}]",
    ] {
        let doc = document(&format!(
            "  char_guid: id\n  class: hunter\n  experience: {records}"
        ));
        assert_eq!(doc.character_summary().unwrap().level, Some(42));
    }
}

#[test]
fn missing_malformed_or_ambiguous_experience_never_guesses_a_level() {
    for records in [
        "null",
        "{}",
        "[]",
        "!Future [{type: Character, level: 42}]",
        "[{type: Specialization, level: 9}]",
        "[{type: character, level: 42}]",
        "[{type: Character}]",
        "[{type: Character, level: '42'}]",
        "[{type: Character, level: -1}]",
        "[{type: Character, level: 1.5}]",
        "[{type: Character, level: !Future 42}]",
        "[!Future {type: Character, level: 42}]",
        "[{type: !Future Character, level: 42}]",
        "[{type: Character, level: 42}, {type: Character, level: 42}]",
        "[{type: Character, level: 42}, {type: Character}]",
        "[{type: Character, level: 42}, {}]",
        "[{type: Character, level: 42}, null]",
    ] {
        let doc = document(&format!(
            "  char_guid: id\n  class: hunter\n  experience: {records}"
        ));
        assert_eq!(doc.character_summary().unwrap().level, None);
    }
    assert_eq!(
        document("  char_guid: id\n  class: hunter")
            .character_summary()
            .unwrap()
            .level,
        None
    );
}

#[test]
fn absent_or_wrong_typed_identity_never_guesses_a_character() {
    for state in [
        "  char_name: Hero\n  currencies: {cash: 42}",
        "  char_guid: id",
        "  class: hunter",
        "  char_guid: null\n  class: hunter",
        "  char_guid: 42\n  class: hunter",
        "  char_guid: id\n  class: []",
        "  char_guid: ''\n  class: hunter",
        "  char_guid: id\n  class: '  '",
        "  char_guid: !Future id\n  class: hunter",
        "  char_guid: id\n  class: !Future hunter",
    ] {
        let doc = document(state);
        assert_eq!(doc.kind(), DocumentKind::Unknown);
        assert!(doc.character_summary().is_none());
    }
    for yaml in [
        "state: []",
        "state: !Future {char_guid: id, class: hunter}",
        "profile: {}",
        "future: {}",
    ] {
        let doc = SaveDocument::parse(yaml.as_bytes()).unwrap();
        assert_eq!(doc.kind(), DocumentKind::Unknown);
        assert!(doc.character_summary().is_none());
    }
}

#[test]
fn summary_values_are_not_coerced_or_defaulted_to_zero() {
    for fields in [
        "",
        "  char_name: 42\n  currencies: []\n  inventory: []",
        "  char_name: !Future Hero\n  currencies: {cash: '42', eridium: -1}",
        "  currencies: {cash: true, eridium: 1.5}",
        "  currencies: {cash: !Future 42, eridium: null}",
        "  currencies: !Future {cash: 42, eridium: 3}",
    ] {
        let doc = document(&format!("  char_guid: id\n  class: hunter\n{fields}"));
        let summary = doc.character_summary().unwrap();
        assert_eq!(summary.name, None);
        assert_eq!(summary.cash, None);
        assert_eq!(summary.eridium, None);
        assert_eq!(summary.backpack, None);
    }
    let doc = document(
        "  char_guid: id\n  class: hunter\n  currencies: {cash: 0, eridium: 18446744073709551615}",
    );
    let summary = doc.character_summary().unwrap();
    assert_eq!(summary.cash, Some(0));
    assert_eq!(summary.eridium, Some(u64::MAX));
}

#[test]
fn unavailable_backpack_is_distinct_from_empty_and_bad_entries_are_retained() {
    for inventory in [
        "{}",
        "{items: {backpack: []}}",
        "{items: {backpack: !Future {}}}",
    ] {
        let doc = document(&format!(
            "  char_guid: id\n  class: hunter\n  inventory: {inventory}"
        ));
        assert!(doc.character_summary().unwrap().backpack.is_none());
    }
    let doc = document("  char_guid: id\n  class: hunter\n  inventory: {items: {backpack: {}}}");
    assert_eq!(doc.character_summary().unwrap().backpack, Some(vec![]));
    let doc = document(concat!(
        "  char_guid: id\n  class: hunter\n",
        "  inventory:\n    items:\n      backpack:\n",
        "        slot_0: {serial: 42}\n",
        "        slot_1: null\n",
        "        slot_2: {serial: !Future opaque}\n",
        "        7: {serial: '  preserve exactly  '}\n",
    ));
    let items = doc.character_summary().unwrap().backpack.unwrap();
    assert_eq!(items.len(), 4);
    assert!(items[..3].iter().all(|item| item.serial.is_none()));
    assert_eq!(items[3].slot, None);
    assert_eq!(items[3].serial, Some("  preserve exactly  "));
}

#[test]
fn inspection_preserves_original_bytes_unknown_tree_dirty_state_and_revision() {
    for yaml in [
        "state: {char_guid: id, class: hunter, char_name: Hero}\nfuture: !Unknown {nested: [true, '42']}",
        "profile: {future: !Unknown [1, '2', null]}",
        "inputprefs: {}\nonlineprefs: {}\ndomains: {}\nfuture: !Unknown [1, '2', null]",
    ] {
        let context = ContainerContext {
            account_id: "72623859790382856".into(),
            platform: Platform::Steam,
        };
        let ciphertext = encode(yaml.as_bytes(), &context).unwrap();
        let session = SaveSession::open(ciphertext.clone(), context.clone()).unwrap();
        let before = session.working_document().clone();
        let snapshot = session.snapshot();
        let _ = session.working_document().kind();
        let _ = session.working_document().character_summary();
        assert_eq!(session.working_document(), &before);
        assert!(!session.is_dirty());
        assert_eq!(session.revision(), 0);
        assert!(session.snapshot_is_current(&snapshot));
        assert_eq!(session.unchanged_ciphertext(), Some(ciphertext.as_slice()));
        let rebuilt = SaveSession::open(snapshot.reconstruct().unwrap(), context).unwrap();
        assert_eq!(rebuilt.working_document(), &before);
    }
}

#[test]
fn observed_profile_shape_is_recognized_without_character_summaries() {
    let doc = SaveDocument::parse(
        b"inputprefs: {}\nonlineprefs: {}\ndomains: {}\nfuture: !Unknown {nested: [1, '2']}\n",
    )
    .unwrap();
    assert_eq!(doc.kind(), DocumentKind::Profile);
    assert!(doc.character_summary().is_none());
}

#[test]
fn incomplete_mistyped_or_tagged_profile_evidence_remains_unknown() {
    for yaml in [
        "inputprefs: {}\nonlineprefs: {}",
        "inputprefs: {}\ndomains: {}",
        "onlineprefs: {}\ndomains: {}",
        "inputprefs: []\nonlineprefs: {}\ndomains: {}",
        "inputprefs: {}\nonlineprefs: null\ndomains: {}",
        "inputprefs: {}\nonlineprefs: {}\ndomains: 'profile'",
        "inputprefs: !Future {}\nonlineprefs: {}\ndomains: {}",
        "inputprefs: {}\nonlineprefs: !Future {}\ndomains: {}",
        "inputprefs: {}\nonlineprefs: {}\ndomains: !Future {}",
    ] {
        let doc = SaveDocument::parse(yaml.as_bytes()).unwrap();
        assert_eq!(doc.kind(), DocumentKind::Unknown);
        assert!(doc.character_summary().is_none());
    }
}

#[test]
fn conflicting_profile_and_character_state_never_exposes_a_character_summary() {
    for state in [
        "{char_guid: id, class: hunter, char_name: Hero}",
        "{}",
        "null",
        "!Future {}",
    ] {
        let yaml = format!("inputprefs: {{}}\nonlineprefs: {{}}\ndomains: {{}}\nstate: {state}");
        let doc = SaveDocument::parse(yaml.as_bytes()).unwrap();
        assert_eq!(doc.kind(), DocumentKind::Unknown);
        assert!(doc.character_summary().is_none());
    }
}

#[test]
#[ignore = "requires a private profile export; prints no values"]
fn private_profile_matches_observed_shape_and_preserves_the_full_tree() {
    let path = std::env::var("BL4_PROFILE_YAML_PATH").expect("set BL4_PROFILE_YAML_PATH");
    let source = std::fs::read(&path).expect("read private profile export");
    let doc =
        SaveDocument::parse(&source).unwrap_or_else(|_| panic!("private profile parse failed"));
    for key in ["inputprefs", "onlineprefs", "domains"] {
        assert!(
            matches!(doc.root().get(key), Some(serde_yaml_ng::Value::Mapping(_))),
            "profile field type differs"
        );
    }
    assert!(
        doc.root().get("state").is_none(),
        "unexpected character state"
    );
    assert!(
        doc.kind() == DocumentKind::Profile,
        "profile classification failed"
    );
    assert!(
        doc.character_summary().is_none(),
        "profile exposed character summary"
    );
    let serialized = doc
        .to_yaml_bytes()
        .unwrap_or_else(|_| panic!("private profile serialization failed"));
    let reopened = SaveDocument::parse(&serialized)
        .unwrap_or_else(|_| panic!("private profile reparse failed"));
    assert!(doc == reopened, "private profile tree changed");
    assert!(
        std::fs::read(path).unwrap() == source,
        "private profile source changed"
    );
}

#[test]
#[ignore = "requires a private character YAML export; prints no values"]
fn private_character_has_the_observed_field_types() {
    let path = std::env::var("BL4_YAML_PATH").expect("set BL4_YAML_PATH");
    let source = std::fs::read(&path).expect("read private export");
    let doc = SaveDocument::parse(&source).unwrap_or_else(|_| panic!("private parse failed"));
    assert!(
        doc.kind() == DocumentKind::Character,
        "character identity unavailable"
    );
    let summary = doc.character_summary().expect("character summary");
    assert!(summary.name.is_some(), "name type differs");
    assert!(summary.cash.is_some(), "cash type differs");
    assert!(summary.eridium.is_some(), "eridium type differs");
    assert!(summary.level.is_some(), "character level unavailable");
    let items = summary.backpack.expect("backpack type differs");
    assert!(!items.is_empty(), "expected populated private backpack");
    assert!(
        items
            .iter()
            .all(|item| item.slot.is_some() && item.serial.is_some()),
        "backpack entry type differs"
    );
    assert!(std::fs::read(path).unwrap() == source, "source changed");
}
