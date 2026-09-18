# Windows deployment assets — development staging and authoring probe

## Verified development staging

`prepare-stage.ps1` now creates the fixed installation-tree **assets**, not an
installer. Use PowerShell 7.4+ as an ordinary user with the existing local Rust
1.97.1, Visual Studio C++ tools, dependency cache and verified runtime build:

```powershell
./installer/windows/prepare-stage.ps1 `
  -RuntimeBuildDirectory "$PWD/build/mo-runtime-relocatable-verified" `
  -RimeIceSourceDir "$env:TEMP/mo-rime-ice-6810e89" `
  -OutputDirectory "$PWD/build/mo-windows-stage-new" -RustToolchain stable
./installer/windows/verify-stage.ps1 -StageDirectory "$PWD/build/mo-windows-stage-new/stage"
./installer/windows/test-staging.ps1 -StageDirectory "$PWD/build/mo-windows-stage-new/stage"
./installer/windows/test-stage-runtime.ps1 -BuildDirectory "$PWD/build/mo-windows-stage-new" -FaultRepetitions 10
```

The output must be a **new child of repository `build/`**, without reparse-point
ancestors. Existing output is never overwritten, sources are never mutated,
and the pipeline never registers/enables an IME, changes defaults, starts UAC,
downloads tools or installs packages. Rustup auto-install is disabled during
toolchain discovery; the explicit installed toolchain must report Rust 1.97.1.
Cargo compilation is locked/offline with an explicit x64 target and fresh cache.
The build output and Visual Studio installation paths must not contain percent,
exclamation or quote characters: quoted cmd paths still permit percent expansion.
This restriction is build-only, not a limitation on end-user Chinese input.

The builder:

- Checks the v2 core+Lua runtime's six source archive pins, eleven Mo source
  snapshots, public ABI header, DLL and all 33 adjacent OpenCC resources.
- Archives the locked rime-ice Git object, verifies its archive hash and extracts
  fresh inputs. Mutable checkout files, ignored `build/`, upstream platform
  skins and installer recipes are not consumed.
- Copies 64 selected source resources without modifying upstream configuration
  or Lua, then compiles all 29 required schema/dictionary outputs with a small
  `/W4 /WX` build-only deploy helper and the verified DLL. Deployment happens
  only in an isolated marked directory, never on the input hot path.
- Snapshots Mo sources and freshly builds the release Broker, x64/x86 TIPs and
  registrar. No arbitrary prebuilt Broker/TIP arguments are accepted. The Broker
  must reject `--fake`; both default ABI probes verify the diagnostics interface
  is absent. Compiler environment overrides are rejected.
- Retains the exact rime-ice archive, LICENSE and Credits outside the payload,
  records source/output hashes and build dependencies, and verifies pending
  metadata before renaming it to the final `mo-stage.json` completion marker.

The final tree contains `payload/Mo/bin`, `tip/x64`, `tip/x86`,
`runtime/librime/rime.dll` + `opencc/`, and `data/rime-ice/build` + selected
source resources. `evidence/` contains six evidence files. Intermediate objects,
probes, diagnostic executables and logs remain under sibling `working/`.

`verify-stage.ps1` checks exact inventory, sizes/hashes, path/case rules, PE
architecture/kind, data and image receipts, ABI/plugin/source pins, and the
non-installable development flags. It never executes staged code. The runtime
test **does execute trusted, locally built artifacts**: unchanged staged TIP
  bytes in a disposable installed-like layout, paired with a copied diagnostic
Broker, plus the staged runtime/data. It does not run the installed release
Broker as a working input service or prove real system registration.

The resource-pack golden probe starts with empty managed user/staging directories
and an explicit installed prebuilt-data path. Chinese, Emoji, English, date,
Unicode, number and calculator selections each commit exactly once; staging
must stay empty throughout. These seven cases do not prove every Lua module,
all enabled schemas or real host input behavior.

These unsigned, self-declared receipts are **consistency evidence, not
authentication**. Do not run untrusted stages just because verification passes.
The trusted build host/toolchain/Cargo home are not hermetically isolated; path
checks are not a handle-based concurrency/ACL proof and do not reject every
possible hard link. The bundle has not closed user/Lua/staging override policy,
VC prerequisites, signatures, full per-file notices/SBOM, corresponding-source
review or legal approval. All manifests explicitly remain development-only,
non-redistributable and non-installable. This is not G3 or daily-use acceptance.

## WiX placeholder (still non-deployable)

`Package.wxs` and `Bundle.wxs` record the intended WiX v4 split: an elevated,
per-machine MSI owns x64/x86 TIP files, the x64 user broker, the registrar, and
both COM registry views; Burn will eventually coordinate prerequisites and a
non-elevated current-user finalizer.

This is deliberately not a deployable installer. It does **not** call
`ITfInputProcessorProfileMgr::RegisterProfile`, enable the profile with
`InstallLayoutOrTip`, start a per-user broker, stop TSF hosts, implement rollback
for TSF state, or sign either package. The intended broker artifact is now the
default release named-pipe broker, which accepts no arguments and requires the
fixed Known Folder layout recorded by ADR 0016. The placeholder builder now
requires a verified `-StageDirectory`; the former four arbitrary artifact
parameters have been removed. Its current authoring still includes only the
four original Mo binaries, **not** the runtime/data trees. It must not be
treated as a production packaging path.

The build script has two safety gates:

1. If the WiX v4 `wix` command is absent it fails immediately and never downloads
   anything.
2. Even with WiX installed, `-AllowPlaceholderBuild` is required and the outputs
   are named as placeholders.
3. The stage must pass development consistency validation before WiX is invoked.

Before any release, replace the placeholder flow with transactional machine
registration plus a user-context finalizer; define repair/uninstall recovery;
use versioned binaries to tolerate loaded TIP DLLs; validate clean install,
upgrade, rollback, repair, and uninstall in disposable VMs; sign the x64/x86
DLLs, broker, MSI, and Burn EXE with timestamping; and enforce signature checks
in CI. Package the allowlisted self-built librime at `runtime/librime/rime.dll`
and precompiled rime-ice at `data/rime-ice/build`; prepare the current user's
`LocalAppData/Mo/Rime` data and staging directories without running deployment
on the input hot path. Reject diagnostic/debug-assertions artifacts in the
release packaging pipeline. Never run an updater or broker as LocalSystem.
