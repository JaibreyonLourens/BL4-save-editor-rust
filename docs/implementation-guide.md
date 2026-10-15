# Implementation guide: from the shell to real save inspection

Updated: 15 October 2026. Read [project status](setup-status.md) for current
implementation and evidence, then use this guide for the remaining milestones.

## Current starting point

The workspace and read-only Slint desktop exist. The UI displays loaded character
summaries and separate equipped/backpack groups with serial inspection and supported
weapon base names. The core decodes and encodes containers, preserves supported YAML
documents and retains original/working sessions. Private Steam byte round trips pass.
The local build/check results and remaining setup checks are recorded in
[setup status](setup-status.md).

The next deliverable is broader real-save/item evidence and naming coverage.
Both encoded saves passed the user's editor check; in-game checks remain pending.
Parser policies and preservation findings are recorded
in [YAML preservation](yaml-preservation.md).
The read-only UI is implemented; that does not close the game-compatibility gate.
Keep Save and Save As disabled until the separate character-editing milestone.

Read these documents in order:

1. This guide: implementation order, responsibilities and ticket handoffs.
2. [Format contract](format-contract.md): known container behavior and open questions.
3. [Fixture specification](fixture-specification.md): required samples and expected results.
4. [Validation checklist](validation-checklist.md): evidence needed to accept each gate.

## What you need before implementing

| Requirement | Current state | Action |
| --- | --- | --- |
| Rust, Windows C++ tools and SDK | Installed; shell builds locally | Use the existing workspace and development script |
| Format research notes | Available locally | Use the established format contract and private samples |
| Container specification | Steam layout verified | Use the corrected zlib-plus-four-byte-length layout |
| Synthetic fixtures | In-code vectors and round trips present | Complete the independent fixture catalog and remaining coverage |
| Real Steam save | Private sample tested | Keep it private; record game build and classification locally |
| Real profile save | Private YAML export verified | Encrypted profile fixture and game acceptance still needed |
| Real Epic save | Not supplied | Synthetic codec work can proceed; genuine Epic support stays unverified |
| Independent decoded output | User confirmed exported YAML and both generated saves load in a compatible editor | Complete parse/serialize and in-game checks |
| In-game access | Not established | Required to accept the forced round-trip gate |

Keep private format research and fixtures outside version control. Do not add
external editor tooling as an application runtime dependency.

## Scope and architecture

Keep format logic in `crates/bl4-core`. The desktop crate handles dialogs, visible
state and background jobs. Both decoding and encoding must have one implementation
in the core. Work on item serials is a separate track: initial inventory inspection
can preserve serial strings and show fallback names without decoding their parts.

The following are proposed module responsibilities, not existing APIs:

| Planned area | Inputs | Outputs and responsibility |
| --- | --- | --- |
| Core error model | Failure context | Structured recoverable failures; no account IDs or save contents in ordinary messages |
| Save container | Encrypted bytes, validated account/platform, limits | Validated YAML bytes and platform metadata; reconstruction of validated containers |
| Save document | YAML bytes | Complete generic tree, classification and optional summaries |
| Save session | Original file bytes and metadata, full document | Original/working state, revision, source fingerprint and dirty state |
| Desktop worker | Open request and request ID | Completed inspection or recoverable failure, associated with the originating request |
| Desktop presentation | Core summaries and status | Character/inventory rows and details; no arbitrary YAML mutations |

Retain unknown content in the full tree. Summary records are views of that tree;
they must never become the only source used for reconstruction.

Add dependencies as their implementation ticket needs them. The codec already uses
`aes` 0.9.3, `flate2` 1.1.10 and `adler2` 2.0.1, with direct ECB block handling and
PKCS#7 validation. `adler2` verifies the zlib checksum; no additional checksum is
appended. `serde_yaml_ng` is used behind event/scanner validation and preservation tests.
Native file dialogs such as `rfd` are needed at BL4-009, not for the codec itself.
Update the application lockfile whenever dependencies change.

