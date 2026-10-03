# Core + Lua development runtime

This Windows x64 builder uses only exact Git objects from the locked librime,
four dependency submodules and Lua plugin. It exports fresh archives, checks
every source archive hash, extracts into a new repository `build/` child and
applies the tracked resource, Lua build/data-boundary and learning-policy patches. Mutable checkouts, old headers/libs and other
plugins are not build inputs. No downloads or overwrites occur in the builder.
Run the scripts with PowerShell 7 (`pwsh`). Mo sources/patches and the validated
Emoji pack are snapshotted under the new output's `inputs/` before compilation.

Prepare these **explicit** inputs:

- librime `33e78140250125871856cdc5b42ddc6a5fcd3cd4` with only
  `deps/leveldb`, `deps/marisa-trie`, `deps/opencc`, `deps/yaml-cpp` initialized.
- `plugins/lua` at `ec52e48ea18f11af37717a01c337f853215cf70b`.
- Official CMake 3.31.10 Windows x64 ZIP, Boost 1.84.0 ZIP, Lua 5.4.9 tar.gz.
  Archive hashes are fixed in the builder, not accepted from downloaded metadata.
- Visual Studio 2022 C++ tools and an explicit Python executable. Python is used
  by upstream OpenCC's standard dictionary generation, not by the Broker.
- The validated `compile-opencc-data.ps1` output, explicitly passed via
  `-OpenccDataDir`; user/shared directories are not conversion-resource inputs.

```powershell
./tools/runtime-build/build.ps1 -SourceDir build/mo-runtime-source `
  -CmakeArchivePath build/cmake-3.31.10-windows-x86_64.zip `
  -BoostArchivePath build/boost_1_84_0.zip -LuaArchivePath build/lua-5.4.9.tar.gz `
  -PythonPath C:/path/to/python.exe -OpenccDataDir build/compiled-opencc-6810e89 `
  -OutputDirectory build/mo-runtime-new

./tools/tip-rime-smoke.ps1 -LibrimeDistDir build/mo-runtime-new/dist `
  -SharedDataDir C:/path/to/pinned-rime-ice -UserDataDir C:/path/to/disposable-user `
  -Architecture All -PreparedResources -OpenccDataDir build/compiled-opencc-6810e89
```

The build enables only merged Lua and disables external plugins, separate
libraries and native content logging. The new C export wrapper uses `/W4 /WX`;
upstream code still emits warnings (including DLL-interface/size conversions)
and is not claimed warning-free. Source archives and upstream license files
remain under `inputs/`. Build provenance includes source/tool archives, the
snapshotted Mo inputs, patches, wrappers, CMake hook, DLL and every packaged
OpenCC resource hash; format 2 is not a signed release manifest.
Full command logs remain under `commands/`. Lua's signed stack-count fixes keep
`/sdl` enabled; its CLI `main` sources are excluded from the merged DLL. CMP0091
is set before every first project to keep static MSVC runtimes consistent.
The machine-data patch replaces Lua's search path with only shared `lua/?.lua`
and `lua/?/init.lua`, clears the native-module path and loads only shared
`rime.lua`. A source-policy parser rejects user/default search paths and ambiguous
entry points; provenance binds the policy version and patch hash.
The tracked learning patches gate core Memory and merged Lua Memory user-dictionary
writes on the session option `mo_disable_learning`. Run
`tools/test-learning-option.ps1` against a fresh build and prepared shared data;
it exports one disposable user dictionary after private, normal and private
sessions and checks entry count and frequency (0, 1, 1).
OpenCC uses the same pinned Marisa 0.3.1 as librime, via explicit include/library
paths. The builder verifies OpenCC did not replace that library with its bundled
0.2.6 copy. This is separate from the existing standalone dictionary compiler's
pinned 0.2.6 build; native runtime probes must read its resulting `.ocd2` pack.
Cache source-path guards reject foreign headers/libraries before native builds;
the source-path policy checks cover matches, foreign paths, missing and duplicate
entries. OpenCC explicitly uses C++17 for the pinned Marisa interface.
The deferred hook sets wrapper properties in `TARGET_DIRECTORY rime`, checks
their visibility there, and the builder verifies actual generated MSBuild
source-level W4/WX metadata for both wrappers and the OpenCC opener before
compilation. Seven policy cases prevent silently accepting missing, weaker,
duplicate or wrong-configuration source properties (ADR 0025).

