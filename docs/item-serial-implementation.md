# Item serial implementation worksheet

Updated: 13 October 2026. This is a read-only item-codec milestone. It does not
authorize save editing, slot allocation, item creation, deletion, or level sync.

## Starting point

The core can preserve the complete supported YAML document, distinguish known
character/profile shapes, and expose raw backpack slot/serial strings. The
`save::item_serial` module now decodes the supported Base85/bitstream transport
into bounded, read-only tokens. It intentionally does not assign game meanings
to header values or part indexes.

The decoder also exposes `TwoSectionItemLayout` for the repeated public shape:
four numbers, a hard separator, two numbers, two hard separators, parts and a
final hard separator. Its `level_candidate()` is the fourth first-section value.
That is a display/research hint only, not authorization to change an item level.

`save::item_catalog` is the separate display-mapping boundary. A versioned
catalog maps a structural root ID to a broad family/manufacturer and maps a
`(root ID, part index)` pair to an optional display name. `bundled_catalog()`
now provides 71 root records and 7,050 part records from a pinned
community snapshot. Its exact game build remains unverified. Unknown records
remain visible as their numeric values and must
never make a serial invalid.

Weapon records also retain the upstream CSV's part category and internal
identifier. The desktop displays a friendly name when present, then category
and/or internal identifier, then `Unknown part #N`. A missing friendly name is
distinct from a missing catalog entry. Regenerate with
`python scripts/import_item_catalog.py docs/internal/catalog-upstream`.

The import includes native naming roles, rarity metadata and unique-accessory
flags. The weapon base-name resolver prefers named unique barrel accessories,
then Pearl/Legendary composition titles, then named barrels. Equally ranked
conflicting names remain unresolved. The desktop uses the base name when known
and keeps manufacturer/type as the fallback. Prefixes, exceptional weapon rules,
equipment titles, localization, foreign-root references and calculated stats
still require further resolver work. The desktop now displays loaded backpacks.
The catalog snapshot identifies its source revision; the current API does not
automatically verify that a save uses the same game build.

The documented character backpack location is:

```text
state.inventory.items.backpack.slot_N.serial
```

Equipped paths are not yet part of the Rust format contract. Establish them from
private copies before making them public summary fields. Do not add a recursive
"find every serial" fallback: that can confuse unknown or future structures with
player equipment.

## Implementation order

1. **Collect independent, redacted vectors.** For each vector, keep the original
   private save outside Git and record only non-sensitive expectations: whether
   decoding succeeds, container path category, expected item level, root form,
   and token/block count. Include malformed input and at least one unknown form.
2. **Extend the outer serial tests.** The current decoder validates the `@U`
   marker, the custom Base85 alphabet, partial final groups and a decoded-size
   cap. Add independent vectors for every accepted final-group length and each
   rejection outcome. Do not replace it with a generic Base85 decoder.
3. **Extend the structural parser.** The current bounded cursor parses the magic
   header, separators, VarInt, VarBit (including zero), all three documented part
   forms, and ASCII strings. Translate
   every supported token into `SerialBlock`. Reject truncation, overlong integer
   encodings and impossible lengths without panicking. Preserve structurally
   valid-but-unknown tokens as `Unknown` only when their size boundary is known.
4. **Interpret verified headers.** The current two-section view preserves the
   observed fourth-first-section level candidate. Establish item root, category
   and verified level semantics only after private fixtures cover each supported
   item family. Other structures must remain raw tokens.
5. **Populate read-only display mapping.** Keep identifier-to-name/rarity/type
   lookup data in `item_catalog`, separate from parsing. Add a catalog only with
   its game version and independently verified root/part expectations. An
   unknown root must display a stable numeric fallback, never an invented name.
6. **Verify equipped paths.** Add private tests for exact equipped locations and
   field types. Then add an optional `equipped` summary, keeping unavailable
   separate from a known empty set.
7. **Only then consider encoding.** Build it separately, prove decode/encode
   expectations with fixed vectors, reconstruct copied saves, and perform the
   private in-game acceptance gate before enabling mutation controls.

## Suggested small commits

| Commit | Deliverable | Required tests |
| --- | --- | --- |
| `test(core)` | Redacted serial fixture harness | Valid, malformed and bounded-size cases |
| `test(core)` | Extend Base85 coverage | Fixed expected decoded bytes; no panics |
| `test(core)` | Extend token-parser coverage | Every token form, truncation and overflow |
| `feat(core)` | Verified header interpretation | Known level/root plus unsupported version |
| `feat(core)` | Read-only item display lookup | Known and unknown root fallback |
| `test(core)` | Private equipped-path evidence | No serial/YAML values printed; source unchanged |

## Guardrails

- Keep serials and decoded private content out of committed fixtures, test
  failures and ordinary logs.
- Do not make a serial editable because it parses. Parsing proves only the
  tested read-only representation.
- Do not use summary records to reconstruct YAML; `SaveDocument` remains the
  complete source of truth.
- Enforce a maximum serial text length, decoded byte length, block count and
  list length before allocating or recursing. Put the chosen values beside tests
  that prove their acceptance boundaries.
- Prefer `Result` over fallback values. `None`, unknown display data and a parse
  error have different meanings and should remain distinguishable.

## Definition of done: read-only serial inspection

- Public synthetic golden vectors test both successful and rejected inputs.
- Private tests verify at least one observed backpack item and the equipped-path
  shape without exposing values.
- Unknown item forms remain visible as their exact raw serial with an unavailable
  decoded summary.
- Formatting, core tests and Clippy pass.
- The desktop still has no serial mutation or production save controls enabled.
