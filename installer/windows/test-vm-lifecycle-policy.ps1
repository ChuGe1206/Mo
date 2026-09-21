#Requires -Version 7.4
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'test-fixture.ps1')
. (Join-Path $PSScriptRoot 'vm-test-policy.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $repo ('build/mo-vm-policy-' + [Guid]::NewGuid().ToString('N'))
$null = Assert-MoNewBuildOutput $fixture $repo
New-Item -ItemType Directory -Path $fixture | Out-Null
$script:testCount = 0

function Pass([string]$Label, [scriptblock]$Action) {
    & $Action
    $script:testCount++
    Write-Host "PASS $Label"
}

function Reject([string]$Label, [scriptblock]$Action, [string]$Pattern) {
    $rejected = $false
    try { & $Action | Out-Null } catch {
        if ($_.Exception.Message -notmatch $Pattern) {
            throw "Unexpected failure for ${Label}: $($_.Exception.Message)"
        }
        $rejected = $true
    }
    if (-not $rejected) { throw "Expected rejection: $Label" }
    $script:testCount++
    Write-Host "PASS $Label rejected"
}

function New-State([string]$Phase) {
    $values = switch ($Phase) {
        Clean { @('missing','missing','false','false','false','missing','missing') }
        Installed { @('missing','missing','true','true','false','v1','mo-user-finalizer-install-v1-disabled') }
        Repaired { @('missing','missing','true','true','false','v1','mo-user-finalizer-repair-v1-enabled') }
        Uninstalled { @('missing','missing','false','false','false','missing','mo-user-finalizer-remove-v1-enabled') }
    }
    $names = @('com.x64','com.x86','profile.registered','profile.enabled','profile.active','user.finalizer','user.finalizer.transaction')
    $lines = for ($index = 0; $index -lt $names.Count; $index++) { "$($names[$index])=$($values[$index])" }
    return ConvertFrom-MoVmRegistrarStatus $lines
}

try {
    foreach ($phase in @('Clean', 'Installed', 'Repaired', 'Uninstalled')) {
        Pass "exact $phase lifecycle state" { Assert-MoVmLifecycleState (New-State $phase) $phase }
    }
    foreach ($bad in @(
        @('duplicate status', @('com.x64=missing','com.x64=missing'), 'duplicate'),
        @('unknown boolean', @('com.x64=missing','com.x86=missing','profile.registered=maybe','profile.enabled=false','profile.active=false','user.finalizer=missing','user.finalizer.transaction=missing'), 'boolean'),
        @('unknown transaction', @('com.x64=missing','com.x86=missing','profile.registered=false','profile.enabled=false','profile.active=false','user.finalizer=missing','user.finalizer.transaction=other'), 'transaction'),
        @('extra status', @('com.x64=missing','com.x86=missing','profile.registered=false','profile.enabled=false','profile.active=false','user.finalizer=missing','user.finalizer.transaction=missing','extra=value'), 'count')
    )) {
        Reject $bad[0] { ConvertFrom-MoVmRegistrarStatus $bad[1] } $bad[2]
    }
    foreach ($phase in @('Clean', 'Installed', 'Repaired', 'Uninstalled')) {
        $changed = New-State $phase
        $changed['profile.active'] = 'true'
        Reject "$phase default-input mutation" { Assert-MoVmLifecycleState $changed $phase } 'profile.active'
    }

    $install = Join-Path $fixture 'installed'
    New-Item -ItemType Directory -Path (Join-Path $install 'bin'),(Join-Path $install 'data') | Out-Null
    'broker' | Set-Content -LiteralPath (Join-Path $install 'bin/mo-broker.exe') -Encoding utf8NoBOM
    'registrar' | Set-Content -LiteralPath (Join-Path $install 'bin/mo-tip-registrar.exe') -Encoding utf8NoBOM
    'schema' | Set-Content -LiteralPath (Join-Path $install 'data/default.yaml') -Encoding utf8NoBOM
    $entries = [ordered]@{}
    foreach ($relative in @('bin/mo-broker.exe','bin/mo-tip-registrar.exe','data/default.yaml')) {
        $file = Join-Path $install ($relative.Replace('/', '\'))
        $entries["payload/Mo/$relative"] = [ordered]@{
            size = (Get-Item -LiteralPath $file).Length
            sha256 = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash
        }
    }
    $manifestPath = Join-Path $fixture 'mo-stage.json'
    [ordered]@{
        format = 1; kind = 'mo-windows-development-stage'; development_only = $true
        redistributable = $false; installable = $false; payload_root = 'payload/Mo'
        rime_ice_commit = ('a' * 40); rime_ice_archive_sha256 = ('A' * 64); files = $entries
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $manifestPath -Encoding utf8NoBOM
    $contract = Get-MoVmPayloadContract $manifestPath 3
    Pass 'exact installed payload' { Assert-MoVmInstalledPayload $install $contract }
    'changed' | Set-Content -LiteralPath (Join-Path $install 'data/default.yaml') -Encoding utf8NoBOM
    Reject 'tampered installed payload' { Assert-MoVmInstalledPayload $install $contract } 'hash/size'
    'schema' | Set-Content -LiteralPath (Join-Path $install 'data/default.yaml') -Encoding utf8NoBOM
    $caseContract = [Collections.Specialized.OrderedDictionary]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($key in $contract.Keys) {
        $caseKey = if ($key -ceq 'data/default.yaml') { 'DATA/default.yaml' } else { $key }
        $caseContract.Add($caseKey, $contract[$key])
    }
    Reject 'installed payload path case mismatch' {
        Assert-MoVmInstalledPayload $install $caseContract
    } 'path case mismatch'
    'extra' | Set-Content -LiteralPath (Join-Path $install 'extra.txt') -Encoding utf8NoBOM
    Reject 'extra installed payload' { Assert-MoVmInstalledPayload $install $contract } 'file count'
    Remove-Item -LiteralPath (Join-Path $install 'extra.txt')
    Remove-Item -LiteralPath (Join-Path $install 'bin/mo-broker.exe')
    Reject 'missing installed payload' { Assert-MoVmInstalledPayload $install $contract } 'file count|missing'

    $badManifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json -AsHashtable
    $badManifest.files['payload/Mo/../escape'] = $badManifest.files['payload/Mo/data/default.yaml']
    $badManifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $manifestPath -Encoding utf8NoBOM
    Reject 'payload traversal contract' { Get-MoVmPayloadContract $manifestPath 4 } 'Unsafe payload'

    $kitRoot = Join-Path $fixture 'kit'
    New-Item -ItemType Directory -Path $kitRoot | Out-Null
    $kitNames = @(
        'initialize-disposable-vm.ps1', 'mo-stage.json', 'mo-tip-registrar.exe',
        'mo-setup-development-unsigned.exe', 'run-vm-installer-lifecycle.ps1',
        'vm-test-policy.ps1'
    )
    foreach ($name in $kitNames) { $name | Set-Content -LiteralPath (Join-Path $kitRoot $name) -Encoding utf8NoBOM }
    $kitManifestPath = Join-Path $kitRoot 'vm-test-kit.json'
    $kitManifest = [ordered]@{
        format = 1; kind = 'mo-installer-vm-test-kit'; development_only = $true
        redistributable = $false; install_execution_authorized = $false
        wix_version = '4.0.6+73c89738'
        bundle_sha256 = (Get-FileHash -LiteralPath (Join-Path $kitRoot 'mo-setup-development-unsigned.exe')).Hash
        stage_manifest_sha256 = (Get-FileHash -LiteralPath (Join-Path $kitRoot 'mo-stage.json')).Hash
        files = Get-MoStageInventory $kitRoot
    }
    $kitManifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $kitManifestPath -Encoding utf8NoBOM
    Pass 'exact VM test kit inventory' { $null = Assert-MoVmTestKit $kitRoot }
    $policyPath = Join-Path $kitRoot 'vm-test-policy.ps1'
    $policyBytes = [IO.File]::ReadAllBytes($policyPath)
    'tampered' | Set-Content -LiteralPath $policyPath -Encoding utf8NoBOM
    Reject 'tampered VM test kit' { Assert-MoVmTestKit $kitRoot } 'inventory mismatch'
    [IO.File]::WriteAllBytes($policyPath, $policyBytes)
    'extra' | Set-Content -LiteralPath (Join-Path $kitRoot 'extra.txt') -Encoding utf8NoBOM
    Reject 'extra VM test kit file' { Assert-MoVmTestKit $kitRoot } 'unexpected or missing'
    Remove-Item -LiteralPath (Join-Path $kitRoot 'extra.txt')
    New-Item -ItemType Directory -Path (Join-Path $kitRoot 'unexpected') | Out-Null
    Reject 'extra VM test kit directory' { Assert-MoVmTestKit $kitRoot } 'must not contain directories'
    Remove-Item -LiteralPath (Join-Path $kitRoot 'unexpected')

    $matrixRoot = Join-Path $fixture 'matrix-kit'
    New-Item -ItemType Directory -Path $matrixRoot | Out-Null
    $matrixNames = @(
        'initialize-disposable-vm.ps1', 'mo-stage.json', 'mo-tip-registrar.exe',
        'mo-setup-base-unsigned.exe', 'mo-setup-upgrade-unsigned.exe',
        'run-vm-installer-matrix.ps1', 'vm-test-policy.ps1'
    )
    foreach ($name in $matrixNames) {
        "matrix-$name" | Set-Content -LiteralPath (Join-Path $matrixRoot $name) -Encoding utf8NoBOM
    }
    $matrixManifestPath = Join-Path $matrixRoot 'vm-matrix-test-kit.json'
    $matrixManifest = [ordered]@{
        format = 1; kind = 'mo-installer-vm-matrix-test-kit'; development_only = $true
        redistributable = $false; install_execution_authorized = $false
        wix_version = '4.0.6+73c89738'; base_version = '0.0.1.0'; upgrade_version = '0.0.2.0'
        base_bundle_sha256 = (Get-FileHash -LiteralPath (Join-Path $matrixRoot 'mo-setup-base-unsigned.exe')).Hash
        upgrade_bundle_sha256 = (Get-FileHash -LiteralPath (Join-Path $matrixRoot 'mo-setup-upgrade-unsigned.exe')).Hash
        stage_manifest_sha256 = (Get-FileHash -LiteralPath (Join-Path $matrixRoot 'mo-stage.json')).Hash
        msi_upgrade_code = '{8245E1F5-8BC4-4D41-BA9C-65F30CEBCB32}'
        bundle_upgrade_code = '{E08C0321-3F73-4D39-BC28-E7DF02C5134E}'
        base_product_code = '{11111111-1111-4111-8111-111111111111}'
        upgrade_product_code = '{22222222-2222-4222-8222-222222222222}'
        files = Get-MoStageInventory $matrixRoot
    }
    $matrixManifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $matrixManifestPath -Encoding utf8NoBOM
    Pass 'exact VM matrix test kit inventory' { $null = Assert-MoVmMatrixTestKit $matrixRoot }
    $matrixManifest.upgrade_product_code = $matrixManifest.base_product_code
    $matrixManifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $matrixManifestPath -Encoding utf8NoBOM
    Reject 'duplicate VM matrix product code' {
        Assert-MoVmMatrixTestKit $matrixRoot
    } 'product codes must differ'
    $matrixManifest.upgrade_product_code = '{22222222-2222-4222-8222-222222222222}'
    $matrixManifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $matrixManifestPath -Encoding utf8NoBOM
    $matrixPolicy = Join-Path $matrixRoot 'vm-test-policy.ps1'
    $matrixPolicyBytes = [IO.File]::ReadAllBytes($matrixPolicy)
    'tampered' | Set-Content -LiteralPath $matrixPolicy -Encoding utf8NoBOM
    Reject 'tampered VM matrix test kit' {
        Assert-MoVmMatrixTestKit $matrixRoot
    } 'inventory mismatch'
    [IO.File]::WriteAllBytes($matrixPolicy, $matrixPolicyBytes)
    Write-Host "VM lifecycle policy tests passed: $script:testCount. No installer, registration, elevation or input-state mutation."
} finally {
    $resolved = Assert-MoPlainPath $fixture
    Assert-MoOwnedFixtureTree $resolved
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
