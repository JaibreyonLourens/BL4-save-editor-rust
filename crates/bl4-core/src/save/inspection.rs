//! Read-only views of evidenced save fields. The complete document remains the
//! source of truth; these views must never be used to reconstruct a save.
use serde_yaml_ng::{Mapping, Value};

use super::document::SaveDocument;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
    Character,
    Profile,
    Unknown,
}

/// Borrowed data for display, without mutation or item-serial decoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterSummary<'a> {
    pub name: Option<&'a str>,
    pub class: &'a str,
    pub level: Option<u64>,
    pub cash: Option<u64>,
    pub eridium: Option<u64>,
    /// `None` means unavailable; `Some([])` means a known empty backpack.
    pub backpack: Option<Vec<InventoryEntry<'a>>>,
    pub equipped: Option<Vec<InventoryEntry<'a>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InventoryEntry<'a> {
    /// Unrecognized/non-string keys remain represented as unavailable slots.
    pub slot: Option<&'a str>,
    /// Exact text, including whitespace. Missing/unsupported values stay absent.
    pub serial: Option<&'a str>,
}

// Match exact variants: serde_yaml accessors can look through unknown tags.
// A tagged field has unverified semantics and must not become a known summary.
fn mapping(value: &Value) -> Option<&Mapping> {
    match value {
        Value::Mapping(value) => Some(value),
        _ => None,
    }
}

fn string(value: &Value) -> Option<&str> {
    match value {
        Value::String(value) => Some(value),
        _ => None,
    }
}

fn unsigned(value: &Value) -> Option<u64> {
    match value {
        Value::Number(value) => value.as_u64(),
        _ => None,
    }
}

fn at<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(root, |value, key| {
        mapping(value)?.get(Value::String((*key).into()))
    })
}

fn identity(root: &Value) -> Option<&str> {
    let guid = string(at(root, &["state", "char_guid"])?)?;
    let class = string(at(root, &["state", "class"])?)?;
    if guid.trim().is_empty() || class.trim().is_empty() {
        None
    } else {
        Some(class)
    }
}

fn profile_shape(root: &Value) -> bool {
    ["inputprefs", "onlineprefs", "domains"]
        .iter()
        .all(|key| at(root, &[*key]).and_then(mapping).is_some())
}

fn character_level(root: &Value) -> Option<u64> {
    let Value::Sequence(records) = at(root, &["state", "experience"])? else {
        return None;
    };
    let mut selected = None;
    for record in records {
        // An unrecognized record shape could hide a second Character record.
        let record_type = at(record, &["type"]).and_then(string)?;
        if record_type == "Character" {
            if selected.is_some() {
                return None;
            }
            selected = Some(at(record, &["level"]).and_then(unsigned)?);
        }
    }
    selected
}

impl SaveDocument {
    /// Recognizes observed character/profile shapes, not game compatibility or
    /// permission to edit. Unsupported and conflicting shapes remain unknown.
    pub fn kind(&self) -> DocumentKind {
        let root = self.root();
        if profile_shape(root) {
            // The observed profile has no character state. Reject conflicts,
            // including malformed state, instead of giving either kind priority.
            if at(root, &["state"]).is_some() {
                DocumentKind::Unknown
            } else {
                DocumentKind::Profile
            }
        } else if identity(root).is_some() {
            DocumentKind::Character
        } else {
            DocumentKind::Unknown
        }
    }

    pub fn character_summary(&self) -> Option<CharacterSummary<'_>> {
        if self.kind() != DocumentKind::Character {
            return None;
        }
        let root = self.root();
        let class = identity(root)?;
        let backpack = at(root, &["state", "inventory", "items", "backpack"])
            .and_then(mapping)
            .map(|items| {
                items
                    .iter()
                    .map(|(key, value)| InventoryEntry {
                        slot: string(key),
                        serial: at(value, &["serial"]).and_then(string),
                    })
                    .collect()
            });
        let equipped = at(
            root,
            &["state", "inventory", "equipped_inventory", "equipped"],
        )
        .and_then(mapping)
        .map(|items| {
            let mut entries = Vec::new();
            for (key, value) in items {
                let slot = string(key);
                match value {
                    Value::Sequence(records) => {
                        entries.extend(records.iter().map(|record| InventoryEntry {
                            slot,
                            serial: at(record, &["serial"]).and_then(string),
                        }));
                    }
                    record => entries.push(InventoryEntry {
                        slot,
                        serial: at(record, &["serial"]).and_then(string),
                    }),
                }
            }
            entries
        });

        Some(CharacterSummary {
            name: at(root, &["state", "char_name"]).and_then(string),
            class,
            level: character_level(root),
            cash: at(root, &["state", "currencies", "cash"]).and_then(unsigned),
            eridium: at(root, &["state", "currencies", "eridium"]).and_then(unsigned),
            backpack,
            equipped,
        })
    }
}
