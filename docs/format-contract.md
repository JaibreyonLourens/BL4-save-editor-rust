# Save-format contract (BL4-004, incomplete)

These are implementation observations, not evidence of in-game compatibility.

The supported format derives a 32-byte AES key from `PUBLIC_KEY`. Steam uses the
account number as an unsigned 64-bit little-endian integer, XORing its eight bytes
into the first eight public-key bytes. Epic trims the account string, encodes it as
UTF-16LE, and XORs up to the first 32 encoded bytes into the public key.

The container is AES-256-ECB ciphertext. A private Steam sample verified on 13
October 2026 shows that, after decryption and PKCS#7 removal, the final layout is
a complete zlib stream followed by the uncompressed byte count as little-endian
u32. The zlib stream's normal final four bytes are its Adler-32 checksum in
big-endian order; they are not a separate Steam trailer. Encoding compresses at
level 9, appends the length, pads to an AES block and encrypts. Epic still needs
its own genuine-save confirmation.

The implementation validates padding and checksums, bounds input and decompression
sizes, and rejects unsupported variants. Account parsing rejects malformed or
overflowing IDs.

Before completing BL4-004:

- Record the item alphabet, bit order, magic and all supported token/header forms
  using documented format evidence; include capacity/truncation cases.
- Generate independently checked synthetic Steam/Epic containers and expected keys,
  plaintext, checksums, lengths and malformed variants.
- Add item examples for simple/int/list parts, string roots and short headers.
- Acquire private character/profile samples and reference decoded outputs; record
  game version and account format locally.
- Verify actual YAML tags/scalars before selecting the parser.

Steam decoding and encoding are implemented. The user verified that exported YAML
loads in a separate compatible editor. Synthetic platform round trips and private
Steam encoding round trips pass; the user confirmed both newly encoded saves load
in a compatible editor. In-game compatibility remains pending. Serial fixtures and the complete independent
fixture catalog remain unfinished. The UI's demo rows are layout data only.

Use the [implementation guide](implementation-guide.md),
[fixture specification](fixture-specification.md) and
[validation checklist](validation-checklist.md) alongside this contract.

## Exact public-key bytes
The public key is 32 bytes, written here as hexadecimal:

35 EC 33 77 F3 5D B0 EA BE 6B 83 11 54 03 EB FB
27 25 64 2E D5 49 06 29 05 78 BD 60 BA 4A A7 87

Steam modifies positions 0 through 7 using the eight account-number bytes; the
remaining bytes stay unchanged. Epic UTF-16LE has no byte-order mark and is limited
to the first 32 encoded bytes for XOR, not the first 32 characters.

## Required strict decoding pipeline

1. Validate account/platform input and encrypted input size. Reject empty ciphertext
   and lengths not divisible by the 16-byte AES block size before decryption.
2. Derive the selected platform key and decrypt using AES-256-ECB.
3. Validate PKCS#7: the final byte is a count from 1 through 16 and every byte in
   that suffix equals the count. Remove exactly that suffix. Empty content cannot
   be indexed or accepted as padding.
4. Require at least eight unpadded bytes. Read the checksum from the last eight
   bytes as zlib's big-endian Adler-32 and split off only the final four-byte
   little-endian length.
5. Planned: reject a declared output length above the configured maximum before
   decompression. The current code bounds actual output and verifies length after
   decompression; this early rejection is still missing.
6. Decompress the remaining complete zlib stream as one bounded stream. Enforce the maximum
   while reading output, not after reading an unlimited buffer. Do not allocate
   unchecked memory based on trailer length.
7. Require stream completion and exact consumption of the compressed region. The
   initial strict policy rejects extra bytes and concatenated streams; genuine
   variants require separate investigation before adding support.
8. Require output length equal to the stored count and Adler-32 equal to the
   calculated checksum. Return a recoverable error on any mismatch.
9. Pass validated plaintext bytes to the document layer. Perform UTF-8 and YAML
   validation there without dropping invalid bytes or substituting replacement text.

The Adler-32 is zlib's own checksum. The declared length counts bytes, not Unicode
characters. The intended document encoding is UTF-8; the container codec preserves
its input bytes without validating text and uses zlib level 9.

If platform detection is added, each candidate must pass the entire validation
pipeline. Reject ambiguity. An explicit platform choice is sufficient initially.
No platform-marker prefix has been established by the format observations here.
Wrong account input and corrupted ciphertext can fail at the same stage; do not
present either cause as proven by a padding or decompression failure alone.

## Required encoding and preservation behavior

Byte-level encoding, full supported-document serialization and session preservation
are implemented. Private character reconstruction now includes parse/dump/reopen
semantic comparison; in-game acceptance remains pending.

For reconstruction, serialize the full supported document as UTF-8, check its byte
length is representable and within limits, zlib-compress it, append the
little-endian length, apply PKCS#7, and encrypt with the session's known key.
Always add padding, including a full block when the unpadded length is aligned.
Do not select a new platform from the destination filename.

For an unchanged copy, preserve exact original ciphertext. Test forced reconstruction
separately through parse/dump/re-encode, then decrypt/reparse and compare whole
semantic trees. Semantic equality includes scalar types, tags, unknown nodes and
sequence order; YAML formatting and compressed-byte equality are not required.

