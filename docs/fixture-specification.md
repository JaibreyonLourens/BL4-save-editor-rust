# Fixture specification

Use with the [implementation guide](implementation-guide.md) and
[format contract](format-contract.md). All cases below are required work, not
fixtures already present in the repository.

## Storage and provenance

Proposed public location: `fixtures/synthetic/`, grouped into container, document
and serial cases. This location contains generated artificial inputs only. The
current ignore rules exclude `*.sav` globally, so use descriptive `.bin` files for
synthetic ciphertext or deliberately review a narrow ignore exception later.
Do not weaken the private-save exclusions.

Private location: `fixtures/private/`. Keep genuine saves, decoded trees, account
IDs, backups, reference outputs and game screenshots there or in another private
location outside the repository. That directory is ignored except its README.
Check Git's ignored-file behavior before adding samples.

Every public fixture needs a record containing:

| Field | Required content |
| --- | --- |
| Identity | Stable case ID, category and purpose |
| Origin | Synthetic or genuine; document the generating tool/version where relevant |
| Inputs | Artificial account/platform, exact plaintext bytes and generation settings |
| Files | Relative artifact paths, byte lengths and SHA-256 hashes |
| Expected success | Key bytes, trailer values, plaintext or semantic document as applicable |
| Expected failure | Failure stage/category and why that stage is reachable |
| Reproduction | Generator/tool version, settings and independent verification method |
| Status | Draft, independently checked, or awaiting evidence |

Record real account IDs only in private metadata. Synthetic examples should use
clearly artificial IDs. Do not label a fixture independently checked when its
expectations were produced only by the implementation under test.

## Golden results and malformed cases

For the first known-good container, document each intermediate boundary: key,
compressed bytes, trailer bytes, padded plaintext and ciphertext. Compare these
against a separate tool/reference calculation. Then check the Rust decoder against
the stored expected plaintext. Cross-check checksum and length manually or with an
independent tool; decoding by another editor alone is insufficient for checksum validation.

Build malformed fixtures so they isolate the intended validation. A random encrypted
byte flip usually fails padding or decompression first and does not reliably test a
trailer mismatch. To test that mismatch, build valid compressed data with a deliberately
wrong trailer and valid padding, then encrypt it with the correct synthetic key.

For boundary tests, use reduced configured limits and small artifacts where possible
instead of committing large decompression payloads. Record the limit associated
with each case.

## Key and container matrix

| Case | Input or variation | Expected result |
| --- | --- | --- |
| KEY-01 | Steam ID with distinct little-endian bytes | Exact independently checked 32-byte key |
| KEY-02 | Steam u64 range boundaries, including overflow | Representable numeric inputs derive correctly; overflow rejected; account plausibility tracked separately |
| KEY-03 | Empty/whitespace/non-ASCII digits/signs/embedded punctuation | Rejected under the documented Steam parser policy |
| KEY-04 | Epic artificial ASCII ID | Expected UTF-16LE XOR result |
| KEY-05 | Epic encoding boundary at and beyond 32 bytes | Only the first 32 encoded bytes affect the key |
| KEY-06 | Synthetic non-ASCII Epic derivation input | Confirms UTF-16LE behavior at the primitive level; does not establish accepted real account syntax |
| CON-01 | Valid Steam container | Exact expected YAML bytes and little-endian trailer checksum |
| CON-02 | Valid synthetic Epic container | Exact expected YAML bytes and big-endian trailer checksum |
| CON-03 | Payload size requiring a full PKCS#7 block | Padding added and removed correctly |
| CON-04 | Empty ciphertext or non-multiple-of-16 length | Rejected before decryption |
| CON-05 | Padding length 0, greater than 16 or inconsistent bytes | Rejected as invalid padding |
| CON-06 | Correctly padded decrypted content shorter than eight bytes | Rejected as missing trailer |
| CON-07 | Truncated/invalid zlib stream with otherwise valid wrapper | Rejected during decompression |
| CON-08 | Valid zlib stream, wrong additional Adler-32 | Rejected by checksum validation |
| CON-09 | Valid zlib stream/checksum, wrong length | Rejected by length validation |
| CON-10 | Wrong checksum byte order for chosen platform | Rejected unless independently documented as a supported variant |
| CON-11 | Wrong account or platform key | Recoverable validation failure; no panic or partial session |
| CON-12 | Encrypted input above configured maximum | Rejected before large allocation/decryption |
| CON-13 | Decompression exceeds maximum or declared length | Stops at the enforced bound, without unbounded output |
| CON-14 | Extra bytes between zlib end and trailer, or second zlib member | Rejected by the initial strict one-stream policy |
| CON-15 | Valid reconstructed container for each synthetic platform | Decrypts to expected bytes; no claim of genuine Epic compatibility |

