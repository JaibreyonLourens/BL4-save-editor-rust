# Private compatibility samples

Place genuine character/profile samples here only as local copies. Saves, account IDs,
decoded outputs and generated backups must not be committed. This directory's
contents are ignored except this README.

For each local sample record the game version, platform, account format and expected
decoded output in a local note. Steam decoding has been checked with a private
sample; encoding compatibility with an editor and the game requires separate checks.

## Private encoding tests

Set these variables in your PowerShell session to existing copied files:

```powershell
$env:BL4_STEAM_ID = "your Steam ID"
$env:BL4_SAVE_PATH = "C:\path\to\copied-save.sav"
$env:BL4_YAML_PATH = "C:\path\to\exported-save.yaml"
```

Encode an exported YAML file and decode the generated save to check exact bytes:

```powershell
cargo test -p bl4-core --test real_save encodes_private_steam_yaml -- --ignored --nocapture
```

Decode a copied save, force encoding from its exact YAML bytes, then decode again:

```powershell
cargo test -p bl4-core --test real_save decodes_and_reencodes_private_steam_save -- --ignored --nocapture
```

Each test prints its results directory under `fixtures/private/test-results/`.
Each run uses a new directory and preserves artifacts produced before a failure.

| Test | Saved artifacts |
| --- | --- |
| Encode exported YAML | `source.yaml`, `encoded.sav`, `decoded.yaml`, `results.txt` |
| Decode and re-encode save | `source.sav`, `source.yaml`, `reencoded.sav`, `redecoded.yaml`, `results.txt` |

Reports contain byte counts, validation results and source-preservation checks.
Account IDs and save contents are not logged. Generated artifacts are private and
ignored by Git. The input files are only read; artifacts never replace them.
`BL4_DECODED_YAML_PATH` is also accepted as an input YAML path when `BL4_YAML_PATH`
is unset, for compatibility with the earlier export test.

Open `encoded.sav` or `reencoded.sav` in a compatible editor for the next manual
check. These tests prove container byte preservation without parsing/reserializing
YAML. They do not establish YAML parser preservation or in-game compatibility.
