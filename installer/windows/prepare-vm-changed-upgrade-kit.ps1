#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$BaseBundlePath,
    [Parameter(Mandatory = $true)][string]$UpgradeBundlePath,
    [Parameter(Mandatory = $true)][string]$BaseStageDirectory,
    [Parameter(Mandatory = $true)][string]$UpgradeStageDirectory,
    [Parameter(Mandatory = $true)][string]$BaseLinkedEvidencePath,
    [Parameter(Mandatory = $true)][string]$UpgradeLinkedEvidencePath,
    [Parameter(Mandatory = $true)][string]$UpgradePairEvidencePath,
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'changed-upgrade-policy.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$output = Assert-MoNewBuildOutput $OutputDirectory $repo
$baseStage = Assert-MoPlainPath $BaseStageDirectory
$upgradeStage = Assert-MoPlainPath $UpgradeStageDirectory
$pairPath = Assert-MoPlainPath $UpgradePairEvidencePath
$pair = Get-Content -LiteralPath $pairPath -Raw | ConvertFrom-Json
Assert-MoChangedUpgradePair $pair
$copies = [ordered]@{
    'mo-setup-base-unsigned.exe' = (Assert-MoPlainPath $BaseBundlePath)
    'mo-setup-upgrade-unsigned.exe' = (Assert-MoPlainPath $UpgradeBundlePath)
    'base-stage.json' = (Join-Path $baseStage 'mo-stage.json')
    'upgrade-stage.json' = (Join-Path $upgradeStage 'mo-stage.json')
    'base-linked.json' = (Assert-MoPlainPath $BaseLinkedEvidencePath)
    'upgrade-linked.json' = (Assert-MoPlainPath $UpgradeLinkedEvidencePath)
    'linked-upgrade-pair-evidence.json' = $pairPath
    'mo-tip-registrar.exe' = (Join-Path $upgradeStage 'payload/Mo/bin/mo-tip-registrar.exe')
    'vm-test-policy.ps1' = (Join-Path $PSScriptRoot 'vm-test-policy.ps1')
    'changed-upgrade-policy.ps1' = (Join-Path $PSScriptRoot 'changed-upgrade-policy.ps1')
    'initialize-disposable-vm.ps1' = (Join-Path $PSScriptRoot 'initialize-disposable-vm.ps1')
    'run-vm-changed-payload-upgrade.ps1' = (Join-Path $PSScriptRoot 'run-vm-changed-payload-upgrade.ps1')
}
foreach ($file in $copies.Values) {
    $null = Assert-MoPlainPath $file
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw 'Missing changed upgrade kit input.' }
}
# A historical base ABI is evidence for the old install, not a new-build runtime.
$null = Get-MoVmPayloadContract $copies['base-stage.json']
$baseMetadata = Read-MoStageJson $copies['base-stage.json']
Assert-MoInventory $baseStage $baseMetadata['files'] @('mo-stage.json')
$null = Assert-MoPreparedStage $upgradeStage
$bindings = @{
    'mo-setup-base-unsigned.exe' = $pair.base_bundle_sha256
    'mo-setup-upgrade-unsigned.exe' = $pair.upgrade_bundle_sha256
    'base-stage.json' = $pair.base_stage_manifest_sha256
    'upgrade-stage.json' = $pair.upgrade_stage_manifest_sha256
    'base-linked.json' = $pair.base_linked_evidence_sha256
    'upgrade-linked.json' = $pair.upgrade_linked_evidence_sha256
}
foreach ($name in $bindings.Keys) {
    if ((Get-FileHash -LiteralPath $copies[$name]).Hash -cne $bindings[$name]) {
        throw "Changed upgrade input hash mismatch: $name"
    }
}
New-Item -ItemType Directory -Path $output | Out-Null
foreach ($name in $copies.Keys) { Copy-Item -LiteralPath $copies[$name] -Destination (Join-Path $output $name) }
$manifest = [ordered]@{
    format = 1; kind = 'mo-installer-vm-changed-upgrade-kit'
    development_only = $true; redistributable = $false; install_execution_authorized = $false
    wix_version = '4.0.6+73c89738'
    base_version = $pair.base_version; upgrade_version = $pair.upgrade_version
    base_product_code = $pair.base_product_code; upgrade_product_code = $pair.upgrade_product_code
    base_bundle_sha256 = $pair.base_bundle_sha256; upgrade_bundle_sha256 = $pair.upgrade_bundle_sha256
    base_stage_manifest_sha256 = $pair.base_stage_manifest_sha256
    upgrade_stage_manifest_sha256 = $pair.upgrade_stage_manifest_sha256
    pair_sha256 = (Get-FileHash -LiteralPath $pairPath).Hash
    files = Get-MoStageInventory $output
}
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (
    Join-Path $output 'vm-changed-upgrade-kit.json') -Encoding utf8NoBOM
$null = Assert-MoVmChangedUpgradeKit $output
Write-Host "Prepared hash-locked changed-payload upgrade kit: $output"
Write-Warning 'No installer was executed. The guest driver requires a matching disposable VM sentinel and both execution switches.'