Keep valid padding coverage broad: test one-byte, intermediate and full-block
padding. Use an asymmetric checksum value for byte-order cases so the test cannot
accidentally pass in either order.

## Document preservation matrix

| Case | Content | Expected result |
| --- | --- | --- |
| DOC-01 | Unknown nested mappings/sequences beside known fields | Unknown content retained through parse/dump/reparse |
| DOC-02 | Strings, integers, floats, booleans, nulls and empty collections | Supported scalar/collection types preserved distinctly |
| DOC-03 | Quoted numeric-looking strings, Unicode and multiline strings | String value/type preserved; no lossy character replacement |
| DOC-04 | Explicit tags, including nested tags seen in genuine samples | Tag identity and associated value preserved, or document rejected as unsupported |
| DOC-05 | Anchors/aliases, duplicate keys and non-string keys | Behavior explicitly recorded; no silent data loss |
| DOC-06 | Invalid UTF-8, malformed YAML, excessive nesting or multiple documents | Recoverable rejection under documented parsing limits/policy |
| DOC-07 | Confirmed character shape, confirmed profile shape, unknown shape | Evidence-based classification; unknown stays unknown |
| DOC-08 | Missing or wrong-type summary fields | Unavailable summary without losing original nodes |
| DOC-09 | Unknown/undecodable inventory serial string | Exact string retained and item remains visible |
| DOC-10 | Open, selection and filtering | Original document/source bytes unchanged; dirty state remains false |

Compare whole trees using the agreed semantic model. Do not reduce comparison to
the name, currency and item-count summaries. Formatting/comments need not be
preserved for a forced round trip, but values, types, tags and sequence order must
be. If aliases cannot be represented without altering behavior, reject that case
until the parser policy is resolved.

## Item examples to research in BL4-004

These prepare M5; they do not require implementing the item codec during M2.

| Case | Required example/evidence |
| --- | --- |
| ITEM-01 | Custom Base85 alphabet and one complete known token stream |
| ITEM-02 | Per-byte bit reversal and seven-bit magic at verified boundaries |
| ITEM-03 | Simple part, part with integer value and part with list value |
| ITEM-04 | String root and numeric root examples |
| ITEM-05 | Normal header and short level-one header with expected level/seed |
| ITEM-06 | Varint/varbit boundary values and values exceeding capacity |
| ITEM-07 | Truncated input, unknown token and invalid string/alphabet input |
| ITEM-08 | Repeated parts, part order and attached values retained |
| ITEM-09 | Ambiguous zero-length varbit example, if evidence can be found |

Record original serial, decoded bytes/tokens, field interpretation and reference
symbols. Define which cases require binary equality and which intentionally permit
canonical encoding with semantic equality. A documented truncation case is
a negative compatibility case, not the desired expected output.

## Private samples required for milestone acceptance

For each genuine sample record a private identifier, account/platform, game build,
character/profile classification, capture date, original hash and reference output.
Capture character name, currencies, representative inventory/equipment and progression
observations to support the later in-game comparison.

Minimum initial evidence is a Steam character forced round trip and a profile sample
for classification/preservation. Epic needs its own genuine fixtures and in-game
checks before advertised support. Missing samples must remain visible in the milestone
status; synthetic fixtures cannot replace those acceptance checks.
