#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use bl4_core::save::item_inspection::{ItemPartSummary, inspect_item};
use bl4_core::save::{
    container::{ContainerContext, Platform},
    inspection::InventoryEntry,
    session::SaveSession,
};
slint::include_modules!();

mod save_location;

const PREVIEW_SERIAL: &str = "@Ugd77*FnkbUIz`YKRG/`es4}RMsG6vgs4=KLs6VJss9LCU2m";

fn character_view(session: &SaveSession) -> CharacterView {
    let summary = session.working_document().character_summary();
    let number = |value: Option<u64>| -> slint::SharedString {
        value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "Unavailable".into())
            .into()
    };
    CharacterView {
        name: summary
            .as_ref()
            .and_then(|summary| summary.name)
            .unwrap_or("Unavailable")
            .into(),
        class_name: summary
            .as_ref()
            .map(|summary| summary.class)
            .unwrap_or("Unavailable")
            .into(),
        level: number(summary.as_ref().and_then(|summary| summary.level)),
        cash: number(summary.as_ref().and_then(|summary| summary.cash)),
        eridium: number(summary.as_ref().and_then(|summary| summary.eridium)),
    }
}

fn part_label(part: &ItemPartSummary) -> String {
    if let Some(name) = &part.name {
        return format!("{}: {name}", part.index);
    }
    let label = match (&part.category, &part.internal_name) {
        (Some(category), Some(internal)) => format!("{category} · {internal}"),
        (Some(category), None) => category.clone(),
        (None, Some(internal)) => internal.clone(),
        (None, None) => return format!("Unknown part #{}", part.index),
    };
    format!("{}: {label}", part.index)
}

fn item_row(serial: &str) -> ItemRow {
    let summary = match inspect_item(serial) {
        Ok(summary) => summary,
        Err(error) => {
            return ItemRow {
                name: "Unknown item".into(),
                family: "Unavailable".into(),
                level: "Unavailable".into(),
                description: format!("Unable to decode: {error:?}\nSerial: {serial}").into(),
            };
        }
    };
    let family = summary.item_type.as_deref().unwrap_or("Unknown type");
    let fallback_name = match summary.manufacturer.as_deref() {
        Some(manufacturer) => format!("{manufacturer} {family}"),
        None => family.to_owned(),
    };
    let name = summary.weapon_name.as_deref().unwrap_or(&fallback_name);
    let parts = summary
        .parts
        .iter()
        .map(part_label)
        .collect::<Vec<_>>()
        .join("\n");
    ItemRow {
        name: name.into(),
        family: family.into(),
        level: summary
            .level_candidate
            .map(|level| level.to_string())
            .unwrap_or_else(|| "Unavailable".into())
            .into(),
        description: format!(
            "Manufacturer: {}\nParts:\n{parts}\n\nSerial: {}",
            summary.manufacturer.as_deref().unwrap_or("Unknown"),
            summary.serial
        )
        .into(),
    }
}

fn backpack_rows(session: &SaveSession) -> Option<Vec<ItemRow>> {
    let backpack = session.working_document().character_summary()?.backpack?;
    Some(entry_rows(&backpack))
}

fn equipped_rows(session: &SaveSession) -> Option<Vec<ItemRow>> {
    let equipped = session.working_document().character_summary()?.equipped?;
    Some(entry_rows(&equipped))
}

