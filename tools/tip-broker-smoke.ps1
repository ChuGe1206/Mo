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

function Invoke-BrokerProbe(
    [string]$Platform,
    [string]$Probe,
    [string[]]$ProbeArguments,
    [string]$Label
) {
    if (-not (Test-Path -LiteralPath $Probe -PathType Leaf)) {
        throw "Missing ${Label}: $Probe"
    }
    $process = Start-Process -FilePath $broker -ArgumentList '--fake' -PassThru -WindowStyle Hidden
    try {
        # TIP activation intentionally has a small fail-open deadline. Allow a
        # freshly spawned Broker to create the pipe before loading the TIP.
        Start-Sleep -Milliseconds 250
        & $Probe @ProbeArguments
        if ($LASTEXITCODE -ne 0) { throw "$Platform $Label failed: $LASTEXITCODE" }
        if (-not $process.WaitForExit(5000)) {
            throw "$Platform Broker did not exit after $Label closed"
        }
        if ($process.ExitCode -ne 0) {
            throw "$Platform Broker failed during ${Label}: $($process.ExitCode)"
        }
    } finally {
        if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
        $process.Dispose()
    }
}

$platforms = if ($Architecture -eq 'All') { @('x64', 'Win32') } else { @($Architecture) }
foreach ($platform in $platforms) {
    $binaryDirectory = Join-Path $repoRoot "native\windows-tip\out\msbuild\$platform\Release"
    $ipcProbe = Join-Path $binaryDirectory 'mo_tip_ipc_probe.exe'
    $abiProbe = Join-Path $binaryDirectory 'mo_tip_abi_probe.exe'
    $tip = Join-Path $binaryDirectory 'mo_tip.dll'
    Invoke-BrokerProbe $platform $ipcProbe @() 'IPC probe'
    Invoke-BrokerProbe $platform $abiProbe @($tip, '--broker-input') 'TIP edit-session probe'
}

Write-Host "C++ $($platforms -join '/') clients completed framed I/O and TIP edit-session commits through the Rust Broker."
