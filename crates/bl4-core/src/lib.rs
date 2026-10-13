//! Save-format and editing logic, independent of the desktop UI.
//!
//! Save-container operations currently return explicit unfinished errors until
//! their validation and preservation fixtures are implemented.

pub mod save;
/// Presentation-neutral inventory data. Demo entries are never real serials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventorySummary {
    pub name: String,
    pub family: String,
    pub level: u32,
    pub description: String,
}

/// Synthetic examples for exercising selection, search and layout.
pub fn demo_inventory() -> Vec<InventorySummary> {
    [
        (
            "Demo assault rifle",
            "Weapon",
            12,
            "Synthetic weapon entry. Part decoding arrives with the item codec.",
        ),
        (
            "Demo shield",
            "Shield",
            10,
            "Synthetic shield entry. No game serial or save data is attached.",
        ),
        (
            "Demo class mod",
            "Class mod",
            8,
            "Synthetic equipment entry. Editing is unavailable in this shell.",
        ),
    ]
    .into_iter()
    .map(|(name, family, level, description)| InventorySummary {
        name: name.into(),
        family: family.into(),
        level,
        description: description.into(),
    })
    .collect()
}
