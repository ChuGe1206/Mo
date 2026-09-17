[CmdletBinding()]
param(
    [ValidateSet('All', 'x64', 'Win32')]
    [string]$Architecture = 'All',
    [ValidatePattern('^[A-Za-z0-9._-]+$')]
    [string]$RustToolchain = 'stable',
    [switch]$Registered,
    [ValidateRange(1, 100)]
    [int]$FaultRepetitions = 1
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'registered-tip-state.ps1')
. (Join-Path $PSScriptRoot 'broker-fault-harness.ps1')

if ($Registered -and $Architecture -ne 'All') {
    throw 'Registered smoke requires -Architecture All so both COM views are present.'
}
if ($Registered) {
    Assert-MoRegisteredUserPreflight (Join-Path $repoRoot 'native\windows-tip\out\msbuild\x64\Release\mo_tip_registrar.exe')
}

& (Join-Path $repoRoot 'native\windows-tip\build-probe.ps1') -Architecture $Architecture -Backend MSBuild
if ($LASTEXITCODE -ne 0) { throw "TIP build/probe failed: $LASTEXITCODE" }

$registrar = $null
$tipX64 = $null
$tipX86 = $null
if ($Registered) {
    $x64Directory = Join-Path $repoRoot 'native\windows-tip\out\msbuild\x64\Release'
    $x86Directory = Join-Path $repoRoot 'native\windows-tip\out\msbuild\Win32\Release'
    $registrar = Join-Path $x64Directory 'mo_tip_registrar.exe'
    $tipX64 = Join-Path $x64Directory 'mo_tip.dll'
    $tipX86 = Join-Path $x86Directory 'mo_tip.dll'
    foreach ($artifact in @($registrar, $tipX64, $tipX86)) {
        if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) {
            throw "Missing registered smoke artifact: $artifact"
        }
    }

}

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
        if ($process.HasExited -and $process.ExitCode -ne 0) {
            throw "$Platform Broker exited unexpectedly during ${Label}: $($process.ExitCode)"
        }
    } finally {
        try {
            if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
            if (-not $process.WaitForExit(3000)) { throw 'Owned fake Broker did not exit before the next probe.' }
        } finally { $process.Dispose() }
    }
}

$platforms = if ($Architecture -eq 'All') { @('x64', 'Win32') } else { @($Architecture) }
foreach ($platform in $platforms) {
    $binaryDirectory = Join-Path $repoRoot "native\windows-tip\out\msbuild\$platform\Release"
    $ipcProbe = Join-Path $binaryDirectory 'mo_tip_ipc_probe.exe'
    $abiProbe = Join-Path $binaryDirectory 'mo_tip_abi_probe.exe'
    $tip = Join-Path $binaryDirectory 'mo_tip.dll'
    Invoke-BrokerProbe $platform $ipcProbe @($broker, '--reject-unexpected') 'IPC server identity rejection probe'
    Invoke-BrokerProbe $platform $ipcProbe @($broker) 'IPC probe'
    Invoke-BrokerProbe $platform $ipcProbe @($broker, '--pool') '16-client pipe pool probe'
    Invoke-BrokerProbe $platform $abiProbe @($tip, '--broker-input') 'TIP edit-session probe'
    foreach ($faultTrial in 1..$FaultRepetitions) {
        Write-Host "$platform fake fault trial $faultTrial/$FaultRepetitions"
        Invoke-MoBrokerFaultProbe $broker @('--fake') $abiProbe $tip
    }
}

Write-Host "C++ $($platforms -join '/') IPC, pool and TIP UI/edit checks passed, including $FaultRepetitions fault repetitions per architecture (two Broker crashes/restarts each) and no commit replay."

if ($Registered) {
    Invoke-MoRegisteredUserTest $registrar $tipX64 $tipX86 {
        foreach ($platform in @('x64', 'Win32')) {
            $binaryDirectory = Join-Path $repoRoot "native\windows-tip\out\msbuild\$platform\Release"
            $abiProbe = Join-Path $binaryDirectory 'mo_tip_abi_probe.exe'
            Invoke-BrokerProbe $platform $abiProbe @('--registered-broker-input') 'registered TSF system-key route probe'
        }
    }

    Write-Host 'Registered x64/Win32 TSF system-key routes passed; temporary current-user enablement and HKCU COM state were removed.'
    Write-Host 'The machine profile/category remains registered; remove it from an elevated PowerShell with tools\machine-profile.ps1 -Action Unregister.'
}
