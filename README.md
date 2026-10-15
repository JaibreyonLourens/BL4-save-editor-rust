# Borderlands 4 Rust Save Editor

Windows desktop project for inspecting and editing Borderlands 4 save files.
The desktop opens saves read-only and displays character details, equipped items
and backpack entries. The GUI-free core decodes/encodes containers, preserves
supported YAML documents and resolves supported item serials against a bundled
catalog. Editing and application save-file writing are still pending.

## Start on Windows

Prerequisites: stable Rust (`x86_64-pc-windows-msvc`), Visual Studio C++ Build Tools
and a Windows SDK. VS Code will recommend rust-analyzer and the Slint extension.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev.ps1 run
```

The script also finds Rust in `%USERPROFILE%\.cargo\bin` when the terminal PATH
has not refreshed. The first build downloads Cargo dependencies and may take several minutes.
With Cargo already on PATH, `cargo run -p bl4-desktop --locked` works directly.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev.ps1 check
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev.ps1 build
```

The release executable is `target/release/bl4-desktop.exe`. Windows CI runs formatting,
workspace tests, Clippy and a release build. Commit `Cargo.lock` with the source.
The commands use a process-only execution-policy override because this machine
blocks PowerShell scripts by default; they do not change your system policy.

## Layout and next work

- `crates/bl4-core`: container codec and presentation-neutral types.
- `crates/bl4-desktop`: Slint UI and Rust presentation wiring.
- `fixtures/private`: ignored local compatibility samples.
- `docs/setup-status.md`: verification results and remaining milestone gates.
- `docs/item-serial-implementation.md`: read-only item-codec worksheet and evidence gates.
- `resources/manifest.json`: bundled catalog revision, input hashes and import metadata.
- `THIRD_PARTY_NOTICES.md`: bundled catalog attribution and licensing.

Select Open, check the detected account ID and Steam/Epic selection, then choose
Load selected save. Character and Inventory pages are read-only; inventory search
filters the Equipped and Backpack groups. Unsupported serials remain visible.
The bundled catalog identifies supported weapon base names; full prefixes,
equipment naming and patch-specific coverage remain incomplete.

Next: broaden compatibility fixtures, complete item-name resolution and verify
in-game round trips. Keep real saves local and use copies for compatibility checks.
Save-writing controls remain disabled until their validation gates pass.

Start with [project status](docs/setup-status.md), then the
[implementation guide](docs/implementation-guide.md).
It provides the ordered tickets, prerequisites and proposed core responsibilities.
Use its [fixture specification](docs/fixture-specification.md),
[format contract](docs/format-contract.md) and
[validation checklist](docs/validation-checklist.md) for the required evidence.

Licensed under AGPL-3.0-only; see [LICENSE](LICENSE).
