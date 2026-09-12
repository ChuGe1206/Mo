[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$LibrimeDistDir,

    [Parameter(Mandatory = $true)]
    [string]$SharedDataDir,

    [Parameter(Mandatory = $true)]
    [string]$UserDataDir,

    [ValidateSet('All', 'x64', 'Win32')]
    [string]$Architecture = 'All',

    [ValidatePattern('^[A-Za-z0-9._-]+$')]
    [string]$RustToolchain = 'stable',

    [switch]$Deploy
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot

function Resolve-Directory([string]$Path, [string]$Label) {
    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        throw "$Label does not exist: $Path"
    }
    return (Resolve-Path -LiteralPath $Path).Path
}

function Require-File([string]$Path, [string]$Label) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Label does not exist: $Path"
    }
    return (Resolve-Path -LiteralPath $Path).Path
}

function Invoke-Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $Program"
    }
}

$dist = Resolve-Directory $LibrimeDistDir 'librime dist directory'
$shared = Resolve-Directory $SharedDataDir 'Rime shared data directory'
$user = Resolve-Directory $UserDataDir 'Rime user data directory'
$libraryDirectory = Resolve-Directory (Join-Path $dist 'lib') 'librime library directory'
$dynamicLibrary = Require-File (Join-Path $libraryDirectory 'rime.dll') 'librime runtime DLL'

if ($Deploy) {
    $deployer = Require-File (Join-Path $dist 'bin\rime_deployer.exe') 'Rime deployer'
    Push-Location $libraryDirectory
    try {
        Invoke-Checked $deployer @('--build', $user, $shared, (Join-Path $user 'build'))
    } finally {
        Pop-Location
    }
}

foreach ($required in @(
    (Join-Path $shared 'default.yaml'),
    (Join-Path $shared 'rime_ice.schema.yaml'),
    (Join-Path $user 'build\default.yaml'),
    (Join-Path $user 'build\rime_ice.schema.yaml')
)) {
    [void](Require-File $required 'required rime-ice deployment input')
}

& (Join-Path $repoRoot 'native\windows-tip\build-probe.ps1') -Architecture $Architecture -Backend MSBuild
if ($LASTEXITCODE -ne 0) { throw "TIP build/probe failed: $LASTEXITCODE" }

Push-Location $repoRoot
try {
    & cargo "+$RustToolchain" build -p mo-broker --bin mo-broker
    if ($LASTEXITCODE -ne 0) { throw "Rust broker build failed: $LASTEXITCODE" }
} finally {
    Pop-Location
}

$brokerPath = Join-Path $repoRoot 'target\debug\mo-broker.exe'

function Invoke-RimeBrokerProbe(
    [string]$Platform,
    [string]$Probe,
    [string[]]$ProbeArguments,
    [string]$Label
) {
    if (-not (Test-Path -LiteralPath $Probe -PathType Leaf)) {
        throw "Missing ${Label}: $Probe"
    }
    $probeUser = Join-Path $user ("mo-smoke-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $probeUser | Out-Null
    Copy-Item -LiteralPath (Join-Path $user 'build') -Destination (Join-Path $probeUser 'build') -Recurse
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $brokerPath
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardError = $true
    [void]$startInfo.ArgumentList.Add('--rime')
    [void]$startInfo.ArgumentList.Add($dynamicLibrary)
    [void]$startInfo.ArgumentList.Add($shared)
    [void]$startInfo.ArgumentList.Add($probeUser)
    $process = [System.Diagnostics.Process]::Start($startInfo)
    if ($null -eq $process) { throw "Failed to start Broker for $Platform $Label" }

    try {
        # Wait for the Broker's explicit readiness line rather than guessing at
        # librime startup time. This wait belongs to the harness, not the TIP.
        $readyTask = $process.StandardError.ReadLineAsync()
        if (-not $readyTask.Wait(30000)) {
            throw "$Platform Broker readiness timed out during $Label"
        }
        $readyLine = $readyTask.Result
        if ($readyLine -notmatch ' listening on ') {
            $brokerError = $process.StandardError.ReadToEnd()
            throw "$Platform Broker failed before ${Label}: $readyLine $brokerError"
        }
        & $Probe @ProbeArguments
        if ($LASTEXITCODE -ne 0) { throw "$Platform $Label failed: $LASTEXITCODE" }
        if (-not $process.WaitForExit(5000)) {
            throw "$Platform Broker did not exit after $Label closed"
        }
        if ($process.ExitCode -ne 0) {
            throw "$Platform Broker failed during ${Label}: $($process.ExitCode)"
        }
    } finally {
        if (-not $process.HasExited) { $process.Kill($true) }
        $process.Dispose()
        if (Test-Path -LiteralPath $probeUser -PathType Container) {
            $resolvedProbeUser = (Resolve-Path -LiteralPath $probeUser).Path
            $expectedPrefix = $user.TrimEnd('\') + '\'
            if (-not $resolvedProbeUser.StartsWith(
                    $expectedPrefix,
                    [System.StringComparison]::OrdinalIgnoreCase)) {
                throw "Refusing to clean probe directory outside disposable user root: $resolvedProbeUser"
            }
            Remove-Item -LiteralPath $resolvedProbeUser -Recurse -Force
        }
    }
}

$platforms = if ($Architecture -eq 'All') { @('x64', 'Win32') } else { @($Architecture) }
foreach ($platform in $platforms) {
    $binaryDirectory = Join-Path $repoRoot "native\windows-tip\out\msbuild\$platform\Release"
    $ipcProbe = Join-Path $binaryDirectory 'mo_tip_ipc_probe.exe'
    $abiProbe = Join-Path $binaryDirectory 'mo_tip_abi_probe.exe'
    $tip = Join-Path $binaryDirectory 'mo_tip.dll'
    Invoke-RimeBrokerProbe $platform $ipcProbe @('--rime-ice') 'rime-ice IPC probe'
    Invoke-RimeBrokerProbe $platform $abiProbe @($tip, '--broker-rime-ice') 'rime-ice TIP edit-session probe'
}

Write-Host "C++ $($platforms -join '/') clients committed nihao -> 你好 through TIP -> Broker -> librime/rime-ice."
