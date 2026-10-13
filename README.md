# Borderlands 4 Rust Save Editor

Windows desktop project for inspecting and editing Borderlands 4 save files.
The current implementation includes a Character and Inventory shell with synthetic
data, plus a GUI-free core crate that validates and decodes Steam save containers.
YAML editing, encoding and save-file writing are still in progress.

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

- `crates/bl4-core`: presentation-neutral types; future container/document/edit APIs.
- `crates/bl4-desktop`: Slint UI and Rust presentation wiring.
- `resources/manifest.json`: local resource inventory and future imports.
- `fixtures/private`: ignored local compatibility samples.
- `docs/setup-status.md`: verification results and remaining milestone gates.

Next: implement encoding and synthetic round-trip fixtures, then document parsing,
editing and controlled save-file writing. Keep real saves local and use copies for
compatibility checks. Save-writing controls remain disabled until their validation
gates pass.

Start implementation with the [implementation guide](docs/implementation-guide.md).
It provides the ordered tickets, prerequisites and proposed core responsibilities.
Use its [fixture specification](docs/fixture-specification.md),
[format contract](docs/format-contract.md) and
[validation checklist](docs/validation-checklist.md) for the required evidence.

Licensed under AGPL-3.0-only; see [LICENSE](LICENSE).
