//! Data-driven item display resolution.
//!
//! The serial codec intentionally knows only the wire format. This module maps
//! catalog records onto a decoded two-section layout. The bundled snapshot is
//! attributed in THIRD_PARTY_NOTICES.md; its game build is unverified.

use super::item_serial::{Part, TwoSectionItemLayout};

#[path = "item_catalog_data.rs"]
mod bundled_data;

/// Offline catalog from the pinned upstream snapshot. Its build is unverified;
/// mappings are display hints and never authorize item mutations.
pub fn bundled_catalog() -> &'static ItemCatalog<'static> {
    &bundled_data::CATALOG
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootRecord<'a> {
    /// First value in an observed two-section serial layout.
    pub root_id: u32,
    pub family: &'a str,
    pub manufacturer: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartRecord<'a> {
    /// Catalog root whose part-index namespace owns this record.
    pub root_id: u32,
    pub part_index: u16,
    pub display_name: Option<&'a str>,
    pub category: Option<&'a str>,
    pub internal_name: Option<&'a str>,
    /// Native role for naming, separate from the display category.
    pub naming_role: Option<&'a str>,
    pub rarity: Option<&'a str>,
    pub unique_accessory: bool,
}

/// A catalog snapshot for one explicitly recorded game version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemCatalog<'a> {
    pub game_version: &'a str,
    pub roots: &'a [RootRecord<'a>],
    pub parts: &'a [PartRecord<'a>],
}

