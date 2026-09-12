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
$platforms = if ($Architecture -eq 'All') { @('x64', 'Win32') } else { @($Architecture) }
foreach ($platform in $platforms) {
    $probe = Join-Path $repoRoot "native\windows-tip\out\msbuild\$platform\Release\mo_tip_ipc_probe.exe"
    if (-not (Test-Path -LiteralPath $probe -PathType Leaf)) { throw "Missing IPC probe: $probe" }

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $brokerPath
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    [void]$startInfo.ArgumentList.Add('--rime')
    [void]$startInfo.ArgumentList.Add($dynamicLibrary)
    [void]$startInfo.ArgumentList.Add($shared)
    [void]$startInfo.ArgumentList.Add($user)
    $process = [System.Diagnostics.Process]::Start($startInfo)
    if ($null -eq $process) { throw "Failed to start Broker for $platform probe" }

    try {
        & $probe '--rime-ice'
        if ($LASTEXITCODE -ne 0) { throw "$platform rime-ice IPC probe failed: $LASTEXITCODE" }
        if (-not $process.WaitForExit(5000)) { throw "$platform Broker did not exit after client close" }
        if ($process.ExitCode -ne 0) { throw "$platform Broker failed: $($process.ExitCode)" }
    } finally {
        if (-not $process.HasExited) { $process.Kill($true) }
        $process.Dispose()
    }
}

Write-Host "C++ $($platforms -join '/') clients completed Broker -> librime -> rime-ice candidate and commit I/O."
