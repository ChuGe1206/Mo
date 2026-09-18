#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$BeforeStageDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$reentryProcessPath = [Environment]::GetEnvironmentVariable('PATH', 'Process')
Remove-Item Env:Path -ErrorAction SilentlyContinue
$env:Path = $reentryProcessPath
$repo = Split-Path -Parent $PSScriptRoot
. (Join-Path $repo 'installer/windows/staging-policy.ps1')
. (Join-Path $repo 'installer/windows/test-fixture.ps1')
. (Join-Path $PSScriptRoot 'broker-fault-harness.ps1')
$stage = Assert-MoPlainPath $BeforeStageDirectory
$null = Assert-MoPreparedStage $stage
$output = Assert-MoNewBuildOutput $OutputDirectory $repo
$sourceBroker = Assert-MoPlainPath (Join-Path $repo 'target/debug/mo-broker.exe')
$vswhere = Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio/Installer/vswhere.exe'
$visualStudio = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -ne 0 -or -not $visualStudio) { throw 'Regression Visual Studio discovery failed.' }
$msbuild = Assert-MoPlainPath (Join-Path $visualStudio 'MSBuild/Current/Bin/MSBuild.exe')
New-Item -ItemType Directory -Path $output | Out-Null
$fixture = Join-Path $output 'fixture-墨'
try {
    $testMo = Join-Path $fixture 'Mo'
    New-Item -ItemType Directory -Path (Join-Path $testMo 'bin') | Out-Null
    $broker = Join-Path $testMo 'bin/mo-broker.exe'
    Copy-Item -LiteralPath $sourceBroker -Destination $broker
    if ((Get-FileHash -LiteralPath $broker).Hash -ine (Get-FileHash -LiteralPath $sourceBroker).Hash) { throw 'Regression Broker copy mismatch.' }
    foreach ($platform in @('x64', 'Win32')) {
        $native = Join-Path $output "native/$platform"
        $objects = Join-Path $output "objects/$platform"
        & $msbuild (Join-Path $repo 'native/windows-tip/MoTipAbiProbe.vcxproj') /t:Rebuild /m /nologo /v:minimal `
            /p:Configuration=Release "/p:Platform=$platform" /p:MoLatencyTrace=false "/p:OutDir=$native\" "/p:IntDir=$objects\"
        if ($LASTEXITCODE -ne 0) { throw 'Regression probe compilation failed.' }
        $architecture = if ($platform -eq 'x64') { 'x64' } else { 'x86' }
        $sourceTip = Join-Path $stage "payload/Mo/tip/$architecture/mo-tip.dll"
        $tip = Join-Path $testMo "tip/$architecture/mo-tip.dll"
        New-Item -ItemType Directory -Path (Split-Path -Parent $tip) | Out-Null
        Copy-Item -LiteralPath $sourceTip -Destination $tip
        if ((Get-FileHash -LiteralPath $tip).Hash -ine (Get-FileHash -LiteralPath $sourceTip).Hash) { throw 'Regression old TIP copy mismatch.' }
        $expectedFailure = $null
        try { Invoke-MoBrokerFaultProbe $broker @('--fake') (Join-Path $native 'mo_tip_abi_probe.exe') $tip }
        catch { $expectedFailure = $_.Exception.Message }
        # A random UI/transport failure is NOT evidence that the write-reentry
        # guard works. Require the precise injected callback's negative result.
        if ($null -eq $expectedFailure -or $expectedFailure -notmatch 'MO_TERMINATION_REENTRY fired=1 fail_open=0') {
            throw "Expected old-TIP write-reentry failure missing: $platform"
        }
        Write-Host "MO_REENTRY_BEFORE_CONFIRMED platform=$platform tip_sha256=$((Get-FileHash -LiteralPath $sourceTip).Hash)"
    }
    $null = Assert-MoPreparedStage $stage
    Write-Host 'Old TIP write-reentry regression confirmed on both architectures. No system registration.'
} finally {
    if (Test-Path -LiteralPath $fixture) {
        $resolved = Assert-MoPlainPath $fixture
        if (-not $resolved.StartsWith($output + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe regression cleanup target.' }
        Assert-MoOwnedFixtureTree $resolved
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