`test-preparation.ps1` checks success, missing dictionary and missing export in
fresh Unicode fixtures with copied DLL/resources, including unchanged
input/commit, retained Emoji and exact `你好` submission. Optional
`-LegacyDistDir <previous-v1-dist>` verifies that v1 is not a fallback.
An explicit `-BrokerPath <debug-broker.exe>` also checks the failure cases exit
with the preparation error and never announce readiness; it does not build or
terminate any pre-existing Broker.
`test-policy.ps1` checks output scope, overwrite, archive, commit and pack rejection
without compiling or changing the source checkout.
`test-dictionary-compatibility.ps1` uses the runtime's own OpenCC/Marisa CLI to
compare every key and ordered value with source in both precompiled dictionaries.
`test-resource-file.ps1` compiles the same resource opener with `/W4 /WX /sdl`
and tests same-handle reads, non-inheritance, write/delete sharing denial,
Unicode roots, invalid leaves, junctions, hard links and size limits.
`test-relocation.ps1` verifies DLL/resource provenance, then tests genuine
Unicode relocation, valid/invalid cwd and user traps, missing standard data,
path escape/ADS/NUL, malformed/oversized JSON, UTF-8 and structural limits,
corrupt dictionaries and unsupported/duplicate resource owners. Only new
fixtures are mutated; original build inputs and Windows input state are preserved.

`mo_rime_prepare_resources_v2(uintptr_t session_id) -> int` returns exactly 1
only when an empty `rime_ice` session has prepared exactly one Emoji and one
traditionalization Simplifier owner. Rust resolves only v2, never v1.
Zero, busy/other-schema/unknown sessions or converter errors return 0. It does
not send keys, clear input, commit, mutate options or add upstream RimeApi slots.
Only Mo's private resource anchor calls it during Broker startup, inside the
existing startup watchdog and before readiness. The anchor keeps the owners
alive; user sessions remain distinct. Missing/failed preparation is fatal in
`--rime-prepared` and installed mode; ordinary debug `--rime` remains an explicit
legacy development comparison, not a prepared-mode fallback.

The runtime's fixed resource directory is **the loaded DLL's parent / `opencc`**,
not the Broker EXE's parent. Development output is `dist/lib/rime.dll` with
`dist/lib/opencc`; installed layout is `Mo/runtime/librime/rime.dll` with
`Mo/runtime/librime/opencc`. Move DLL and resources together. Mo Simplifiers
accept only `emoji.json` and `s2t.json`; dictionaries must be flat ASCII-named
`.ocd2` files on a local fixed drive. The strict path consumes the same verified,
read-only Win32 handle, rejects root/leaf reparse points and multiple hard links,
and never uses cwd, user/shared overrides or the build prefix. JSON is capped at
64 KiB, each dictionary at 64 MiB; JSON/dictionary/chain/group limits reject
unsupported structures. Legacy OpenCC CLI APIs retain upstream lookup behavior
for development tooling and are not the Mo runtime loading path.

**Development only, do not distribute.** Removing non-allowlisted plugins is not
license approval. Full transitive SBOM/notices, resource provenance, code
signatures, installation ACLs and update authentication remain separate gates.
Path/handle checks do not authenticate resource contents or establish a Lua
sandbox. Machine scripts remain trusted executable code. Build hashes are
provenance, not a trust root.

See ADR 0024/0025/0039 for measured results and acceptance boundaries.
