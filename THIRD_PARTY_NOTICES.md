# Third-party catalog notices

The bundled root mappings and English part names are adapted from
[SuperExboom/Borderlands4-SaveEditor](https://github.com/superexboom/Borderlands4-SaveEditor),
revision `d47f5898c4c9ea1b8722098363330eae623c7d9e`.
The upstream project credits Nicnl and InflamedSebi for serial research, and
Whiteshark-2022 and Mattmab for class-mod/enhancement data and related work.

The upstream project is distributed under GNU AGPL version 3; its full license
is retained in `resources/catalog/LICENSE.upstream.txt`. The conversion was
performed on 13 October 2026 using `scripts/import_item_catalog.py`. Changes
select numeric root/part keys and English display names into Rust records.
On 15 October 2026 the import was extended with the English weapon-part CSV's
part categories and internal identifiers, joined by root ID and part ID.
Existing name-index titles take precedence over CSV names.
The import also retains native naming roles, rarity labels and unique barrel
accessory flags for read-only weapon base-title resolution. The resolver's
priority follows the pinned editor's naming approach; prefixes and exceptional
per-weapon rules are not yet reproduced.
No upstream Python application code runs in this application.

These tables include community-curated and game-derived content. The upstream
software license is not a claim of ownership of Gearbox/2K game content.
The snapshot's exact game build has not been independently verified.

Input hashes, revision and conversion details are in `resources/manifest.json`.
Unknown values remain unresolved; catalog lookup is not an item-legality check.
