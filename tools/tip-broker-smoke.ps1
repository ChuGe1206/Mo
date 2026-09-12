[CmdletBinding()]
param(
    [ValidateSet('All', 'x64', 'Win32')]
    [string]$Architecture = 'All',
    [ValidatePattern('^[A-Za-z0-9._-]+$')]
    [string]$RustToolchain = 'stable'
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot

& (Join-Path $repoRoot 'native\windows-tip\build-probe.ps1') -Architecture $Architecture -Backend MSBuild
if ($LASTEXITCODE -ne 0) { throw "TIP build/probe failed: $LASTEXITCODE" }

Push-Location $repoRoot
try {
    & cargo "+$RustToolchain" build -p mo-broker --bin mo-broker
    if ($LASTEXITCODE -ne 0) { throw "Rust broker build failed: $LASTEXITCODE" }
} finally {
    Pop-Location
}

$broker = Join-Path $repoRoot 'target\debug\mo-broker.exe'
$platforms = if ($Architecture -eq 'All') { @('x64', 'Win32') } else { @($Architecture) }
foreach ($platform in $platforms) {
    $probe = Join-Path $repoRoot "native\windows-tip\out\msbuild\$platform\Release\mo_tip_ipc_probe.exe"
    if (-not (Test-Path -LiteralPath $probe)) { throw "Missing IPC probe: $probe" }

    $process = Start-Process -FilePath $broker -ArgumentList '--fake' -PassThru -WindowStyle Hidden
    try {
        & $probe
        if ($LASTEXITCODE -ne 0) { throw "$platform IPC probe failed: $LASTEXITCODE" }
        if (-not $process.WaitForExit(5000)) { throw "$platform broker did not exit after client close" }
        if ($process.ExitCode -ne 0) { throw "$platform broker failed: $($process.ExitCode)" }
    } finally {
        if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
        $process.Dispose()
    }
}

Write-Host "C++ $($platforms -join '/') clients completed real framed I/O with the Rust named-pipe broker."
