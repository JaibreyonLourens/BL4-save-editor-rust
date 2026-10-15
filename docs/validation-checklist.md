# Validation and acceptance checklist

Use this as a completion record for the [implementation guide](implementation-guide.md).
Unchecked items are requirements, not claims of existing behavior.

Current evidence is recorded in the [agent handoff](agent-handoff.md). Passing
byte-level tests does not close document-preservation or game-compatibility gates.

## Before accepting BL4-004

- [ ] Every format claim identifies reproducible format evidence.
- [ ] Container/key golden results are independently checked and reproducible.
- [ ] Positive and malformed fixtures have expected outcomes and provenance.
- [ ] Item forms/header/encoding examples are recorded or explicitly unresolved.
- [ ] Real saves and their decoded outputs/account details remain private.
- [ ] Parser, classification and format-variant unknowns are listed.

## Before accepting BL4-005 through BL4-007

- [ ] Steam/Epic key vectors match expected bytes and invalid Steam IDs are rejected.
- [ ] Strict padding, trailer, zlib stream, checksum and length validation pass.
- [ ] Input, output and document parsing bounds are enforced before excessive work.
- [ ] Every malformed fixture fails without panic or a partially installed session.
- [ ] Full-tree tests preserve unknown nodes, tags, scalar types and sequence order.
- [ ] Unsupported YAML structures are rejected instead of silently discarded.
- [ ] Character/profile paths and classification rules are backed by sample evidence.
- [ ] Original ciphertext/document and source fingerprint are retained.
- [ ] Inspection and filtering leave the source, document and dirty state unchanged.
- [ ] Core has no GUI dependency and the desktop has no duplicate codec.

## Before accepting BL4-008 and the M2 gate

- [x] Both synthetic platforms reconstruct into valid containers at the byte level.
- [x] Private Steam YAML encoding and save reconstruction recover exact YAML bytes.
- [x] Private encoding tests preserve their inputs and save ignored artifacts/reports.
- [x] An unchanged-copy test preserves exact original encrypted bytes.
- [x] A separate test forces parse/dump/re-encode without the original-byte bypass.
- [x] Reconstructed output decrypts/reparses to the complete original semantic tree.
- [ ] A genuine Steam forced round trip passes the private in-game check below.
- [ ] Full core tests, formatting, Clippy and a Windows release build pass.
- [ ] Missing evidence and unsupported variants are included in the handoff.

If game access or a genuine sample is missing, the synthetic implementation may be
ready, but M2 remains pending. Do not label it game-compatible or proceed to dependent
write features on the strength of unit tests alone.

## Private in-game check for a forced round trip

1. Close the game before creating test copies. Preserve an untouched original outside
   the location used for game testing, and record its hash privately.
2. Record the game version, account format, character identity, currencies,
   representative equipped/backpack items and progression observations.
3. Produce output through the forced parse/dump/re-encode route into a separate file.
   Confirm that the untouched original's hash has not changed.
4. Decrypt/reparse the output and compare the complete semantic document with the
   original. Resolve any mismatch before trying it in-game.
5. Use the copied output in a controlled local game test. Record where it was placed
   and how the original will be restored; account for any enabled cloud synchronization
   so it cannot substitute another file during the test.
6. Load the character and inspect inventory, equipped gear, currencies and progression.
   Record any warning, missing item, changed state or load failure.
7. Exit normally, record the result and restore the preserved source copy if necessary.
   Store observations privately and publish only a sanitized evidence summary.

A game saving the reconstructed file may naturally change metadata/progression.
Keep the pre-load semantic comparison separate from post-game observations. A load
without a visible error is useful evidence but does not replace the inventory and
progression checks.

## Before accepting BL4-009

- [ ] Open a genuine Steam character and inspect supported summaries.
- [ ] Open a profile and an unknown document without enabling edits.
- [ ] Search, select and clear search without losing unsupported items.
- [ ] Cancel Open without changing the current session.
- [ ] Fail an Open request and retain the previous session with a clear error.
- [ ] Start two load requests and ensure an older result cannot replace the newer one.
- [ ] Keep the window responsive during reading/decompression/parsing.
- [ ] Confirm opening and inspecting writes neither source bytes nor backup files.
- [ ] Check resizing, selection details, keyboard focus and readable errors manually.
- [ ] Keep Save and Save As disabled.

## Required completion record

For each ticket, write its ID, implementation summary, fixture IDs/hashes, checks
and results, acceptance outcome, remaining limitations and next dependency. Include
game build/platform and sanitized manual-test results for compatibility claims.
Link private evidence by a non-identifying local case ID rather than exposing account
IDs or real save contents in committed documentation.

Only explicitly checked items are complete. Current core verification and historical
shell checks are recorded separately in [setup status](setup-status.md).
