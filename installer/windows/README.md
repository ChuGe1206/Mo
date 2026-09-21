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

## Full-payload WiX development packages (still non-deployable)

`wix-payload.ps1` deterministically turns a verified stage into one file-owning
WiX component per payload file, plus one registry-only component for the x86
COM view. The generated fragment covers all 131 payload files: x64/x86
TIPs, release Broker/registrar, self-built librime plus 33 OpenCC resources, and
the selected/precompiled rime-ice tree. Stable path-derived identifiers and GUIDs
make repeated authoring byte-identical. A verifier locks the Program Files root,
reconstructs every installed path from XML, checks component/ref/key-path/
bitness/COM ownership one-to-one, and rejects missing, duplicate or redirected
files and roots. Evidence and source archives remain outside the install image.

`Package.wxs` is a per-machine x64 MSI authoring input. MSI components own both
COM registry views. An embedded x64 registrar performs machine profile/category
changes as elevated deferred actions. Before each install/repair or uninstall
mutation it writes a protected fixed-layout marker containing the exact prior
profile/category presence bits. A paired rollback action restores those bits;
the commit action deletes the marker. Major-upgrade removal leaves the stable
profile for the incoming package, whose transaction refreshes it. MSI execution
is rejected when rollback is disabled. No elevated action enables Mo for a user,
changes the default input method, starts the Broker or writes user data.

The registrar's marker serializer/parser runs without elevation in both x64 and
x86 build probes and in the freshly staged x64 binary. This validates all eight
operation/state marker combinations and no-residue cleanup, but does not simulate
Windows Installer cancellation or prove real TSF API rollback.

`Bundle.wxs` now orders the per-machine MSI before a vital `PerMachine=no`
finalizer package. Burn detects an exact marker under the machine-local HKCU
`Software\Classes\Local Settings` tree. The three required base arguments use
the fixed `burn-user-finalizer` prefix; global-action `CommandLine` rows append
one exact install/remove/repair or inverse rollback command. Repair with no
marker fails closed instead of installing. A stable
`Mo.CurrentUserFinalizer.v1` dependency provider gives compatible upgrades one
ref-counted identity. The native command rejects elevated, Session 0 and
AppContainer tokens; rejects either HKCU COM shadow view; and verifies the files,
HKLM COM views, profile and keyboard category before enabling the initiating
user without default/clean-install flags.

A same-session named mutex serializes operations. Before input state changes,
the helper flushes and reads back a durable journal containing the operation and
prior enabled bit. The journal remains after success because Burn may run the
inverse action in another process; rollback consumes it to restore the exact
prior bit. Marker and journal deletion are also flushed and verified. Fresh
install rejects ambiguous pre-enabled/unowned state, repair requires the exact
marker, and uninstall removes only marker-owned state. Failures preserve a
detectable, retryable receipt.

The repository now locks the WiX CLI plus Bal, Util and Dependency extensions to
4.0.6, including exact official NuGet size/SHA-512 metadata and source commit.
`prepare-wix-toolchain.ps1` creates a repository-local, inventoried development
toolchain from either pre-existing packages or an explicit `-AllowDownload`;
nothing is installed globally. A linked-artifact verifier decompiles the MSI,
extracts the Burn attached container and rejects mismatched component/action
counts, scopes, command routing, provider identity or embedded bytes.

The packages remain deliberately non-deployable. The standard BA is temporary
and both outputs are unsigned; VC prerequisites, loaded-TIP upgrade handling,
multi-user uninstall policy, complete notices/SBOM, ACL inspection and release
authorization remain open. A fresh local build linked the MSI and Bundle and
passed structural verification without executing either package or changing
real current-user input state. MSI ICE validation could not run on this host
because the Windows Installer service is unavailable and remains a VM gate.
The mixed-scope chain also produces WIX1140: a per-user Bundle does not register
a dependency on its per-machine MSI. The build suppresses that understood link
warning only after the verifier confirms the intended split; upgrade and
multi-user behavior still require disposable-VM tests.

