[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $RimeIncludeDir,

    [string] $BuildDir = (Join-Path ([IO.Path]::GetTempPath()) "mo-rime-abi-probe"),

    [ValidateSet("x64", "x86", "arm64")]
    [string] $MsvcArch = "x64",

    [string] $RustTarget = ""
)

$ErrorActionPreference = "Stop"

$header = Join-Path $RimeIncludeDir "rime_api.h"
if (-not (Test-Path -LiteralPath $header -PathType Leaf)) {
    throw "Official header not found: $header"
}

$upstreamFile = "$PSScriptRoot/../../native/librime/UPSTREAM.toml"
$hashDeclaration = Get-Content -LiteralPath $upstreamFile |
    Select-String -Pattern '^header_sha256 = "([0-9a-f]{64})"$'
if ($hashDeclaration.Count -ne 1) {
    throw "Expected exactly one header_sha256 in $upstreamFile"
}
$expectedHash = $hashDeclaration.Matches[0].Groups[1].Value
$actualHash = (Get-FileHash -LiteralPath $header -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualHash -ne $expectedHash) {
    throw "rime_api.h SHA-256 differs: expected $expectedHash, got $actualHash"
}

$runningOnWindows = [System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT
$vswhere = "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
$cmake = Get-Command cmake -ErrorAction SilentlyContinue
if ($runningOnWindows -and (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
    $visualStudio = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath
    if (-not $visualStudio) { throw "Visual Studio C tools were not found" }

    $devCommand = Join-Path $visualStudio "Common7/Tools/VsDevCmd.bat"
    $source = (Resolve-Path -LiteralPath "$PSScriptRoot/probe.c").Path
    New-Item -ItemType Directory -Path $BuildDir -Force | Out-Null
    $probe = Join-Path $BuildDir "mo_rime_abi_probe.exe"
    $object = Join-Path $BuildDir "probe.obj"
    $compile = "`"$devCommand`" -no_logo -arch=$MsvcArch -host_arch=x64 >nul " +
        "&& cl.exe /nologo /std:c11 /W4 /WX /I`"$RimeIncludeDir`" " +
        "`"$source`" /Fo:`"$object`" /Fe:`"$probe`""
    & $env:ComSpec /d /s /c $compile
    if ($LASTEXITCODE -ne 0) { throw "MSVC C ABI probe build failed" }
} elseif ($cmake) {
    cmake -S $PSScriptRoot -B $BuildDir "-DRIME_INCLUDE_DIR=$RimeIncludeDir"
    if ($LASTEXITCODE -ne 0) { throw "CMake configure failed" }

    cmake --build $BuildDir --config Release
    if ($LASTEXITCODE -ne 0) { throw "C ABI probe build failed" }

    $probeCandidates = @(
        (Join-Path $BuildDir "Release/mo_rime_abi_probe.exe"),
        (Join-Path $BuildDir "mo_rime_abi_probe.exe"),
        (Join-Path $BuildDir "mo_rime_abi_probe")
    )
    $probe = $probeCandidates |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1
} else {
    throw "CMake is required to build the C ABI probe on this platform"
}

if (-not $probe -or -not (Test-Path -LiteralPath $probe -PathType Leaf)) {
    throw "Built C ABI probe was not found under $BuildDir"
}

$cLayout = & $probe
if ($LASTEXITCODE -ne 0) { throw "C ABI probe failed" }

$cargoArguments = @(
    "+stable",
    "run",
    "--quiet",
    "--manifest-path", "$PSScriptRoot/../../crates/mo-rime-sys/Cargo.toml",
    "--example", "abi_layout"
)
if ($RustTarget) { $cargoArguments += @("--target", $RustTarget) }
$rustLayout = & cargo @cargoArguments
if ($LASTEXITCODE -ne 0) { throw "Rust ABI layout probe failed" }

$difference = Compare-Object -ReferenceObject $cLayout -DifferenceObject $rustLayout
if ($difference) {
    $difference | Format-Table -AutoSize | Out-String | Write-Error
    throw "C and Rust librime ABI layouts differ"
}

Write-Host "librime C/Rust ABI layouts match ($($cLayout.Count) assertions)."
