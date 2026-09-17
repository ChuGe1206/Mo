# Build-time OpenCC resources

This x64 utility compiles the locked rime-ice Emoji dictionaries to OpenCC
`.ocd2`, then reloads each result and verifies **every key and ordered value
list**. It does not change the schema, disable Emoji, or send warm-up keys to
user sessions. It is never linked into or shipped with the TIP/Broker.

Use PowerShell 7 and Visual Studio 2022 C++ tools. Download the explicit archive
from the URL recorded in `third_party/manifest.toml`; build scripts do not
download anything. The source ZIP must have SHA-256
`f8aac3eda054edaf0313aa2103baa965f63f80f4a496b35ac7b632dfd1a33953`.

```powershell
./tools/opencc-build/build.ps1 -SourceArchivePath <OpenCC-1.1.9-source.zip>
./tools/compile-opencc-data.ps1 `
  -SharedDataDir <locked-rime-ice> `
  -CompilerPath ./build/opencc-dict-tool/bin/mo_opencc_dict.exe `
  -OutputDirectory ./build/compiled-opencc
./tools/test-opencc-data.ps1 `
  -OpenccDataDir ./build/compiled-opencc `
  -CompilerPath ./build/opencc-dict-tool/bin/mo_opencc_dict.exe
```

The compiler refuses existing output files and relative paths. Rebuilding the
tool requires every extracted upstream file to match the ZIP; modified/extra
files are rejected. Upstream OpenCC 1.1.9/Marisa sources remain unmodified,
including their notices. This minimal MSBuild target builds only dictionary
classes and bundled Marisa 0.2.6, avoiding a CMake installation. Mo's wrapper
uses `/W4 /WX`; unmodified Marisa has four C4267 warnings, accepted only for
this build-time utility. Production TIP warning policy is unchanged.

Generated packs retain the original three source files, original dictionary
group priority and segmentation chain, plus output hashes and compiler image
hash. Invalid/missing/corrupt packs are rejected before smoke starts; they are
copied into fresh test-owned user directories, never a real profile. The
manifest is a build integrity record, **not a signature, trusted update
manifest, generated SBOM or redistribution approval**. Keep corresponding
sources, attribution and GPL resource notices in the eventual distribution.

Both `tip-rime-smoke.ps1` and `rime-latency-probe.ps1` accept
`-OpenccDataDir <pack>`. The latter supports `-OmitEmoji` for disposable-copy
ablation (never a product default) and `-KeepResources` for an input-free
resource owner. Diagnostic timings contain no text, keys or paths.
