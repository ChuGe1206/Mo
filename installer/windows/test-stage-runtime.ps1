#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$BuildDirectory,
    [ValidateRange(1, 100)][int]$FaultRepetitions = 10,
    [ValidatePattern('^[A-Za-z0-9._-]+$')][string]$RustToolchain = 'stable'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'test-fixture.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $repo 'tools/broker-fault-harness.ps1')
$build = Assert-MoPlainPath $BuildDirectory
$stage = Join-Path $build 'stage'
$null = Assert-MoPreparedStage $stage
$receipt = Read-MoStageJson (Join-Path $stage 'evidence/build-receipt.json')
$payload = Join-Path $stage 'payload/Mo'
$dll = Join-Path $payload 'runtime/librime/rime.dll'
$shared = Join-Path $payload 'data/rime-ice'
foreach ($platform in @('x64', 'Win32')) {
    $probe = Join-Path $build "working/native/$platform/mo_tip_abi_probe.exe"
    if ((Get-FileHash -LiteralPath (Assert-MoPlainPath $probe)).Hash -ine $receipt['abi_probe_images'][$platform]) {
        throw 'Fresh stage ABI probe receipt mismatch.'
    }
}
Push-Location $repo
try {
    & cargo "+$RustToolchain" build --locked --offline -p mo-broker --bin mo-broker
    if ($LASTEXITCODE -ne 0) { throw 'Diagnostic harness Broker build failed.' }
    & cargo "+$RustToolchain" build --locked --offline -p mo-rime --example preparation_probe --example candidate_smoke --example resource_pack_smoke
    if ($LASTEXITCODE -ne 0) { throw 'Stage engine probes build failed.' }
} finally { Pop-Location }
$diagnosticBroker = Join-Path $repo 'target/debug/mo-broker.exe'
$fixture = Join-Path $repo ('build/mo-stage-runtime-墨-' + [Guid]::NewGuid().ToString('N'))
$null = Assert-MoNewBuildOutput $fixture $repo
New-Item -ItemType Directory -Path $fixture | Out-Null
function New-OwnedUser([string]$Name) {
    $user = Join-Path $fixture $Name
    New-Item -ItemType Directory -Path $user | Out-Null
    Copy-Item -LiteralPath (Join-Path $shared 'build') -Destination (Join-Path $user 'build') -Recurse
    return $user
}
try {
    $registrarMarker = Join-Path $fixture 'staged-machine-profile-transaction.marker'
    & (Join-Path $payload 'bin/mo-tip-registrar.exe') self-test-machine-transaction $registrarMarker
    if ($LASTEXITCODE -ne 0 -or (Test-Path -LiteralPath $registrarMarker)) {
        throw 'Staged registrar transaction marker self-test failed or left residue.'
    }
    # A shipping-layout TIP authenticates its sibling bin/mo-broker.exe by file
    # identity. Never weaken that check or replace the staged release Broker.
    # Copy identical TIP bytes into a disposable installed-like layout, paired
    # with a diagnostic Broker copy solely for controlled engine/fault tests.
    $testMo = Join-Path $fixture 'Mo'
    New-Item -ItemType Directory -Path (Join-Path $testMo 'bin') | Out-Null
    $broker = Join-Path $testMo 'bin/mo-broker.exe'
    Copy-Item -LiteralPath $diagnosticBroker -Destination $broker
    if ((Get-FileHash -LiteralPath $broker).Hash -ine (Get-FileHash -LiteralPath $diagnosticBroker).Hash) { throw 'Diagnostic fixture Broker copy mismatch.' }
    $goldenUser = Join-Path $fixture 'managed-prebuilt'
    New-Item -ItemType Directory -Path (Join-Path $goldenUser 'build') | Out-Null
    New-Item -ItemType File -Path (Join-Path $goldenUser 'mo-resource-pack-fixture') | Out-Null
    & (Join-Path $repo 'target/debug/examples/resource_pack_smoke.exe') $dll $shared $goldenUser (Get-Date -Format 'yyyy-MM-dd')
    if ($LASTEXITCODE -ne 0) { throw 'Staged prebuilt-only/Lua/English golden checks failed.' }
    # This directory must remain empty; remove only our known empty fixture dir.
    if (@(Get-ChildItem -LiteralPath (Join-Path $goldenUser 'build') -Force).Count) { throw 'Golden staging generated unexpected files.' }
    Remove-Item -LiteralPath (Join-Path $goldenUser 'build')
    $user = New-OwnedUser 'engine'
    New-Item -ItemType File -Path (Join-Path $user 'mo-preparation-fixture') | Out-Null
    & (Join-Path $repo 'target/debug/examples/preparation_probe.exe') $dll $shared $user success
    if ($LASTEXITCODE -ne 0) { throw 'Staged input-free/Emoji/exactly-once preparation checks failed.' }
    $candidateUser = New-OwnedUser 'candidates'
    & (Join-Path $repo 'target/debug/examples/candidate_smoke.exe') $dll $shared $candidateUser
    if ($LASTEXITCODE -ne 0) { throw 'Staged Actor paging/selection check failed.' }
    foreach ($platform in @('x64', 'Win32')) {
        $probe = Join-Path $build "working/native/$platform/mo_tip_abi_probe.exe"
        $architecture = if ($platform -eq 'x64') { 'x64' } else { 'x86' }
        $stagedTip = Join-Path $payload "tip/$architecture/mo-tip.dll"
        $tip = Join-Path $testMo "tip/$architecture/mo-tip.dll"
        New-Item -ItemType Directory -Path (Split-Path -Parent $tip) | Out-Null
        Copy-Item -LiteralPath $stagedTip -Destination $tip
        if ((Get-FileHash -LiteralPath $tip).Hash -ine (Get-FileHash -LiteralPath $stagedTip).Hash) { throw 'Staged TIP fixture copy mismatch.' }
        for ($trial = 1; $trial -le $FaultRepetitions; ++$trial) {
            Write-Host "$platform staged TIP/runtime/data crash-recovery $trial/$FaultRepetitions"
            $faultUser = New-OwnedUser ("fault-$platform-$trial")
            Invoke-MoBrokerFaultProbe $broker @('--rime-prepared', $dll, $shared, $faultUser) $probe $tip -RimeIce
        }
    }
    $null = Assert-MoPreparedStage $stage
    Write-Host "Actual staged TIP/runtime/locked-source data checks passed: $FaultRepetitions rounds per architecture, two real Broker exits per round. Diagnostic Broker only; no installed/registered system-route claim."
} finally {
    $resolved = Assert-MoPlainPath $fixture
    if (-not $resolved.StartsWith((Join-Path $repo 'build') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe runtime fixture cleanup target.' }
    Assert-MoOwnedFixtureTree $resolved
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
