#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$StageDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateRange(1, 100)][int]$Repetitions = 20,
    [switch]$LatencyTrace,
    [switch]$Fake,
    [switch]$ActivatingTestHost,
    [ValidatePattern('^[A-Za-z0-9._-]+$')][string]$RustToolchain = 'stable'
)
$ErrorActionPreference = 'Stop'
# Match the existing native builders: normalize the case-duplicated PATH
# inherited on some Windows hosts, only in this script's process.
$lifecycleProcessPath = [Environment]::GetEnvironmentVariable('PATH', 'Process')
Remove-Item Env:Path -ErrorAction SilentlyContinue
$env:Path = $lifecycleProcessPath
$repo = Split-Path -Parent $PSScriptRoot
. (Join-Path $repo 'installer/windows/staging-policy.ps1')
. (Join-Path $repo 'installer/windows/test-fixture.ps1')
. (Join-Path $PSScriptRoot 'broker-fault-harness.ps1')
$stage = Assert-MoPlainPath $StageDirectory
$null = Assert-MoPreparedStage $stage
$output = Assert-MoNewBuildOutput $OutputDirectory $repo
$vswhere = Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio/Installer/vswhere.exe'
$visualStudio = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -ne 0 -or -not $visualStudio) { throw 'Visual Studio discovery failed.' }
$msbuild = Assert-MoPlainPath (Join-Path $visualStudio 'MSBuild/Current/Bin/MSBuild.exe')
$lifecyclePreviousAutoInstall = [Environment]::GetEnvironmentVariable('RUSTUP_AUTO_INSTALL', 'Process')
try {
    [Environment]::SetEnvironmentVariable('RUSTUP_AUTO_INSTALL', '0', 'Process')
    $lifecycleCompilerVersion = & rustup run $RustToolchain rustc --version
    if ($LASTEXITCODE -ne 0 -or $lifecycleCompilerVersion -notmatch '^rustc 1\.97\.1 \(') {
        throw 'Lifecycle probes require the installed pinned Rust 1.97.1; no auto-install.'
    }
} finally {
    if ($null -eq $lifecyclePreviousAutoInstall) {
        Remove-Item Env:RUSTUP_AUTO_INSTALL -ErrorAction SilentlyContinue
    } else { [Environment]::SetEnvironmentVariable('RUSTUP_AUTO_INSTALL', $lifecyclePreviousAutoInstall, 'Process') }
}
New-Item -ItemType Directory -Path $output | Out-Null
$payload = Join-Path $stage 'payload/Mo'
$dll = Join-Path $payload 'runtime/librime/rime.dll'
$shared = Join-Path $payload 'data/rime-ice'
$traceProperty = if ($LatencyTrace) { 'true' } else { 'false' }
$cargoTarget = Join-Path $output 'cargo'
Push-Location $repo
try {
    $arguments = @("+$RustToolchain", 'build', '--offline', '--locked', '--target-dir', $cargoTarget, '-p', 'mo-broker', '--bin', 'mo-broker')
    if ($LatencyTrace) { $arguments += @('--features', 'latency-trace') }
    & cargo @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Lifecycle diagnostic Broker build failed.' }
} finally { Pop-Location }
foreach ($platform in @('x64', 'Win32')) {
    foreach ($project in @('MoTip', 'MoTipAbiProbe')) {
        $native = Join-Path $output "native/$platform"
        $objects = Join-Path $output "objects/$platform/$project"
        & $msbuild (Join-Path $repo "native/windows-tip/$project.vcxproj") /t:Rebuild /m /nologo /v:minimal `
            /p:Configuration=Release "/p:Platform=$platform" "/p:MoLatencyTrace=$traceProperty" `
            "/p:OutDir=$native\" "/p:IntDir=$objects\"
        if ($LASTEXITCODE -ne 0) { throw "Lifecycle $project/$platform build failed." }
    }
}
# Test-only installed-like sibling identity; never replace the release stage
# Broker, register a system profile, or change the user's input preferences.
$fixture = Join-Path $output 'fixture-墨'
$testMo = Join-Path $fixture 'Mo'
$broker = Join-Path $testMo 'bin/mo-broker.exe'
try {
    New-Item -ItemType Directory -Path (Join-Path $testMo 'bin') | Out-Null
    Copy-Item -LiteralPath (Join-Path $cargoTarget 'debug/mo-broker.exe') -Destination $broker
    foreach ($platform in @('x64', 'Win32')) {
        $architecture = if ($platform -eq 'x64') { 'x64' } else { 'x86' }
        $tip = Join-Path $testMo "tip/$architecture/mo-tip.dll"
        New-Item -ItemType Directory -Path (Split-Path -Parent $tip) | Out-Null
        $sourceTip = Join-Path $output "native/$platform/mo_tip.dll"
        Copy-Item -LiteralPath $sourceTip -Destination $tip
        if ((Get-FileHash -LiteralPath $sourceTip).Hash -ine (Get-FileHash -LiteralPath $tip).Hash) { throw 'Lifecycle TIP copy mismatch.' }
        $probe = Join-Path $output "native/$platform/mo_tip_abi_probe.exe"
        & $probe $tip
        if ($LASTEXITCODE -ne 0) { throw "Lifecycle ABI/diagnostic-mode checks failed: $platform" }
        for ($trial = 1; $trial -le $Repetitions; ++$trial) {
            Write-Host "MO_LIFECYCLE platform=$platform trial=$trial/$Repetitions trace=$traceProperty fake=$([bool]$Fake) activating=$([bool]$ActivatingTestHost)"
            $user = Join-Path $fixture "user-$platform-$trial"
            New-Item -ItemType Directory -Path $user | Out-Null
            $brokerArguments = @('--fake')
            if (-not $Fake) {
                Copy-Item -LiteralPath (Join-Path $shared 'build') -Destination (Join-Path $user 'build') -Recurse
                $brokerArguments = @('--rime-prepared', $dll, $shared, $user)
            }
            Invoke-MoBrokerFaultProbe $broker $brokerArguments $probe $tip -RimeIce:(-not $Fake) -LatencyTrace:$LatencyTrace -ActivatingTestHost:$ActivatingTestHost
            # Only this trial's owned data. Keep outputs/logs, not user fixtures.
            Assert-MoOwnedFixtureTree $user
            Remove-Item -LiteralPath $user -Recurse -Force
        }
    }
    $null = Assert-MoPreparedStage $stage
    Write-Host "MO_LIFECYCLE_PASS rounds_per_arch=$Repetitions trace=$traceProperty fake=$([bool]$Fake) No system registration; unchanged 50ms key deadline."
} finally {
    if (Test-Path -LiteralPath $fixture) {
        $resolved = Assert-MoPlainPath $fixture
        if (-not $resolved.StartsWith($output + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe lifecycle fixture cleanup target.' }
        Assert-MoOwnedFixtureTree $resolved
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
