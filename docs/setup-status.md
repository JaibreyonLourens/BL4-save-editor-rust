# Setup status — 12 October 2026

Implemented:

- Cargo workspace with `bl4-core` and `bl4-desktop`; the core has no GUI dependencies.
- Aligned Slint/slint-build 1.18.1, with a winit/software-rendered desktop window.
- Character placeholder and searchable synthetic inventory with selectable details.
- Stable Rust configuration, application lockfile, Windows CI and VS Code tasks.
- Development script that locates Rust when PATH has not refreshed.
- Git repository initialized locally; no commit or remote created.
- Resource manifest and AGPLv3 license.
- Steam container decoding with strict AES, padding, zlib, checksum and length checks.
- A private Steam save decodes to YAML accepted by a separate compatible editor.
- Private fixture directory excluded from Git.

Local environment: Rust/Cargo 1.99.0, stable x86_64-pc-windows-msvc and Visual Studio
Build Tools 2026 were already installed. No global toolchain or execution-policy
changes were needed.

Verification:

- Debug desktop build passed.
- `cargo fmt --all -- --check` passed.
- `cargo test --workspace --locked` passed (zero tests at the shell stage).
- `cargo clippy --workspace --all-targets --locked -- -D warnings` passed.
- Desktop process opened a visible, responding window with the expected title.
- Windows release build passed; executable: `target/release/bl4-desktop.exe`.

The Windows CI workflow is configured, but has not run on a remote clean checkout.
Interactive keyboard navigation, selection and search still need a manual smoke
check. No save-format or in-game compatibility is claimed.

Ticket progress:

| Ticket | Status |
| --- | --- |
| BL4-001 | Workspace/checks implemented; remote clean-checkout CI pending |
| BL4-002 | Layout implemented and window startup checked; interactive smoke check pending |
| BL4-003 | Reference/license/manifest recorded; catalog selection and imports deferred |
| BL4-004 | Container notes started; serial research and synthetic fixtures pending |

The next implementation task is encoding with synthetic round-trip fixtures, then
YAML document preservation. No save files are written by the application.

Implementation handoff documentation is available in the
[implementation guide](implementation-guide.md), with a detailed fixture matrix,
expanded container contract and acceptance checklist. These documents specify work
to do; they do not add codec code, fixtures or compatibility evidence.
