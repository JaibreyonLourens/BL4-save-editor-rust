# Project status - 15 October 2026

This file records current implementation, evidence and remaining milestones.
See the [implementation guide](implementation-guide.md) for the milestone sequence.

## Implemented

- Cargo workspace with a GUI-free `bl4-core` and Slint `bl4-desktop`.
- Read-only character name/class/level/currencies and separate collapsible equipped/backpack lists.
- Manual Open with Windows Documents discovery, editable account ID and platform choice.
- Background file loading, retained sessions, shared inventory search and stale-request guards.
- Bounded YAML parsing, full-tree serialization and conservative character/profile classification.
- Item token decoding, weapon base-name resolution and fallback part descriptions.
- Stable Rust configuration, application lockfile, Windows CI and VS Code tasks.
- Development script for locating Rust and running workspace commands.
- AGPLv3 license, third-party notices and a pinned catalog import (71 roots, 7,050 parts).
- Steam/Epic account validation and AES key derivation.
- AES-256-ECB container decoding with padding, zlib, checksum and length validation.
- Encoding from exact YAML bytes: level-9 zlib, four-byte length, PKCS#7 and AES.
- Specification-based encoding tests and private tests that preserve their outputs.
- Private samples, test artifacts and internal notes excluded from Git.

## Verification

Latest checks: 104 active core tests and 10 desktop tests pass (nine private tests
ignored). Workspace formatting and Clippy pass. Earlier private character/profile
inspection and production character YAML round trips passed with source unchanged;
they were not rerun during commit preparation.

Latest core checks passed:

```powershell
cargo fmt --all -- --check
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
```

Private tests are ignored unless explicitly selected with the required local fixtures.

Both new private integration tests were run successfully on 13 October 2026:
encoding exported YAML, and decoding/re-encoding a copied Steam save. Each recovered
the exact 125,156 YAML bytes and confirmed the input file was unchanged. The original
save was 21,296 bytes; the reconstructed save was 20,832 bytes. Compression output
can differ without changing the decoded content.

The user confirmed that the exported decoded YAML loads in a compatible editor.
The user also confirmed both exact-YAML encoded saves and the production-session
reconstructed save load in that editor. In-game acceptance remains unconfirmed.
BL4-007 now has a bounded production document API, full-tree serialization,
session state and a passing private parse/serialize/reconstruct/editor check; see
[YAML preservation](yaml-preservation.md) for findings and unresolved policies.
Profile classification and full-tree YAML preservation pass against a private
export. Encrypted profile reconstruction and genuine Epic support remain unverified.

Earlier shell checks passed a desktop debug build, workspace tests/Clippy, visible
window startup and a Windows release build. These desktop checks were not rerun for
the encoding commits. Remote CI and interactive UI smoke checks remain pending.

## Ticket progress

| Ticket | Current status |
| --- | --- |
| BL4-001 | Workspace/checks implemented; remote CI pending |
| BL4-002 | Character/inventory UI displays real saves read-only |
| BL4-003 | Bundled catalog import, manifest hashes and license notices implemented |
| BL4-004 | Steam layout verified; complete independent fixture catalog and item research pending |
| BL4-005 | Account parsing/key derivation implemented; broader independent Epic vectors pending |
| BL4-006 | Decoder implemented and tested with a private Steam save; remaining boundary/negative coverage pending |
| BL4-007 | Document/session layer, character/profile recognition, character level/summaries and supported YAML policies implemented; broader fixture acceptance pending |
| BL4-008 | Forced private parse/dump/re-encode and editor checks pass; in-game gate pending |
| BL4-009 | Read-only loading, summaries, equipped/backpack grouping and search implemented; broader compatibility acceptance pending |
| BL4-010 onward | Editing and persistence not implemented |

## Latest commits

- `ece8f0b` - document parsing and desktop dialog dependencies.
- `c83dec4` - document/session preservation and conservative save inspection.
- `a7165dc` - bounded item serial token decoding.
- `eec6e1c` - attributed bundled catalog and weapon base-name resolution.
- `4e7236c` - read-only desktop character and grouped inventory inspection.

Production Save/Save As controls remain disabled. See the
[private test instructions](../fixtures/private/README.md) for reproducing tests and
locating generated saves.
