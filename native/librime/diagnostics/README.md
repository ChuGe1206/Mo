# Isolated Windows librime diagnostics

This directory preserves the Win10 Phase 0 experiment described in
[the page evidence record](../../../docs/phase-0/WIN10-IMAGE-PAGE-EVIDENCE.md) and
[the DB-open follow-up](../../../docs/phase-0/WIN10-DB-OPEN-EVIDENCE.md).
It is not consumed by `tools/runtime-build`, staging, or the installer.

## Source and license

`components.patch` is relative to a **copy of the accepted Mo runtime's patched
source**, not a pristine upstream checkout. The base is pinned librime 1.17.0
(`33e78140250125871856cdc5b42ddc6a5fcd3cd4`) plus the allow-listed librime-lua
revision and Mo preparation/data/learning patches recorded in
`third_party/manifest.toml` and the base `mo-build-provenance.json`. Upstream
files retain their BSD-3-Clause notices. The Mo headers and diagnostic
additions are Apache-2.0. No upstream archive or binary is vendored here.

The patch adds nested timing scopes to session construction, component creation,
OpenCC preparation, dictionary loading, native processing, candidate preparation,
and Lua resume. `db-open.patch` is an incremental patch after `components.patch`; it adds DB/IO and mapping scopes. All three headers must be copied to `src/rime/` in that isolated source.
All scope labels come from the fixed test schema/component namespaces; use only
trusted, synthetic fixtures. Never attach this runtime to a real user profile.

## Reproduction boundary

Start from a separate copy of the accepted, provenance-verified runtime source.
From the repository root, with the copy at `build/my-diag-source`:

```powershell
git apply --check --directory=build/my-diag-source native/librime/diagnostics/components.patch
git apply --directory=build/my-diag-source native/librime/diagnostics/components.patch
git apply --check --directory=build/my-diag-source native/librime/diagnostics/db-open.patch
git apply --directory=build/my-diag-source native/librime/diagnostics/db-open.patch
Copy-Item native/librime/diagnostics/mo_*.h build/my-diag-source/src/rime/
```

Use a new CMake build directory and the pinned dependencies/toolchain from the
base provenance. Never overwrite the accepted `dist`, source inputs, or package
stage. Build the `rime` target and place its DLL in a separate diagnostic `dist`
with an exact copy of the accepted adjacent `opencc` resources. Do not rename a
diagnostic DLL into an accepted build input. The archived source snapshots and
hash manifest bind the exact binaries used; rebuilding need not reproduce a PE
byte-for-byte.

A reused MSBuild node produced a `PATH`/`Path` duplicate-key failure during this
run. The successful local command used process-local
`MSBUILDDISABLENODEREUSE=1`, `--parallel 1`, and `/nodeReuse:false`. No system
PATH or persistent environment setting was changed.

Set these only on the owned test process using `ProcessStartInfo.Environment`:

| Variable | Exact value | Effect |
| --- | --- | --- |
| `MO_DIAG_PREFAULT_IMAGE` | `1` | Read one byte from each committed, readable page in this DLL image after input-free Simplifier preparation. |
| `MO_DIAG_PREFAULT_MAPPED` | `1` | Read each page of a newly opened read-only dictionary mapping. |
| `MO_DIAG_TRACE_DB_IO` | `1` | Forward DB file operations through timing wrappers; all Sync calls are recorded, including intervals below 500 µs. |
| `MO_DIAG_REUSE_DB_LOGS` | `1` | Set the pinned LevelDB experimental `reuse_logs` option. It does not override Sync, locking, or recovery. |

Any other value leaves the corresponding preparation disabled. Timing scopes
are always active in this diagnostic build and write synchronous stderr records
for intervals of at least 500 µs (DB Sync uses a zero threshold). They can perturb the timings. Run controls
using the same DLL and explicit zero-valued flags. Preserve failures and first
versus second probe outcomes separately.

Use the marker-guarded `crates/mo-rime/examples/actor_latency_probe.rs` with
absolute paths, a disposable compiled user directory, and `--broker-plan`.
For full-path checks use the existing `mo_tip_abi_probe --broker-rime-ice` and an
owned Broker in prepared mode; keep the 50 ms key deadline. Historical host
harnesses and their output are archived under
`build/win10-evidence-clean-v1/ImagePages-v1/harness`; their root-relative paths
must be re-created before rerunning. Their result files are immutable evidence,
not a general automation interface.