impl<'a> ItemCatalog<'a> {
    /// Base weapon title only. Conflicting titles at equal priority stay unknown.
    /// Prefixes and foreign-root groups require separate interpretation.
    pub fn weapon_name(&self, layout: &TwoSectionItemLayout) -> Option<&'a str> {
        let root = layout.first_section[0].value;
        if !matches!(
            self.root(root)?.family,
            "Pistol" | "Shotgun" | "SMG" | "Sniper" | "Assault Rifle"
        ) {
            return None;
        }
        let mut best: Option<(u8, &'a str)> = None;
        let mut ambiguous = false;
        for part in &layout.parts {
            if !matches!(part.value, super::item_serial::PartValue::None) {
                continue;
            }
            let Some(record) = self.part(root, part.index) else {
                continue;
            };
            let Some(name) = record.display_name.filter(|name| !name.trim().is_empty()) else {
                continue;
            };
            let priority = match (record.naming_role, record.rarity, record.unique_accessory) {
                (Some("barrel_acc"), _, true) => 4,
                (Some("inv_comp"), Some("Pearl"), _) => 3,
                (Some("inv_comp"), Some("Legendary"), _) => 2,
                (Some("barrel"), _, _) => 1,
                _ => continue,
            };
            match best {
                None => {
                    best = Some((priority, name));
                    ambiguous = false;
                }
                Some((rank, _)) if priority > rank => {
                    best = Some((priority, name));
                    ambiguous = false;
                }
                Some((rank, previous)) if priority == rank && previous != name => ambiguous = true,
                _ => {}
            }
        }
        if ambiguous {
            None
        } else {
            best.map(|(_, name)| name)
        }
    }
    pub fn root(&self, root_id: u32) -> Option<&RootRecord<'a>> {
        self.roots.iter().find(|record| record.root_id == root_id)
    }

    pub fn part(&self, root_id: u32, part_index: u16) -> Option<&PartRecord<'a>> {
        self.parts
            .iter()
            .find(|record| record.root_id == root_id && record.part_index == part_index)
    }

    /// Produces display data without altering the serial or assuming that an
    /// absent catalog record makes a structurally valid part invalid.
    pub fn resolve(&'a self, layout: &TwoSectionItemLayout) -> ResolvedItem<'a> {
        let root_id = layout.first_section[0].value;
        ResolvedItem {
            root_id,
            root: self.root(root_id),
            level_candidate: layout.level_candidate(),
            parts: layout
                .parts
                .iter()
                .map(|part| ResolvedPart {
                    part: part.clone(),
                    record: self.part(root_id, part.index),
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedItem<'a> {
    pub root_id: u32,
    pub root: Option<&'a RootRecord<'a>>,
    pub level_candidate: u32,
    pub parts: Vec<ResolvedPart<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPart<'a> {
    pub part: Part,
    pub record: Option<&'a PartRecord<'a>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::item_serial::{ItemSerial, PartValue};

    #[test]
    fn weapon_names_follow_role_and_rarity_priority_not_part_order() {
        let roots = [RootRecord {
            root_id: 7,
            family: "Shotgun",
            manufacturer: None,
        }];
        let make_part = |index, name, role, rarity, unique| PartRecord {
            root_id: 7,
            part_index: index,
            display_name: Some(name),
            category: None,
            internal_name: None,
            naming_role: Some(role),
            rarity,
            unique_accessory: unique,
        };
        let parts = [
            make_part(1, "Misleading magazine name", "magazine", None, false),
            make_part(2, "Barrel title", "barrel", None, false),
            make_part(3, "Legendary title", "inv_comp", Some("Legendary"), false),
            make_part(4, "Pearl title", "inv_comp", Some("Pearl"), false),
            make_part(5, "Unique variant", "barrel_acc", None, true),
            make_part(6, "Conflicting variant", "barrel_acc", None, true),
        ];
        let catalog = ItemCatalog {
            game_version: "synthetic",
            roots: &roots,
            parts: &parts,
        };
        let mut layout = ItemSerial::new("@Ugd77*FnkbUIz`YKRG/`es4}RMsG6vgs4=KLs6VJss9LCU2m")
            .decode()
            .unwrap()
            .two_section_layout()
            .unwrap();
        let simple = |index| Part {
            index,
            value: PartValue::None,
        };
        layout.parts = vec![simple(1)];
        assert_eq!(catalog.weapon_name(&layout), None);
        for (indices, expected) in [
            (vec![1, 2], Some("Barrel title")),
            (vec![3, 2, 1], Some("Legendary title")),
            (vec![2, 4, 3], Some("Pearl title")),
            (vec![5, 4, 3], Some("Unique variant")),
            (vec![5, 6], None),
            (vec![6, 5], None),
        ] {
            layout.parts = indices.into_iter().map(simple).collect();
            assert_eq!(catalog.weapon_name(&layout), expected);
        }
        // A group or valued part cannot masquerade as a simple naming part.
        layout.parts = vec![Part {
            index: 5,
            value: PartValue::Integer(3),
        }];
        assert_eq!(catalog.weapon_name(&layout), None);
        layout.first_section[0].value = u32::MAX;
        assert_eq!(catalog.weapon_name(&layout), None);
    }

    #[test]
    fn bundled_root_mapping_identifies_the_supplied_serial_as_a_shotgun() {
        let catalog = bundled_catalog();
        let decoded = ItemSerial::new("@Ugd77*FnkbUIz`YKRG/`es4}RMsG6vgs4=KLs6VJss9LCU2m")
            .decode()
            .unwrap();
        let layout = decoded.two_section_layout().unwrap();
        let resolved = catalog.resolve(&layout);
        let root = resolved.root.unwrap();
        assert_eq!(root.family, "Shotgun");
        assert_eq!(root.manufacturer, Some("Ripper"));
        assert_eq!(resolved.parts.len(), 14);
        assert!(catalog.root(u32::MAX).is_none());
        assert!(catalog.part(7, u16::MAX).is_none());
    }

    #[test]
    fn bundled_records_have_unique_keys_and_nonempty_labels() {
        let catalog = bundled_catalog();
        let roots: std::collections::BTreeSet<_> =
            catalog.roots.iter().map(|row| row.root_id).collect();
        let parts: std::collections::BTreeSet<_> = catalog
            .parts
            .iter()
            .map(|row| (row.root_id, row.part_index))
            .collect();
        assert_eq!(roots.len(), catalog.roots.len());
        assert_eq!(parts.len(), catalog.parts.len());
        assert!(!roots.is_empty());
        assert!(!parts.is_empty());
        assert!(catalog.parts.iter().all(|row| {
            [row.display_name, row.category, row.internal_name]
                .into_iter()
                .flatten()
                .any(|label| !label.trim().is_empty())
        }));
    }

    #[test]
    fn resolves_only_the_records_present_in_a_versioned_catalog() {
        let catalog = ItemCatalog {
            game_version: "synthetic",
            roots: &[RootRecord {
                root_id: 7,
                family: "synthetic equipment",
                manufacturer: None,
            }],
            parts: &[PartRecord {
                root_id: 7,
                part_index: 19,
                display_name: Some("synthetic core"),
                category: None,
                internal_name: None,
                naming_role: None,
                rarity: None,
                unique_accessory: false,
            }],
        };
        let decoded = ItemSerial::new("@Ugd77*FnkbUIz`YKRG/`es4}RMsG6vgs4=KLs6VJss9LCU2m")
            .decode()
            .expect("known public serial parses");
        let layout = decoded.two_section_layout().expect("known layout");

        let resolved = catalog.resolve(&layout);

        assert_eq!(
            resolved.root.map(|record| record.family),
            Some("synthetic equipment")
        );
        assert_eq!(resolved.level_candidate, 60);
        assert_eq!(
            resolved.parts[0]
                .record
                .and_then(|record| record.display_name),
            Some("synthetic core")
        );
        assert_eq!(resolved.parts[0].part.value, PartValue::None);
        assert!(resolved.parts[1].record.is_none());
    }
}
