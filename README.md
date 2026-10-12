# Borderlands 4 Rust Save Editor

Windows desktop project following [the development plan](Borderlands4-Rust-Development-Plan.md).
The current implementation is an M0 application shell: Character and Inventory
screens, synthetic item selection/search, and a separate GUI-free core crate.
Opening, decoding, editing and writing actual saves are not implemented.

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
- `resources/manifest.json`: pinned upstream provenance and future resource imports.
- `fixtures/private`: ignored local compatibility samples.
- `docs/setup-status.md`: verification results and remaining milestone gates.

Next: BL4-004 format contract and synthetic fixtures, then BL4-005 through BL4-008
for key derivation, decoding, preservation and forced round trips. Keep real saves
local and use copies for compatibility checks. Save-writing controls remain disabled
until their validation gates pass.

Licensed under AGPL-3.0-only; see [LICENSE](LICENSE) and
[third-party notices](THIRD_PARTY_NOTICES.md).