## Interpretation

`PrefaultOwnImage` obtains the current DLL by an address in that module and
queries its image range. It reads only committed `MEM_IMAGE` regions belonging
to that module with readable protections, excluding guard/no-access pages.
It does not change protections, lock pages, send keys, or log memory contents.
The Engine keeps the DLL loaded during this call; the unchanged-reference-count
handle must not be released. Working-set valid counts are instantaneous samples
and do not promise continued residency.

API references: [GetModuleHandleEx](https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-getmodulehandleexa),
[VirtualQuery](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualquery),
[memory protection constants](https://learn.microsoft.com/en-us/windows/win32/memory/memory-protection-constants),
[QueryWorkingSetEx](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-queryworkingsetex).

The experiment moves work into startup. Finite success after Broker readiness
cannot establish the 400 ms activation budget, the first-switch target, cold-boot
behavior, or loaded-TIP VM acceptance. See the evidence record before considering
any production design change.

## Shared prebuilt Actor fixtures

The historical page experiments used a separate compiled `user/build` per
profile. The installed plan uses machine prebuilt data for both staging and
fallback. The Actor probe now supports
`--broker-plan --machine-prebuilt <absolute-fixture-dir>` to model those two
config fields while keeping each user profile separate. The user directory
requires `mo-latency-fixture`; the compiled directory requires
`mo-latency-prebuilt-fixture`, `default.yaml`, and `rime_ice.schema.yaml`.
Use a disposable copy of the pinned compiled data and verify that every file
remains unchanged. This does not exercise Program Files ACLs or installed startup.

## Synthetic DB forwarding/reopen probe

`DbIoProbe.vcxproj` builds an x64 C++17 test with `/W4 /WX` and `/MT`.
Pass explicit pinned `MoLevelDbIncludeDir` and `MoLevelDbLibrary`, an isolated
`MoDiagnosticIncludeDir` containing `rime/mo_diagnostic.h` copied from this
folder, and a fresh `MoDiagnosticOutDir`. For the current host experiment:

```powershell
& 'C:/Program Files/Microsoft Visual Studio/2022/Community/MSBuild/Current/Bin/MSBuild.exe' `
  native/librime/diagnostics/DbIoProbe.vcxproj /nologo /m:1 /nodeReuse:false `
  /p:Configuration=Release /p:Platform=x64 `
  "/p:MoLevelDbIncludeDir=$PWD/build/mo-runtime-learning-v1/prefix/include" `
  "/p:MoLevelDbLibrary=$PWD/build/mo-runtime-learning-v1/prefix/lib/leveldb.lib" `
  "/p:MoDiagnosticIncludeDir=$PWD/build/runtime-component-trace-v1/source/src" `
  "/p:MoDiagnosticOutDir=$PWD/build/my-db-probe-bin"
```

Run `mo_db_io_probe.exe` with one absolute disposable directory containing a
`mo-db-io-fixture` marker and no `synthetic-db` child. The probe checks synchronous
synthetic writes/readback, legacy-to-reuse and reuse-to-legacy reopens, concurrent
lock rejection, missing-DB rejection, and failed file-creation status propagation.
It retains synthetic DB files for inspection and logs no key, value, or path.
These checks do not establish crash/power-loss recovery or large-userdb behavior.

`DbEnv` belongs to the librime DB wrapper and outlives the opened DB; it borrows
LevelDB's default Env. File wrappers own and release the underlying file exactly
once. Unoverridden operations keep EnvWrapper's forwarding behavior. The logger
and random-access file are not wrapped after creation. All original statuses are
returned; contents, names, byte counts, protection and Sync implementation are
unchanged by tracing. Only the separate reuse flag changes a native option.

The pinned LevelDB 1.23 source labels `reuse_logs` experimental. Retain its
normal fresh/large/failed-append fallback. Do not add this option to production
without crash/error recovery and representative profile tests. Current findings
and the startup-budget failures are recorded in the DB-open evidence document.
