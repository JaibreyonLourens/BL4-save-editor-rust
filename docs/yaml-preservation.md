# BL4-007: candidate YAML preservation probes

Started 13 October 2026. `serde_yaml_ng` now backs the production document API,
with `yaml-rust2` events enforcing alias and resource policies before tree creation. Its generic `Value` model retains
nodes that would disappear from a struct containing only known save fields.
See the [Value documentation](https://docs.rs/serde_yaml_ng/0.10.0/serde_yaml_ng/enum.Value.html).

## Measured behavior

The synthetic tests in `crates/bl4-core/tests/yaml_preservation.rs` assert known
values and types before comparing complete parse/serialize/parse trees.

| Construct | Observed candidate behavior | Production document policy |
| --- | --- | --- |
| Unknown fields, nested maps/sequences, sequence order | Preserved | Retain full tree |
| Null, boolean, integer, float, quoted strings | Types and values preserved | Retain types |
| Unicode and multiline strings, empty collections | Values preserved | Retain values |
| Local tags on scalars, sequences and maps, nested tags | Preserved | Retain tag identity and values |
| Core `!!str/int/float/bool/null/seq/map`, including canonical verbatim spellings | Types preserved | Accept matching node kinds; core type annotations may normalize away |
| Unknown global tags and non-core YAML types | May silently discard tag semantics | Reject before tree creation |
| Integer, boolean, sequence and mapping keys | Preserved | Retain generic keys |
| Duplicate keys, including nested and complex keys | Rejected | Reject |
| Anchors/aliases | Expanded; shared identity lost | Reject aliases before tree creation; unreferenced anchors have no retained identity |
| Recursive alias | Rejected | Reject |
| Merge key `<<` | Retained literally; emitter drops literal-key quotes | Reject all mapping keys named `<<`, including quoted/tagged keys; allow `<<` string values |
| Empty input, comments only, empty document | Accepted as null | Reject absent save content before parsing |
| Malformed YAML, invalid byte sample, multiple documents | Rejected | Explicit UTF-8 check, single-document policy |
| Collection nesting / total nodes | Parser-specific recursion limit | 128 collections deep and 100,000 nodes; abort the event scan immediately at either limit |
| Integer boundaries | Values outside 64-bit range may fail or become floating point | Accept exact integers from i64::MIN through u64::MAX; reject overflow before resolution |
| Finite floats / special floats | Binary64; out-of-range literals may become strings | Binary64 precision; reject finite literal overflow and underflow to zero; retain `.inf`, `-.inf`, `.nan` and negative floating zero |
| Quoted numeric text outside supported numeric range | Emitter may remove quotes | Quote affected emitted scalar tokens so reconstruction remains supported text |

These guards are implemented in `save/document.rs`. A pull-based event scan checks
the entire single document before building `Value`. It rejects aliases, unsupported
tags, merges and unrepresentable numbers, and stops at resource limits. The input
and serialized output limit is 16 MiB. Scalar text is still bounded by the byte
limit; event budgets do not provide a separate wall-clock or scalar-token limit.
Quoted text and ordinary `&`/`*` characters remain valid. Floating-point precision
is explicitly binary64, not arbitrary-precision decimal. Unsupported syntax can
be rejected by either underlying parser; this is a tested subset of YAML, not a
claim to support every YAML feature.

The new production tests in `tests/document_contract.rs` assert literal expected
types/values for core/local tags, exact integer limits, special floats and quoted
text. They also test wrong node kinds, merge rejection, numeric overflow/underflow,
aliases/duplicate keys/multiple documents, exact depth/node acceptance and immediate
limit errors even when later input is malformed. The policies use the
[YAML 1.2.2 type model](https://yaml.org/spec/1.2.2/#chapter-10-recommended-schemas)
and explicitly decline the [YAML 1.1 merge extension](https://yaml.org/type/merge.html).

## Private evidence

The private probe parsed an existing 125,156-byte YAML export, serialized it,
reparsed it, and compared the entire generic tree successfully. It also verified
the export was unchanged. Contents and parser diagnostics were not printed.
This establishes preservation of the parser's interpretation, not independent
proof that every original YAML construct retains its meaning.

The production session path opened the private save, parsed and serialized the
document, reconstructed the container, reopened it, and matched its complete
semantic tree. The user confirmed that generated save loads in a compatible
editor. Both earlier exact-YAML artifacts also load there. In-game acceptance
remains untested.

## Current inspection and next acceptance step

The bounded document and session APIs are implemented. Character/profile shapes
and character name/class/currency/backpack summaries use private export evidence.
Level uses the unique plain `type: Character` experience record, never sequence
position or a specialization level. Missing, tagged, malformed or duplicate records
make level unavailable. Whole-tree preservation still requires in-game acceptance
of forced reconstruction, broader genuine samples and remaining container coverage.

```powershell
cargo test -p bl4-core --test yaml_preservation --locked --offline
# Set BL4_YAML_PATH to an existing private YAML export first:
cargo test -p bl4-core --test yaml_preservation private_yaml_semantic_round_trip --locked --offline -- --ignored --nocapture
```