fn entry_rows(entries: &[InventoryEntry<'_>]) -> Vec<ItemRow> {
    entries
        .iter()
        .map(|entry| {
            let mut row = match entry.serial {
                Some(serial) => item_row(serial),
                None => ItemRow {
                    name: "Unknown item".into(),
                    family: "Unavailable".into(),
                    level: "Unavailable".into(),
                    description: "No supported serial in this inventory entry".into(),
                },
            };
            row.description = format!(
                "Slot: {}\n{}",
                entry.slot.unwrap_or("Unknown"),
                row.description
            )
            .into();
            row
        })
        .collect()
}

#[derive(Default)]
struct InventoryRows {
    backpack: Vec<ItemRow>,
    equipped: Vec<ItemRow>,
}

fn main() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    let loaded_session: Arc<Mutex<Option<SaveSession>>> = Arc::new(Mutex::new(None));
    let inventory = Arc::new(Mutex::new(InventoryRows {
        backpack: vec![item_row(PREVIEW_SERIAL)],
        equipped: Vec::new(),
    }));
    let load_request = Arc::new(AtomicU64::new(0));
    let search_inventory = Arc::clone(&inventory);
    let weak = window.as_weak();
    window.on_search_changed(move |query| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let query = query.to_lowercase();
        let Ok(inventory) = search_inventory.lock() else {
            return;
        };
        let rows: Vec<ItemRow> = inventory
            .backpack
            .iter()
            .filter(|item| {
                item.name.to_lowercase().contains(&query)
                    || item.family.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        window.set_items(Rc::new(slint::VecModel::from(rows)).into());
        let equipped: Vec<ItemRow> = inventory
            .equipped
            .iter()
            .filter(|item| {
                item.name.to_lowercase().contains(&query)
                    || item.family.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        window.set_equipped_items(Rc::new(slint::VecModel::from(equipped)).into());
        window.set_selected_index(-1);
    });
    let weak = window.as_weak();
    let open_request = Arc::clone(&load_request);
    window.on_open_save(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let mut dialog = rfd::FileDialog::new().add_filter("Borderlands 4 save", &["sav"]);
        if let Some(root) = save_location::default_save_root() {
            let directory =
                save_location::initial_directory(&root, window.get_account_id().as_str());
            if directory.is_dir() {
                dialog = dialog.set_directory(directory);
            }
        }
        if let Some(path) = dialog.pick_file() {
            open_request.fetch_add(1, Ordering::Relaxed);
            if !window.get_manual_account() {
                window.set_account_id(
                    save_location::account_from_path(&path)
                        .unwrap_or_default()
                        .into(),
                );
            }
            window.set_selected_path(path.to_string_lossy().as_ref().into());
            window.set_status(
                "Save selected. Check the account ID and platform, then click Load selected save."
                    .into(),
            );
        }
    });

    let weak = window.as_weak();
    let session_state = Arc::clone(&loaded_session);
    let load_inventory = Arc::clone(&inventory);
    window.on_load_save(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };

        let path = window.get_selected_path().to_string();
        let account_id = window.get_account_id().to_string();

        if path.is_empty() || account_id.trim().is_empty() {
            window.set_status("Select a save and enter its account ID".into());
            return;
        }

        let platform = match window.get_platform_index() {
            0 => Platform::Steam,
            _ => Platform::Epic,
        };

        window.set_status("Loading save…".into());
        let request_id = load_request.fetch_add(1, Ordering::Relaxed) + 1;

        let weak = window.as_weak();
        let session_state = Arc::clone(&session_state);
        let load_inventory = Arc::clone(&load_inventory);
        let load_request = Arc::clone(&load_request);

        std::thread::spawn(move || {
            let result = std::fs::read(&path)
                .map_err(|_| "Could not read the selected file".to_owned())
                .and_then(|bytes| {
                    SaveSession::open(
                        bytes,
                        ContainerContext {
                            account_id,
                            platform,
                        },
                    )
                    .map_err(|_| "Could not decode or parse the save".to_owned())
                });

            let _ = weak.upgrade_in_event_loop(move |window| {
                if load_request.load(Ordering::Relaxed) != request_id {
                    return;
                }
                match result {
                    Ok(session) => {
                        let rows = backpack_rows(&session);
                        let equipped = equipped_rows(&session);
                        let character = character_view(&session);
                        let status = format!(
                            "Save loaded. Backpack: {}; Equipped: {}",
                            rows.as_ref()
                                .map(|items| items.len().to_string())
                                .unwrap_or_else(|| "unavailable".into()),
                            equipped
                                .as_ref()
                                .map(|items| items.len().to_string())
                                .unwrap_or_else(|| "unavailable".into())
                        );
                        let (Ok(mut stored), Ok(mut inventory)) =
                            (session_state.lock(), load_inventory.lock())
                        else {
                            window.set_status("Could not retain the loaded inventory".into());
                            return;
                        };
                        *stored = Some(session);
                        window.set_equipped_available(equipped.is_some());
                        *inventory = InventoryRows {
                            backpack: rows.unwrap_or_default(),
                            equipped: equipped.unwrap_or_default(),
                        };
                        drop(inventory);
                        drop(stored);
                        window.set_character(character);
                        window.set_has_loaded_save(true);
                        window.set_current_page(1);
                        window.set_search_query("".into());
                        window.invoke_search_changed("".into());
                        window.set_status(status.into());
                    }
                    Err(message) => window.set_status(message.into()),
                }
            });
        });
    });
    window.invoke_search_changed("".into());
    window.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_session(yaml: &str) -> SaveSession {
        SaveSession::new(
            vec![1, 2, 3],
            bl4_core::save::document::SaveDocument::parse(yaml.as_bytes()).unwrap(),
            ContainerContext {
                account_id: "72623859790382856".into(),
                platform: Platform::Steam,
            },
        )
    }

    #[test]
    fn character_view_displays_summary_fields_without_narrowing_currency_values() {
        let session = synthetic_session(
            "state: {char_guid: id, class: hunter, char_name: Hero, experience: [{type: Character, level: 60}], currencies: {cash: 0, eridium: 18446744073709551615}}",
        );
        let view = character_view(&session);
        assert_eq!(view.name.as_str(), "Hero");
        assert_eq!(view.class_name.as_str(), "hunter");
        assert_eq!(view.level.as_str(), "60");
        assert_eq!(view.cash.as_str(), "0");
        assert_eq!(view.eridium.as_str(), "18446744073709551615");
        assert!(!session.is_dirty());
    }

    #[test]
    fn character_view_leaves_unavailable_fields_explicit() {
        let session = synthetic_session("inputprefs: {}\nonlineprefs: {}\ndomains: {}");
        let view = character_view(&session);
        for field in [
            view.name,
            view.class_name,
            view.level,
            view.cash,
            view.eridium,
        ] {
            assert_eq!(field.as_str(), "Unavailable");
        }
        let partial = character_view(&synthetic_session("state: {char_guid: id, class: hunter}"));
        assert_eq!(partial.class_name.as_str(), "hunter");
        assert_eq!(partial.name.as_str(), "Unavailable");
        assert_eq!(partial.level.as_str(), "Unavailable");
    }

    #[test]
    fn backpack_rows_include_known_invalid_and_missing_serial_entries_without_mutation() {
        let session = synthetic_session(&format!(
            "state:\n  char_guid: synthetic\n  class: hunter\n  inventory:\n    items:\n      backpack:\n        slot_0: {{serial: '{PREVIEW_SERIAL}'}}\n        slot_1: {{serial: 'invalid'}}\n        slot_2: null\n"
        ));
        let original = session.working_document().clone();
        let rows = backpack_rows(&session).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].name.as_str(), "Golden God");
        assert!(rows[0].description.contains("Slot: slot_0"));
        assert_eq!(rows[1].name.as_str(), "Unknown item");
        assert!(rows[1].description.contains("Serial: invalid"));
        assert_eq!(rows[2].name.as_str(), "Unknown item");
        assert!(rows[2].description.contains("Slot: slot_2"));
        assert_eq!(session.working_document(), &original);
        assert!(!session.is_dirty());
        assert_eq!(session.revision(), 0);
        assert_eq!(session.original_ciphertext(), &[1, 2, 3]);
    }

    #[test]
    fn known_empty_backpack_is_distinct_from_an_unavailable_view() {
        let empty = synthetic_session(
            "state: {char_guid: id, class: hunter, inventory: {items: {backpack: {}}}}",
        );
        assert!(backpack_rows(&empty).unwrap().is_empty());
        for yaml in [
            "state: {char_guid: id, class: hunter}",
            "inputprefs: {}\nonlineprefs: {}\ndomains: {}",
        ] {
            assert!(backpack_rows(&synthetic_session(yaml)).is_none());
        }
    }

    #[test]
    fn equipped_rows_are_separate_and_retain_the_equipment_slot() {
        let session = synthetic_session(&format!(
            "state:\n  char_guid: id\n  class: hunter\n  inventory:\n    items:\n      backpack: {{slot_0: {{serial: 'invalid'}}}}\n    equipped_inventory:\n      equipped:\n        weapon_1: [{{serial: '{PREVIEW_SERIAL}'}}]\n"
        ));
        let equipped = equipped_rows(&session).unwrap();
        let backpack = backpack_rows(&session).unwrap();
        assert_eq!(equipped.len(), 1);
        assert_eq!(equipped[0].name.as_str(), "Golden God");
        assert!(equipped[0].description.contains("Slot: weapon_1"));
        assert_eq!(backpack.len(), 1);
        assert_eq!(backpack[0].name.as_str(), "Unknown item");
        assert!(!session.is_dirty());
    }

    #[test]
    fn preview_row_uses_the_decoded_summary() {
        let row = item_row(PREVIEW_SERIAL);
        assert_eq!(row.name.as_str(), "Golden God");
        assert_eq!(row.level.as_str(), "60");
        assert!(row.description.contains("19: Golden God"));
        assert!(row.description.contains("2: Body · BOR_SG.part_body"));
        assert!(row.description.contains(PREVIEW_SERIAL));
    }

    #[test]
    fn failed_decoding_retains_a_visible_row_and_exact_serial() {
        let row = item_row("invalid serial");
        assert_eq!(row.name.as_str(), "Unknown item");
        assert_eq!(row.level.as_str(), "Unavailable");
        assert!(row.description.contains("Serial: invalid serial"));
    }

    #[test]
    fn part_labels_use_available_metadata_before_the_unknown_fallback() {
        let mut part = ItemPartSummary {
            index: 999,
            name: None,
            category: None,
            internal_name: None,
        };
        assert_eq!(part_label(&part), "Unknown part #999");
        part.internal_name = Some("synthetic.part".into());
        assert_eq!(part_label(&part), "999: synthetic.part");
        part.category = Some("Body".into());
        assert_eq!(part_label(&part), "999: Body · synthetic.part");
        part.name = Some("Friendly name".into());
        assert_eq!(part_label(&part), "999: Friendly name");
        part.name = None;
        part.internal_name = None;
        assert_eq!(part_label(&part), "999: Body");
    }
}