## BL4-004: establish the contract and fixtures

**Depends on:** the existing workspace. **Produces:** research notes, fixture
metadata and golden expected results, before dependent format code is accepted.

Record reproducible observations for the container, document behavior and item
stream. Avoid relying on memory or assumed field names.

Separate three kinds of evidence: observed compatible-editor behavior, proposed
Rust behavior, and genuine-save/game verification. A compatible editor accepting a
file does not prove every integrity check is valid.

Complete the [fixture matrix](fixture-specification.md), including expected keys,
plaintext bytes, trailer fields and malformed outcomes. Document unresolved item
forms explicitly rather than inventing their encoding. Container tickets can
proceed once their subset is ready, but BL4-004 as a whole stays open until its
item examples and provenance are also ready.

**Accepted when:** another developer can reproduce the expectations from the
documented format observations and fixture inputs; synthetic and genuine samples are clearly
distinguished; unverified cases have explicit status.

## BL4-005: platform account parsing and key derivation

**Depends on:** the container/key subset of BL4-004. **Produces:** pure core account
validation and Steam/Epic key derivation, tested against golden bytes.

Make platform selection explicit in the core. Steam parsing accepts trimmed ASCII
decimal input that fits an unsigned 64-bit integer; do not remove arbitrary
characters or use a digit-count check instead of numeric range validation. Treat
leading-zero behavior as an explicit normalization decision. Epic derivation uses
trimmed text and UTF-16LE bytes; verify the exact accepted ID syntax separately
from that byte algorithm. Do not invent Epic account syntax from synthetic tests.

Use an asymmetric synthetic Steam value so byte order errors are visible. Epic
tests must distinguish UTF-16LE from UTF-8 and cover the 32-byte XOR cutoff.
Test malformed input and overflow without panics. Do not expose derived keys to
the desktop presentation model or normal logs.

**Accepted when:** independent expected-key cases pass and invalid input returns
structured errors. This proves the algorithm for the tested inputs, not genuine
Epic support.

## BL4-006: strict container decoding

**Depends on:** BL4-005 and container fixtures. **Produces:** validated plaintext
bytes, without requiring a GUI or YAML parser.

Implement the pipeline in the [format contract](format-contract.md). Enforce size
limits before allocations where possible, validate AES block alignment and padding,
remove only the four-byte length, and require a complete bounded zlib stream.
Compare the zlib checksum and the byte length. Do not retry malformed data with relaxed
padding or disabled checksums.

Keep low-level diagnostics internally, but avoid claiming that every failure proves
a wrong account ID. Corruption and wrong keys can fail at the same stage.
Automatic platform detection, if exposed, must attempt only documented candidates,
apply the same full validation to each, and reject ambiguity. Explicit platform
input is sufficient for the initial core work.

**Accepted when:** positive fixtures recover the exact expected plaintext and all
negative cases fail safely with bounded resource use. Unsupported variants stay
unsupported until investigated and documented.

## BL4-007: document preservation and session state

**Depends on:** BL4-006 and YAML fixtures. **Produces:** complete document parsing,
conservative classification, summaries and a session that retains original bytes.

First probe the candidate YAML parser with unknown fields, explicit tags, scalar
types, nested sequences/mappings and real decoded outputs. Decide how aliases,
duplicate keys and non-string mapping keys are handled before claiming preservation.
Reject cases the representation cannot preserve. Do not silently normalize away
unsupported structures.

Discover actual character/profile discriminator and summary paths from genuine
samples. Record each path, expected type and owning document kind in
the contract. Missing or unexpected paths produce an unavailable summary or unknown
classification, not a guessed editable character. Profiles remain read-only.

Retain original ciphertext, original document, working document, platform/account
context and a source fingerprint. Opening, inspecting, searching and selecting
must leave dirty state false. Define revisions now so later asynchronous jobs can
be checked against the state that produced them.

