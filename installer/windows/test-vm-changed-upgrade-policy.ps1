#Requires -Version 7.4
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'test-fixture.ps1')
. (Join-Path $PSScriptRoot 'changed-upgrade-policy.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $repo ('build/mo-changed-kit-policy-' + [Guid]::NewGuid().ToString('N'))
$null = Assert-MoNewBuildOutput $fixture $repo
New-Item -ItemType Directory -Path $fixture | Out-Null
$kitRoot = Join-Path $fixture 'kit'
New-Item -ItemType Directory -Path $kitRoot | Out-Null
$script:testCount = 0
function Write-Json([object]$Value, [string]$Path) {
    $Value | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $Path -Encoding utf8NoBOM
}
function Pass([string]$Label, [scriptblock]$Action) {
    & $Action
    $script:testCount++
    Write-Host "PASS $Label"
}
function Reject([string]$Label, [scriptblock]$Action, [string]$Pattern) {
    $rejected = $false
    try { & $Action | Out-Null } catch {
        if ($_.Exception.Message -notmatch $Pattern) { throw "Unexpected failure: $($_.Exception.Message)" }
        $rejected = $true
    }
    if (-not $rejected) { throw "Expected rejection: $Label" }
    $script:testCount++
    Write-Host "PASS $Label rejected"
}
function Refresh-Kit {
    $inventory = Get-MoStageInventory $kitRoot
    $inventory.Remove('vm-changed-upgrade-kit.json')
    $manifest.files = $inventory
    Write-Json $manifest (Join-Path $kitRoot 'vm-changed-upgrade-kit.json')
}
try {
    $fileNames = @(
        'mo-setup-base-unsigned.exe','mo-setup-upgrade-unsigned.exe',
        'base-stage.json','upgrade-stage.json','base-linked.json','upgrade-linked.json',
        'linked-upgrade-pair-evidence.json','mo-tip-registrar.exe','vm-test-policy.ps1',
        'changed-upgrade-policy.ps1','initialize-disposable-vm.ps1','run-vm-changed-payload-upgrade.ps1'
    )
    foreach ($name in $fileNames) {
        "synthetic-$name" | Set-Content (Join-Path $kitRoot $name) -Encoding utf8NoBOM
    }
    $registrarPath = Join-Path $kitRoot 'mo-tip-registrar.exe'
    foreach ($side in @('base','upgrade')) {
        $tree = Join-Path $fixture "$side-tree"
        New-Item -ItemType Directory -Path (Join-Path $tree 'bin'), (Join-Path $tree 'data') | Out-Null
        Copy-Item $registrarPath (Join-Path $tree 'bin/mo-tip-registrar.exe')
        for ($i = 0; $i -lt 131; $i++) {
            "$side-$i" | Set-Content (Join-Path $tree ("data/f{0:D3}.bin" -f $i)) -Encoding utf8NoBOM
        }
        $files = [ordered]@{}
        foreach ($entry in (Get-MoStageInventory $tree).GetEnumerator()) {
            $files['payload/Mo/' + $entry.Key] = $entry.Value
        }
        Write-Json ([ordered]@{
            format=1;kind='mo-windows-development-stage';development_only=$true
            redistributable=$false;installable=$false;payload_root='payload/Mo';files=$files
        }) (Join-Path $kitRoot "$side-stage.json")
    }
    $base = [ordered]@{
        format=3;development_only=$true;install_executed=$false;build_flavor='DevelopmentTest'
        fault_injection_included=$true;wix_version='4.0.6+73c89738';product_version='0.0.11.0'
        msi_product_code='{11111111-1111-4111-8111-111111111111}'
        msi_upgrade_code='{8245E1F5-8BC4-4D41-BA9C-65F30CEBCB32}'
        bundle_id='{33333333-3333-4333-8333-333333333333}'
        bundle_upgrade_code='{E08C0321-3F73-4D39-BC28-E7DF02C5134E}'
        known_link_warning='synthetic';msi_ice_validated=$true
        stage_manifest_sha256=(Get-FileHash (Join-Path $kitRoot 'base-stage.json')).Hash
        msi_sha256=('A'*64);bundle_sha256=(Get-FileHash (Join-Path $kitRoot 'mo-setup-base-unsigned.exe')).Hash
    }
    $upgrade = [ordered]@{}
    foreach ($key in $base.Keys) { $upgrade[$key]=$base[$key] }
    $upgrade.product_version='0.0.12.0'
    $upgrade.msi_product_code='{22222222-2222-4222-8222-222222222222}'
    $upgrade.bundle_id='{44444444-4444-4444-8444-444444444444}'
    $upgrade.stage_manifest_sha256=(Get-FileHash (Join-Path $kitRoot 'upgrade-stage.json')).Hash
    $upgrade.bundle_sha256=(Get-FileHash (Join-Path $kitRoot 'mo-setup-upgrade-unsigned.exe')).Hash
    $upgrade.msi_sha256=('B'*64)
    Write-Json $base (Join-Path $kitRoot 'base-linked.json')
    Write-Json $upgrade (Join-Path $kitRoot 'upgrade-linked.json')
    & (Join-Path $PSScriptRoot 'verify-linked-upgrade-pair.ps1') -PayloadMode Changed -BaseEvidencePath (
        Join-Path $kitRoot 'base-linked.json') -UpgradeEvidencePath (
        Join-Path $kitRoot 'upgrade-linked.json') -OutputDirectory (Join-Path $fixture 'pair')
    Copy-Item (Join-Path $fixture 'pair/linked-upgrade-pair-evidence.json') (
        Join-Path $kitRoot 'linked-upgrade-pair-evidence.json') -Force
    $pair=Get-Content (Join-Path $kitRoot 'linked-upgrade-pair-evidence.json') -Raw | ConvertFrom-Json
    $manifest=[ordered]@{
        format=1;kind='mo-installer-vm-changed-upgrade-kit';development_only=$true
        redistributable=$false;install_execution_authorized=$false;wix_version='4.0.6+73c89738'
        base_version=$pair.base_version;upgrade_version=$pair.upgrade_version
        base_product_code=$pair.base_product_code;upgrade_product_code=$pair.upgrade_product_code
        base_bundle_sha256=$pair.base_bundle_sha256;upgrade_bundle_sha256=$pair.upgrade_bundle_sha256
        base_stage_manifest_sha256=$pair.base_stage_manifest_sha256
        upgrade_stage_manifest_sha256=$pair.upgrade_stage_manifest_sha256
        pair_sha256=(Get-FileHash (Join-Path $kitRoot 'linked-upgrade-pair-evidence.json')).Hash;files=$null
    }
    Refresh-Kit
    Pass 'exact changed upgrade kit' { $null=Assert-MoVmChangedUpgradeKit $kitRoot }
    $baseContract=Get-MoVmPayloadContract (Join-Path $kitRoot 'base-stage.json')
    $upgradeContract=Get-MoVmPayloadContract (Join-Path $kitRoot 'upgrade-stage.json')
    Pass 'separate installed trees match own manifests' {
        Assert-MoVmInstalledPayload (Join-Path $fixture 'base-tree') $baseContract
        Assert-MoVmInstalledPayload (Join-Path $fixture 'upgrade-tree') $upgradeContract
    }
    Reject 'base tree cannot pass upgrade manifest' {
        Assert-MoVmInstalledPayload (Join-Path $fixture 'base-tree') $upgradeContract
    } 'hash/size'
    Reject 'upgrade tree cannot pass base manifest' {
        Assert-MoVmInstalledPayload (Join-Path $fixture 'upgrade-tree') $baseContract
    } 'hash/size'
    $manifest.install_execution_authorized=$true;Refresh-Kit
    Reject 'kit cannot authorize execution' { Assert-MoVmChangedUpgradeKit $kitRoot } 'unauthorized development'
    $manifest.install_execution_authorized=$false;Refresh-Kit
    $manifest.base_stage_manifest_sha256=$manifest.upgrade_stage_manifest_sha256;Refresh-Kit
    Reject 'kit stage shortcut' { Assert-MoVmChangedUpgradeKit $kitRoot } 'kit/pair mismatch'
    $manifest.base_stage_manifest_sha256=$pair.base_stage_manifest_sha256;Refresh-Kit
    $saved=[IO.File]::ReadAllBytes($registrarPath)
    'foreign-registrar'|Set-Content $registrarPath -Encoding utf8NoBOM
    Reject 'raw registrar tampering' { Assert-MoVmChangedUpgradeKit $kitRoot } 'hash/size mismatch'
    Refresh-Kit
    Reject 'relisted registrar must match payload' { Assert-MoVmChangedUpgradeKit $kitRoot } 'registrar does not match'
    [IO.File]::WriteAllBytes($registrarPath,$saved);Refresh-Kit
    $extra=Join-Path $kitRoot 'extra.txt'
    'extra'|Set-Content $extra
    Reject 'extra kit file' { Assert-MoVmChangedUpgradeKit $kitRoot } 'inventory name'
    Remove-Item -LiteralPath $extra
    $extra=Join-Path $kitRoot 'hidden.txt'
    'hidden'|Set-Content $extra
    (Get-Item $extra).Attributes=[IO.FileAttributes]::Hidden
    Reject 'hidden kit file' { Assert-MoVmChangedUpgradeKit $kitRoot } 'inventory name'
    Remove-Item -LiteralPath $extra -Force
    $extra=Join-Path $kitRoot 'unexpected'
    New-Item -ItemType Directory $extra | Out-Null
    Reject 'kit directory' { Assert-MoVmChangedUpgradeKit $kitRoot } 'must be flat'
    Remove-Item -LiteralPath $extra
    $linkedPath=Join-Path $kitRoot 'upgrade-linked.json'
    $savedLinked=[IO.File]::ReadAllBytes($linkedPath)
    $savedPair=[IO.File]::ReadAllBytes((Join-Path $kitRoot 'linked-upgrade-pair-evidence.json'))
    $upgrade.msi_product_code=$base.msi_product_code
    Write-Json $upgrade $linkedPath
    $pair.upgrade_linked_evidence_sha256=(Get-FileHash $linkedPath).Hash
    Write-Json $pair (Join-Path $kitRoot 'linked-upgrade-pair-evidence.json')
    $manifest.pair_sha256=(Get-FileHash (Join-Path $kitRoot 'linked-upgrade-pair-evidence.json')).Hash
    Refresh-Kit
    Reject 'rebound linked receipt still matches product contract' {
        Assert-MoVmChangedUpgradeKit $kitRoot
    } 'linked receipt mismatch'
    [IO.File]::WriteAllBytes($linkedPath,$savedLinked)
    [IO.File]::WriteAllBytes((Join-Path $kitRoot 'linked-upgrade-pair-evidence.json'),$savedPair)
    $manifest.pair_sha256=(Get-FileHash (Join-Path $kitRoot 'linked-upgrade-pair-evidence.json')).Hash
    Refresh-Kit
    foreach($case in @(
        @('same stage pair','base_stage_manifest_sha256',$manifest.upgrade_stage_manifest_sha256,'must differ'),
        @('same package pair','base_bundle_sha256',$manifest.upgrade_bundle_sha256,'must differ'),
        @('same product pair','base_product_code',$manifest.upgrade_product_code,'must differ'),
        @('revision only pair','upgrade_version','0.0.11.1','first three'),
        @('wrong kind pair','kind','mo-linked-upgrade-pair','Invalid changed'),
        @('string boolean pair','development_only','true','Invalid changed'),
        @('foreign Mo family','msi_upgrade_code','{55555555-5555-4555-8555-555555555555}','unexpected Mo'),
        @('malformed pair GUID','upgrade_bundle_id','unknown','Invalid changed-payload GUID'),
        @('invalid linked hash','base_linked_evidence_sha256','unknown','Invalid changed-payload hash')
    )){
        $badPair=Get-Content (Join-Path $kitRoot 'linked-upgrade-pair-evidence.json') -Raw | ConvertFrom-Json
        $badPair.($case[1])=$case[2]
        Reject $case[0] { Assert-MoChangedUpgradePair $badPair } $case[3]
    }
    foreach($path in @('relative.mo','\\\\server\\share\\settings.mo','C:\\settings.mo:stream')){
        Reject 'nonlocal or alternate-stream settings path' { Get-MoVmSettingsFingerprint $path } 'absolute local DOS'
    }
    $settings=Join-Path $fixture 'settings-v1.mo'
    $missing=Get-MoVmSettingsFingerprint $settings
    Pass 'absent settings preserved' { Assert-MoVmSettingsPreserved $missing (Get-MoVmSettingsFingerprint $settings) }
    'synthetic-settings'|Set-Content $settings -Encoding utf8NoBOM
    $stored=Get-MoVmSettingsFingerprint $settings
    Reject 'new settings file counts as mutation' {
        Assert-MoVmSettingsPreserved $missing $stored
    } 'changed the user settings'
    Pass 'stored settings preserved' { Assert-MoVmSettingsPreserved $stored (Get-MoVmSettingsFingerprint $settings) }
    'changed-settings'|Set-Content $settings -Encoding utf8NoBOM
    Reject 'settings bytes changed' {
        Assert-MoVmSettingsPreserved $stored (Get-MoVmSettingsFingerprint $settings)
    } 'changed the user settings'
    Remove-Item -LiteralPath $settings
    Reject 'settings deletion detected' {
        Assert-MoVmSettingsPreserved $stored (Get-MoVmSettingsFingerprint $settings)
    } 'changed the user settings'
    $junction=Join-Path $fixture 'settings-junction'
    New-Item -ItemType Junction -Path $junction -Target $kitRoot | Out-Null
    Reject 'settings path through junction' {
        Get-MoVmSettingsFingerprint (Join-Path $junction 'absent.mo')
    } 'reparse'
    Remove-Item -LiteralPath $junction -Force
    $driver=Join-Path $PSScriptRoot 'run-vm-changed-payload-upgrade.ps1'
    foreach($mode in @('neither','vm-only','execution-only')){
        $driverParameters=@{EvidenceDirectory=(Join-Path $fixture 'never-created')}
        if($mode -eq 'vm-only'){$driverParameters.DisposableVm=$true}
        if($mode -eq 'execution-only'){$driverParameters.AllowInstallerExecution=$true}
        Reject "driver refuses $mode" { & $driver @driverParameters } 'Pass both switches'
    }
    Reject 'driver refuses missing sentinel before installer' {
        & $driver -DisposableVm -AllowInstallerExecution -SentinelPath (
            Join-Path $fixture 'absent-sentinel.json') -EvidenceDirectory (Join-Path $fixture 'never-created')
    } 'sentinel is missing'
    if(Test-Path (Join-Path $fixture 'never-created')){throw 'Driver wrote evidence before preconditions'}
    Write-Host "Changed upgrade kit policy tests passed: $script:testCount. Synthetic files only; no installers executed."
} finally {
    $resolved=Assert-MoPlainPath $fixture
    if(-not $resolved.StartsWith((Join-Path $repo 'build')+'\',[StringComparison]::OrdinalIgnoreCase)){
        throw 'Unsafe changed kit fixture cleanup target.'
    }
    Assert-MoOwnedFixtureTree $resolved
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
