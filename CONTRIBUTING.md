# Contributing to Mo

Mo is currently in Phase 0. Contributions should reduce one of the documented risks in `docs/phase-0/ACCEPTANCE.md`.

## Engineering rules

- Keep `mo-domain` and `mo-engine` independent of Windows and librime types.
- Only `mo-rime-sys` may expose raw librime FFI.
- Only the Engine Actor may invoke librime.
- The Windows TIP must not contain engine, dictionary, update, network, or product-setting logic.
- Do not copy source from GPL platform frontends into Apache-2.0 Mo components.
- Never record raw keys, preedit, candidates, commits, surrounding text, clipboard contents, or password fields in logs or fixtures.
- Use synthetic corpus data in tests.

## Local checks

```powershell
cargo +stable fmt --all -- --check
cargo +stable clippy --workspace --all-targets -- -D warnings
cargo +stable test --workspace
```

The checked-in `rust-toolchain.toml` pins the release toolchain. In restricted development sandboxes where rustup cannot update its own directory, `+stable` selects the already installed toolchain; CI must verify that its exact version matches the pin.

## Licensing

New Mo-authored source is Apache-2.0 unless a directory clearly states otherwise. Every external asset must have a source URL, immutable revision, license, attribution, and transformation record before it enters a distributable pack.