**Accepted when:** whole-tree semantic comparisons preserve supported tags/types and
unknown nodes, classification is evidence-based, and unsupported documents remain
intact without writable presentation fields.

## BL4-008: encoding and forced round-trip verification

Current status: byte-level encoding, synthetic platform round trips and two private
Steam encoding tests are implemented and pass. The remaining work below includes
YAML parse/dump preservation and manual game compatibility. Those gates stay open.

**Depends on:** BL4-006 and BL4-007. **Produces:** reconstruction through the core,
plus compatibility evidence on a copied genuine Steam save.

Encode YAML bytes, zlib-compress them, append only the four-byte little-endian length, apply
PKCS#7 and encrypt. For unchanged-copy behavior, retain and return original bytes.
Test reconstruction separately: parse, serialize, compress and encrypt even when
the session is unchanged. Otherwise an original-byte bypass could conceal bugs.

Decrypt and reparse reconstructed output, compare the complete document tree, and
run the [private in-game checklist](validation-checklist.md). Byte-identical encrypted
output is not required for a forced round trip because YAML formatting and compression
can differ; preserved document meaning and game acceptance are required.

Produce round-trip output in a separate private test location. This ticket does not
enable production Save/Save As controls or source replacement. Those belong to
BL4-010 and the persistence milestones.

**Accepted when:** synthetic round trips pass and a genuine Steam copy loads in-game
with inventory/progression intact. Without genuine samples or game access, record
that synthetic work is complete and leave the M2 compatibility gate pending.

## BL4-009: read-only Open and inspection

**Depends on:** the M2 acceptance gate. **Produces:** the first useful real-save UI.

Add manual file selection, account input and an explicit platform choice or validated
detection. Run reading, decoding and parsing off the UI thread. Use an Open request
ID to discard stale results: if request B replaces request A, A must never replace
B's displayed session after it finishes. UI updates happen on the UI thread.

Keep the previous session visible until a new load succeeds. On failure, show a
clear error and keep the previous selection/session intact. Show loading state,
filename, platform and document classification. Opening must not write a backup,
touch the source file or enable save controls.

Display available character/currency summaries and inventory entries. Unknown
families or undecodable serials remain visible using fallback labels; retain the
exact original serial. Implement search and selection against real summaries while
keeping the full document in the core session.

**Accepted when:** a genuine save can be opened and inspected responsively; source
bytes and dirty state remain unchanged; profiles/unknown data are clearly handled;
failed and stale jobs cannot corrupt the displayed session.

## What follows, and when to stop

After read-only inspection, BL4-010 adds validated name/currency commands and verified
Save As. Before that work, document exact field paths, bounds, character/profile
ownership and the expected changed-path set. Add same-directory temporary output,
output verification, backups for existing destinations and external-source-change
checks before exposing writes. The validation checklist defines that gate.

The item codec is M5. Its research fixtures start in BL4-004, but item editing,
equipped/backpack synchronization, slot allocation, imports and duplication must wait
for separately verified item rules. Do not expand this phase into skill/progression
editing, stat generation, automatic discovery or release packaging.

## Working and handing off a ticket

Keep one ticket narrow enough to review against its fixtures. Run formatting,
targeted core tests and Clippy for changed code; run the full core suite and Windows
release build at a milestone boundary. Use the existing development script for the
workspace checks. Documentation-only work needs link and consistency checks, not
an application rebuild.

Close or hand off a ticket with:

- Its ID, concrete behavior delivered and files changed.
- Fixtures used, their provenance and exact expected outcomes.
- Checks run and their results, including genuine-save/game evidence when required.
- Remaining unknowns and whether the acceptance gate passed or is still pending.
- The next dependent ticket that can now begin.

Suggested next implementation request:

> Review project status and extend read-only item naming with independent vectors.
> Preserve unknown serials, separate weapon prefixes/equipment rules from token
> parsing, and keep Save/Save As disabled until compatibility gates pass.
