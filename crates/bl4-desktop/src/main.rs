#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::rc::Rc;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    let inventory = Rc::new(bl4_core::demo_inventory());
    let weak = window.as_weak();
    window.on_search_changed(move |query| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let query = query.to_lowercase();
        let rows: Vec<ItemRow> = inventory
            .iter()
            .filter(|item| {
                item.name.to_lowercase().contains(&query)
                    || item.family.to_lowercase().contains(&query)
            })
            .map(|item| ItemRow {
                name: item.name.clone().into(),
                family: item.family.clone().into(),
                level: item.level as i32,
                description: item.description.clone().into(),
            })
            .collect();
        window.set_items(Rc::new(slint::VecModel::from(rows)).into());
        window.set_selected_index(-1);
    });
    window.invoke_search_changed("".into());
    window.run()
}
