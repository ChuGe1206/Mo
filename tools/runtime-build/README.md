# Core + Lua development runtime

This Windows x64 builder uses only exact Git objects from the locked librime,
four dependency submodules and Lua plugin. It exports fresh archives, checks
every source archive hash, extracts into a new repository `build/` child and
applies the tracked resource and Lua build patches. Mutable checkouts, old headers/libs and other
plugins are not build inputs. No downloads or overwrites occur in the builder.

Prepare these **explicit** inputs:

- librime `33e78140250125871856cdc5b42ddc6a5fcd3cd4` with only
  `deps/leveldb`, `deps/marisa-trie`, `deps/opencc`, `deps/yaml-cpp` initialized.
- `plugins/lua` at `ec52e48ea18f11af37717a01c337f853215cf70b`.
- Official CMake 3.31.10 Windows x64 ZIP, Boost 1.84.0 ZIP, Lua 5.4.9 tar.gz.
  Archive hashes are fixed in the builder, not accepted from downloaded metadata.
- Visual Studio 2022 C++ tools and an explicit Python executable. Python is used
  by upstream OpenCC's standard dictionary generation, not by the Broker.

```powershell
./tools/runtime-build/build.ps1 -SourceDir build/mo-runtime-source `
  -CmakeArchivePath build/cmake-3.31.10-windows-x86_64.zip `
  -BoostArchivePath build/boost_1_84_0.zip -LuaArchivePath build/lua-5.4.9.tar.gz `
  -PythonPath C:/path/to/python.exe -OutputDirectory build/mo-runtime-new

./tools/tip-rime-smoke.ps1 -LibrimeDistDir build/mo-runtime-new/dist `
  -SharedDataDir C:/path/to/pinned-rime-ice -UserDataDir C:/path/to/disposable-user `
  -Architecture All -PreparedResources -OpenccDataDir build/compiled-opencc-6810e89
```

The build enables only merged Lua and disables external plugins, separate
libraries and native content logging. The new C export wrapper uses `/W4 /WX`;
upstream code still emits warnings (including DLL-interface/size conversions)
and is not claimed warning-free. Source archives and upstream license files
remain under `inputs/`. Build provenance includes source/tool archives, the
patches, wrapper, CMake hook and DLL hashes; it is not a signed release manifest.
Full command logs remain under `commands/`. Lua's signed stack-count fixes keep
`/sdl` enabled; its CLI `main` sources are excluded from the merged DLL. CMP0091
is set before every first project to keep static MSVC runtimes consistent.
OpenCC uses the same pinned Marisa 0.3.1 as librime, via explicit include/library
paths. The builder verifies OpenCC did not replace that library with its bundled
0.2.6 copy. This is separate from the existing standalone dictionary compiler's
pinned 0.2.6 build; native runtime probes must read its resulting `.ocd2` pack.
Cache source-path guards reject foreign headers/libraries before native builds;
the source-path policy checks cover matches, foreign paths, missing and duplicate
entries. OpenCC explicitly uses C++17 for the pinned Marisa interface.

`test-preparation.ps1` checks success, missing dictionary and missing export in
fresh Unicode fixtures, including unchanged input/commit and retained Emoji.
An explicit `-BrokerPath <debug-broker.exe>` also checks both failure cases exit
with the preparation error and never announce readiness; it does not build or
terminate any pre-existing Broker.
`test-policy.ps1` checks output scope, overwrite, archive and commit rejection
without compiling or changing the source checkout.
`test-dictionary-compatibility.ps1` uses the runtime's own OpenCC/Marisa CLI to
compare every key and ordered value with source in both precompiled dictionaries.

`mo_rime_prepare_resources_v1(uintptr_t session_id) -> int` returns exactly 1
only when an empty `rime_ice` session has prepared both actual Simplifier owners.
Zero, busy/other-schema/unknown sessions or converter errors return 0. It does
not send keys, clear input, commit, mutate options or add upstream RimeApi slots.
Only Mo's private resource anchor calls it during Broker startup, inside the
existing startup watchdog and before readiness. The anchor keeps the owners
alive; user sessions remain distinct. Missing/failed preparation is fatal in
`--rime-prepared` and installed mode; ordinary debug `--rime` remains an explicit
legacy development comparison, not a prepared-mode fallback.

**Development only, do not distribute.** Removing non-allowlisted plugins is not
license approval. Full transitive SBOM/notices, resource provenance, code
signatures, installation ACLs and relocation are separate gates. In particular,
upstream OpenCC still has build-prefix/CWD search behavior; this builder does
not yet establish a relocatable, fail-closed production resource loader.

See ADR 0024 for measured results and acceptance boundaries.
