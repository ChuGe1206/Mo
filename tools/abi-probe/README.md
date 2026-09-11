# librime ABI probe

This probe compares the committed Rust declarations with the exact official
`rime_api.h` chosen for the build.  It compiles only a C executable and does not
download or link librime.  Before compiling, it also checks the header against
the SHA-256 recorded in `native/librime/UPSTREAM.toml`.

On Windows, the script uses CMake when available and otherwise locates MSVC via
`vswhere.exe`:

```powershell
./tools/abi-probe/compare.ps1 `
  -RimeIncludeDir C:/path/to/librime/src
```

Run the same comparison for every supported target architecture.  In
particular, x86 results must not be inferred from x64 results because
`RimeSessionId` is `uintptr_t` and therefore pointer-width dependent.
For example, after installing the Rust target:

```powershell
./tools/abi-probe/compare.ps1 `
  -RimeIncludeDir C:/path/to/librime/src `
  -MsvcArch x86 `
  -RustTarget i686-pc-windows-msvc
```