The `DevelopmentTest` Bundle also contains two hidden, default-zero fault
variables. `MoTestFailAfterMachineProfile=1` forwards one secure MSI property
and forces a deferred failure after the machine-profile mutation but before its
commit action. `MoTestFailAfterUserFinalizer=1` schedules a vital chain-tail
probe after the user finalizer. The linked verifier proves both paths in the
actual manifests and embedded bytes. They are only for the guarded disposable-VM
matrix. The `ProductionShape` flavor removes these variables, the MSI property
and deferred failure action, the Burn failure package, and the registrar command
at compile/preprocess time. Its linked verifier requires that absence; it is
still unsigned, development-branded and non-deployable, not a release candidate.

The build pipeline has four safety gates:

1. The toolchain must match the locked WiX `4.0.6+73c89738`, extension files and
   complete inventory. Preparation never downloads unless `-AllowDownload` is
   explicitly supplied.
2. Even with WiX installed, `-AllowDevelopmentBuild` is required. The default
   `DevelopmentTest` outputs are named `development-unsigned`; `ProductionShape`
   additionally requires `-AllowProductionShapeBuild` and is named
   `production-shape-unsigned`.
3. The stage must pass development consistency validation; generated authoring
   is then independently verified before WiX is invoked.
4. A build is successful only after the linked MSI and Bundle are decompiled or
   extracted and their actual manifests and embedded payload hashes pass.

For an offline, non-installing local build from already downloaded packages:

```powershell
./installer/windows/prepare-wix-toolchain.ps1 `
  -PackageDirectory "$PWD/build/tools/wix-4.0.6-packages" `
  -OutputDirectory "$PWD/build/mo-wix-toolchain-new"
./installer/windows/build.ps1 `
  -StageDirectory "$PWD/build/mo-windows-stage-new/stage" `
  -WixToolchainDirectory "$PWD/build/mo-wix-toolchain-new" `
  -OutputDirectory "$PWD/build/mo-linked-installer-new" `
  -AllowDevelopmentBuild
```

To prove the fault-free linked shape without producing a distributable package,
prepare a stage without `-DevelopmentFaultInjection`, then build with
`-BuildFlavor ProductionShape -AllowDevelopmentBuild -AllowProductionShapeBuild`.
To build the destructive VM-test flavor, prepare its stage with
`-DevelopmentFaultInjection` and use the default `DevelopmentTest` flavor. The
builder rejects a stage whose registrar does not match the requested flavor.

For non-mutating authoring checks:

```powershell
./installer/windows/test-package-authoring.ps1 `
  -StageDirectory "$PWD/build/mo-windows-stage-new/stage"
./installer/windows/test-vm-lifecycle-policy.ps1
```

`prepare-vm-test-kit.ps1` can bind a verified Bundle, staged registrar and stage
manifest into a hash-inventoried transfer directory. Its Windows PowerShell 5.1
guest driver is fail-closed unless a separately initialized disposable VM,
matching machine sentinel, two explicit execution switches and a non-elevated
interactive token are all present. It validates clean install, repair and
uninstall state plus every installed payload byte; it is not run automatically.
See `docs/phase-0/VM-INSTALLER-TEST.md` and ADR 0032.

`verify-linked-upgrade-pair.ps1` and `prepare-vm-matrix-test-kit.ps1` additionally
bind two versions into a hash-locked rollback/repair/Major Upgrade matrix. Its
Windows PowerShell 5.1 driver verifies both forced rollback boundaries, missing-
marker repair rejection, old/new MSI ProductCode transition and final uninstall.
The current host only prepares and rejects this kit; real results require a
separately initialized disposable VM. See ADR 0033. Flavor isolation and linked
production-shape evidence are recorded in ADR 0034.

Before any release, replace or brand the temporary standard BA as needed; use
versioned binaries to tolerate loaded TIP DLLs; validate clean
install, repair, upgrade, forced rollback and uninstall in disposable VMs; sign
the x64/x86 DLLs, Broker, MSI and Bundle with timestamping; and enforce signature
checks in CI. Never run the Broker, updater or current-user finalizer as
LocalSystem.
