# Repository Guidelines

## Project Structure & Module Organization

Mo is a Phase 0 Windows input method. Rust crates live in `crates/`: `mo-domain` holds platform-independent types, `mo-engine` coordinates the engine, and `mo-broker` serves IPC. The C++17 TSF/COM frontend is in `native/windows-tip/`. Installer checks are in `installer/windows/`; PowerShell tools are in `tools/`. Rust integration tests live in `crates/*/tests/`; script tests use `test-*.ps1`. Design decisions and acceptance criteria live in `docs/adr/` and `docs/phase-0/`. External asset provenance belongs in `third_party/`.

## Build, Test, and Development Commands

Run these from the repository root in PowerShell on Windows with the pinned Rust toolchain and Visual Studio C++ tools:

```powershell
cargo build --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./native/windows-tip/build-probe.ps1 -Architecture All -Backend MSBuild
./tools/tip-broker-smoke.ps1 -Architecture All
```

These commands build Rust, check formatting and lint, run workspace tests, probe both TIP architectures, and exercise TIP-to-Broker IPC. Real librime tests require verified runtime and rime-ice paths; see `README.md` for `tools/tip-rime-smoke.ps1` arguments.

## Coding Style & Naming Conventions

Follow `.editorconfig`: UTF-8, final newline, four spaces for Rust, TOML, C++, headers, and WiX; two spaces for JSON/YAML. Rust uses edition 2024 and `rustfmt.toml` (100-column width). Keep crate names in `mo-*` form, Rust modules in `snake_case`, and PowerShell tests named `test-*.ps1`. Treat Clippy warnings as errors.

## Testing Guidelines

Add unit or integration tests near changed Rust code and policy tests for changed scripts or installer rules. Use synthetic data; never place real keystrokes, candidate text, clipboard contents, or passwords in logs or fixtures. No numeric coverage threshold is specified. Run relevant targeted checks and workspace checks; use Windows smoke scripts for native or IPC changes.

## Commit & Pull Request Guidelines

Use `develop` for ongoing development and Win10 validation. Reserve `main` for a future formal release baseline.

Recent commits use Conventional Commit subjects such as `feat(settings): ...`, `fix(tip): ...`, and `test(installer): ...`. Keep each change scoped. In pull requests, describe the behavior and risk addressed, link the relevant issue or Phase 0 acceptance item, list verification commands, and include screenshots for visible candidate-window or settings UI changes. Update the related ADR or status document when changing an architectural contract or acceptance evidence.

## Security & Licensing

Keep `mo-domain` and `mo-engine` free of Windows and librime types; raw librime FFI belongs in `mo-rime-sys`, and only the Engine Actor calls librime. New Mo code is Apache-2.0. Record source, revision, license, attribution, and transformations for external assets before packaging.
