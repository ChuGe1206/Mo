# librime native boundary

Mo consumes only librime's public C API.  `UPSTREAM.toml` identifies the exact
source revision used to generate and probe the Rust declarations.  Native
headers, libraries and transitive dependencies are intentionally not committed
at this Phase 0 step.

Expected local/CI inputs are supplied explicitly:

- `RIME_INCLUDE_DIR`: directory containing the pinned official `rime_api.h`,
  used by `tools/abi-probe`.
- `MO_LIBRIME_LIB_DIR`: directory containing the built library, used only when
  a `mo-rime-sys` `link-*` feature is enabled.
- `MO_LIBRIME_LIB_NAME`: optional link name; defaults to `rime`.

No build script in this repository may silently download an unpinned librime or
fall back to a different system version.

## Production distribution boundary

The official 1.17.0 Windows verification archive is hash-pinned in
`third_party/manifest.toml`, but it is **not** a Mo release input. Its bundled
`rime.dll` statically contains `librime-octagram`, which is GPL-3.0-only.
Phase 0 uses that binary locally to validate ABI and behavior only.

Production packages must build the pinned librime source themselves with an
audited plugin allow-list. The current minimum is the BSD-3-Clause core plus
BSD-3-Clause `librime-lua`, because the pinned rime-ice schemas require Lua.
Adding any other plugin requires a manifest and license-policy change.
