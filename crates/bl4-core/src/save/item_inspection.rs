use super::item_catalog::bundled_catalog;
use super::item_serial::{ItemSerial, ItemSerialError};

#[derive(Debug, PartialEq, Eq)]
pub struct ItemSummary {
    pub serial: String,
    pub weapon_name: Option<String>,
    pub manufacturer: Option<String>,
    pub item_type: Option<String>,
    pub level_candidate: Option<u32>,
    pub parts: Vec<ItemPartSummary>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ItemPartSummary {
    pub index: u16,
    pub name: Option<String>,
    pub category: Option<String>,
    pub internal_name: Option<String>,
}

pub fn inspect_item(serial: &str) -> Result<ItemSummary, ItemSerialError> {
    let decoded = ItemSerial::new(serial).decode()?;

    let mut summary = ItemSummary {
        serial: serial.to_owned(),
        weapon_name: None,
        manufacturer: None,
        item_type: None,
        level_candidate: None,
        parts: Vec::new(),
    };

    let Some(layout) = decoded.two_section_layout() else {
        return Ok(summary);
    };

    let resolved = bundled_catalog().resolve(&layout);
    summary.weapon_name = bundled_catalog().weapon_name(&layout).map(str::to_owned);

    summary.manufacturer = resolved
        .root
        .and_then(|root| root.manufacturer)
        .map(str::to_owned);

    summary.item_type = resolved.root.map(|root| root.family.to_owned());

    summary.level_candidate = Some(resolved.level_candidate);

    summary.parts = resolved
        .parts
        .iter()
        .map(|part| ItemPartSummary {
            index: part.part.index,
            name: part
                .record
                .and_then(|record| record.display_name)
                .map(str::to_owned),
            category: part
                .record
                .and_then(|record| record.category)
                .map(str::to_owned),
            internal_name: part
                .record
                .and_then(|record| record.internal_name)
                .map(str::to_owned),
        })
        .collect();

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspects_known_shotgun() {
        let serial = "@Ugd77*FnkbUIz`YKRG/`es4}RMsG6vgs4=KLs6VJss9LCU2m";

        let summary = inspect_item(serial).expect("serial should decode");

        assert_eq!(summary.serial, serial);
        assert_eq!(summary.manufacturer.as_deref(), Some("Ripper"));
        assert_eq!(summary.item_type.as_deref(), Some("Shotgun"));
        assert_eq!(summary.level_candidate, Some(60));
        assert_eq!(summary.parts.len(), 14);
        assert_eq!(summary.parts[0].index, 19);
        assert_eq!(summary.weapon_name.as_deref(), Some("Golden God"));

        println!("{summary:#?}");
    }

    #[test]
    fn rejects_missing_prefix() {
        assert_eq!(inspect_item("invalid"), Err(ItemSerialError::InvalidPrefix),);
    }

    #[test]
    fn rejects_malformed_base85_after_a_valid_marker() {
        for serial in ["@U", "@Ua", "@Uabc ~", "@Uabcé", "@U~~~~~"] {
            assert_eq!(
                inspect_item(serial),
                Err(ItemSerialError::InvalidTextEncoding),
                "expected malformed Base85 to fail: {serial:?}",
            );
        }
    }

    #[test]
    fn valid_tokens_without_the_known_layout_keep_the_original_serial() {
        // Fixed synthetic wire vector: magic, VarInt(42), hard separator.
        // It is a valid token stream, without the six-number item layout.
        let serial = "@UglGW";
        let summary = inspect_item(serial).expect("valid synthetic token stream");

        assert_eq!(
            summary,
            ItemSummary {
                serial: serial.to_owned(),
                weapon_name: None,
                manufacturer: None,
                item_type: None,
                level_candidate: None,
                parts: Vec::new(),
            }
        );
    }

    #[test]
    fn unknown_catalog_entries_keep_the_level_and_numeric_part_index() {
        // Fixed synthetic vector: 65535,0,1,60|2,2125||{65535}|.
        // Both the root and part key are absent from the bundled catalog.
        let serial = "@Ug#7=X0VWteh%lWZ=>Pu-";
        let summary = inspect_item(serial).expect("valid synthetic item layout");

        assert_eq!(summary.serial, serial);
        assert_eq!(summary.manufacturer, None);
        assert_eq!(summary.weapon_name, None);
        assert_eq!(summary.item_type, None);
        assert_eq!(summary.level_candidate, Some(60));
        assert_eq!(
            summary.parts,
            vec![ItemPartSummary {
                index: 65535,
                name: None,
                category: None,
                internal_name: None,
            }]
        );
    }

    #[test]
    fn known_root_with_an_unnamed_part_keeps_that_part_visible() {
        let summary = inspect_item("@Ugd77*FnkbUIz`YKRG/`es4}RMsG6vgs4=KLs6VJss9LCU2m")
            .expect("known serial");

        assert_eq!(summary.item_type.as_deref(), Some("Shotgun"));
        assert_eq!(
            summary.parts[1],
            ItemPartSummary {
                index: 2,
                name: None,
                category: Some("Body".into()),
                internal_name: Some("BOR_SG.part_body".into()),
            }
        );
        assert_eq!(summary.parts.len(), 14);
    }
}
