[CmdletBinding()]
param(
    [ValidateSet('All', 'x64', 'Win32')]
    [string]$Architecture = 'All',
    [ValidatePattern('^[A-Za-z0-9._-]+$')]
    [string]$RustToolchain = 'stable',
    [switch]$Registered
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot

if ($Registered -and $Architecture -ne 'All') {
    throw 'Registered smoke requires -Architecture All so both COM views are present.'
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

    $initialStatus = (& $registrar status | Out-String)
    if ($LASTEXITCODE -ne 0) { throw "Registrar status failed: $LASTEXITCODE" }
    $initialStatusLines = @($initialStatus -split "\r?\n" | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    $cleanStatus = @(
        'com.x64=missing',
        'com.x86=missing',
        'profile.registered=true',
        'profile.enabled=false',
        'profile.active=false'
    )
    foreach ($line in $cleanStatus) {
        if ($initialStatusLines -notcontains $line) {
            throw "Registered smoke requires a clean user state and a pre-registered machine profile. Run tools\machine-profile.ps1 -Action Register from an elevated PowerShell first.`n$initialStatus"
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
    Invoke-BrokerProbe $platform $ipcProbe @($broker, '--reject-unexpected') 'IPC server identity rejection probe'
    Invoke-BrokerProbe $platform $ipcProbe @($broker) 'IPC probe'
    Invoke-BrokerProbe $platform $ipcProbe @($broker, '--pool') '16-client pipe pool probe'
    Invoke-BrokerProbe $platform $abiProbe @($tip, '--broker-input') 'TIP edit-session probe'
}

Write-Host "C++ $($platforms -join '/') IPC, 16-client pool capacity/isolation/reuse and TIP candidate window, mouse paging/selection, layout, deferred cancellation and reconnect checks passed."

if ($Registered) {
    $comOwned = $false
    $enabledOwned = $false
    try {
        & $registrar register-com-user $tipX64 $tipX86
        if ($LASTEXITCODE -ne 0) { throw "COM registration failed: $LASTEXITCODE" }
        $comOwned = $true

        & $registrar enable-current-user
        if ($LASTEXITCODE -ne 0) { throw "Current-user profile enable failed: $LASTEXITCODE" }
        $enabledOwned = $true

        foreach ($platform in @('x64', 'Win32')) {
            $binaryDirectory = Join-Path $repoRoot "native\windows-tip\out\msbuild\$platform\Release"
            $abiProbe = Join-Path $binaryDirectory 'mo_tip_abi_probe.exe'
            Invoke-BrokerProbe $platform $abiProbe @('--registered-broker-input') 'registered TSF system-key route probe'
        }
    } finally {
        if ($enabledOwned) {
            & $registrar disable-current-user
            if ($LASTEXITCODE -ne 0) { Write-Warning "Profile disable cleanup failed: $LASTEXITCODE" }
        }
        if ($comOwned) {
            & $registrar unregister-com-user
            if ($LASTEXITCODE -ne 0) { Write-Warning "COM unregister cleanup failed: $LASTEXITCODE" }
        }
        & $registrar status
    }

    Write-Host 'Registered x64/Win32 TSF system-key routes passed; temporary current-user enablement and HKCU COM state were removed.'
    Write-Host 'The machine profile/category remains registered; remove it from an elevated PowerShell with tools\machine-profile.ps1 -Action Unregister.'
}