## Read-only inspection contract

`SaveDocument::kind()` recognizes the observed character shape only when
`state.char_guid` and `state.class` are nonblank strings. This is an application
policy based on private character evidence, not universal format detection.
Missing or unexpected identity types produce `Unknown`. The observed profile
shape requires untagged mappings at `inputprefs`, `onlineprefs` and `domains`, with
no `state` field. A complete profile shape combined with any `state` field is
ambiguous and remains `Unknown`, without character summaries. Profile recognition
is based on one private export; it does not prove support for every profile version
or its encrypted container. Profiles expose no character summaries or edit permission.

`character_summary()` returns a borrowed view only for recognized characters:

| Path | Expected type | Result |
| --- | --- | --- |
| `state.char_name` | string | Optional name |
| `state.class` | nonblank string | Exact class identifier |
| `state.experience` | sequence of plain mappings | Optional level from the unique `type: Character` record with an unsigned `level` |
| `state.currencies.cash` | unsigned integer | Optional cash |
| `state.currencies.eridium` | unsigned integer | Optional Eridium |
| `state.inventory.items.backpack` | mapping | Optional ordered inventory rows |
| backpack mapping key | string | Optional slot identifier |
| backpack entry `serial` | string | Optional exact serial text |

Every ancestor must be an untagged mapping. Locally tagged fields, wrong types and
missing fields remain unavailable; values are never coerced. Zero is an available
currency value. Unavailable inventory differs from a known empty mapping. Every
backpack entry remains a row, including malformed entries and non-string keys.
Serials are not decoded, trimmed or filtered. Unknown tree nodes remain intact.
Inspection does not change ciphertext, document content, revision or dirty state.
Level selection is independent of sequence order and ignores other well-typed
experience types. Duplicate `Character` records, wrong types or malformed record
identities make level unavailable; no bounds for editing are inferred.
No edit permission is implied by classification. Equipped pairing and flags await
further evidence. Supported YAML tag/numeric/merge and resource policies are
specified in [YAML preservation](yaml-preservation.md).

## Compatibility requirements

The Rust core must preserve valid bytes or reject invalid text. The planned read-only
Open operation performs no filesystem writes; backup creation belongs to later
persistence operations. The core accepts an explicit platform; any automatic
detection must use full validation and reject ambiguity.

## Decisions needed before accepting dependent tickets

| Decision | Required evidence or policy | Needed for |
| --- | --- | --- |
| Maximum encrypted/decompressed size | Record conservative numeric limits, rationale and boundary fixtures; use actual private sample sizes when available | BL4-006 |
| YAML nesting/alias expansion | Parser capabilities and bounded-resource policy | BL4-007 |
| Empty YAML, multiple documents, duplicate keys | Explicit supported semantics; reject unsupported inputs without silent loss | BL4-007 |
| Tags, aliases, non-string keys | Genuine tags plus candidate-parser preservation probes | BL4-007 |
| Character/profile discriminator | Exact reference paths and genuine decoded examples | BL4-007 |
| Name/currency/inventory paths | Path segments, owning document, expected type, optionality and fixture evidence | BL4-007/009 |
| Account normalization | Steam leading zeros and accepted Epic syntax, separate from byte derivation | BL4-005 |
| Error categories | Account, size, alignment, padding, zlib, checksum, length, text, YAML and unsupported cases | BL4-006/007 |

Limits are application policies, not game-format constants; record values before
implementing guards. Unknown fields alone are not grounds for rejection. Unsupported
representation and ambiguous classification are separate concerns. Do not invent
summary paths or writable-field bounds without sample evidence.

## Item contract still to establish

The original plan identifies custom Base85, per-byte bit reversal, seven-bit magic,
varint/varbit, simple/int/list parts, string roots and normal/short headers as research
targets. Exact constants/layouts are not established by the container source used
for these notes.

Investigate the item stream and document:

- Alphabet, byte grouping, partial groups and invalid-character policy.
- Bit order, magic value, token discriminators and termination/padding rules.
- Integer capacities/sign rules and string encoding/length bounds.
- Root identity, part layouts, part order, duplicates and attached values.
- Normal/short headers, level/seed fields and unknown structural fields.
- Representability checks and the ambiguous zero-length varbit case.
- Evidence for the reported truncation and ineffective `new_level` override.

Record exact source symbols and expected bytes/tokens in the item fixture records.
Keep unsupported serial strings intact. Do not guess missing rules to close a ticket.

## Compatibility status

Synthetic Steam/Epic round trips and a genuine Steam byte round trip pass. A private
Steam sample produces 125,156 YAML bytes; its ciphertext changes from 21,296 to
20,832 bytes after reconstruction while the YAML remains identical. These checks
alone do not prove in-game compatibility. The production session's character
parse/serialize/reconstruct/reopen path also passes semantic comparison. Profile
classification and full-tree YAML preservation pass against a private export;
encrypted profile reconstruction, genuine Epic samples and in-game round trips remain
pending. See the [agent handoff](agent-handoff.md) for code locations and test evidence.

Current size policies are 16 MiB uncompressed and 32 MiB ciphertext. Broader limit
boundary tests and independent key/container fixture coverage remain outstanding.
