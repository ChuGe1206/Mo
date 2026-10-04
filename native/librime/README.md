# librime native boundary

Mo consumes librime's public C API plus a versioned Mo-only, input-free resource
preparation export in its experimental self-built runtime (ADR 0024/0025).
`UPSTREAM.toml` identifies the exact
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

`tools/runtime-build` now builds the pinned core plus Lua into a fresh development
runtime, with external plugins disabled. The tracked preparation patch does not
change `rime_api.h` or add upstream API-table slots. Installed-mode Broker requires
the v3 extension to succeed before readiness; v1/v2 have no prepared-mode fallback.
Official verification DLLs remain
usable only via the explicit legacy debug entry point. Build provenance is not
release approval. The Mo Simplifier path uses only verified files in the loaded
DLL's adjacent `opencc` directory, with no cwd/prefix/user/shared search. Legacy
OpenCC tooling APIs remain upstream-compatible. The Lua data-policy patch replaces
`package.path` with the two machine shared-data patterns, clears `package.cpath`,
and executes only the machine shared `rime.lua`; installed staging and prebuilt
data are the same Program Files directory. This is a source boundary, not a Lua
sandbox. Signatures, authenticated resource updates and final release review remain
separate gates.

## Local diagnostic experiments

[diagnostics/README.md](diagnostics/README.md) preserves the isolated Win10
component/startup and image/mapped-page experiment. Its patch and headers are
not runtime-build, staging, or installer inputs. Keep diagnostic outputs separate
from the accepted runtime and consult the linked acceptance evidence before
changing a production preparation contract.

## User dictionary startup policy

`preparation/userdb-preserve.patch` is a runtime-build input applied after
resources-v2 and the core learning patch. It enables strict LevelDB recovery,
removes automatic recovery scheduling from failed UserDictionary::Load, and
checks the required main dictionary during input-free preparation. ABI v3 and
bound provenance distinguish it from old v2 DLLs; no prepared-mode fallback exists.

`diagnostics/UserDbFixtureProbe.vcxproj` builds a strict x64 synthetic helper with
explicit pinned LevelDB include/library and isolated output properties. The
marker-guarded helper seeds/verifies an existing rime_ice.userdb, with 32 fixed
synchronous synthetic records. `tools/test-userdb-errors.ps1` verifies actual
file sharing and checksum failures through native/Actor/Broker startup, original
data-file preservation, and subsequent reopening. All fixtures are new.
See [fault evidence](../../docs/phase-0/WIN10-USERDB-ERRORS-EVIDENCE.md), ADR 0055
and the ABI v3 integration evidence before installed-host adoption.
