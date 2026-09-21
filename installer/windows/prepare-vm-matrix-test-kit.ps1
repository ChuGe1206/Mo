#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$BaseBundlePath,
    [Parameter(Mandatory = $true)][string]$UpgradeBundlePath,
    [Parameter(Mandatory = $true)][string]$ProbeRegistrarPath,
    [Parameter(Mandatory = $true)][string]$StageDirectory,
    [Parameter(Mandatory = $true)][string]$UpgradePairEvidencePath,
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$output = Assert-MoNewBuildOutput $OutputDirectory $repo
$baseBundle = Assert-MoPlainPath $BaseBundlePath
$upgradeBundle = Assert-MoPlainPath $UpgradeBundlePath
$registrar = Assert-MoPlainPath $ProbeRegistrarPath
$stage = Assert-MoPlainPath $StageDirectory
$pairPath = Assert-MoPlainPath $UpgradePairEvidencePath
foreach ($file in @($baseBundle, $upgradeBundle, $registrar, $pairPath)) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
        throw "VM matrix test kit input is missing: $file"
    }
}
$null = Assert-MoPreparedStage $stage
$stageManifest = Join-Path $stage 'mo-stage.json'
$stageRegistrar = Join-Path $stage 'payload\Mo\bin\mo-tip-registrar.exe'
$pair = Read-MoStageJson $pairPath
$required = @(
    'format', 'kind', 'development_only', 'install_executed', 'build_flavor',
    'fault_injection_included', 'base_version',
    'upgrade_version', 'stage_manifest_sha256', 'msi_upgrade_code',
    'bundle_upgrade_code', 'base_product_code', 'upgrade_product_code',
    'base_bundle_id', 'upgrade_bundle_id', 'base_bundle_sha256',
    'upgrade_bundle_sha256'
)
if ($pair.Count -ne $required.Count -or
    @($required | Where-Object { -not $pair.Contains($_) }).Count -or
    $pair['format'] -ne 2 -or $pair['kind'] -cne 'mo-linked-upgrade-pair' -or
    $pair['development_only'] -ne $true -or $pair['install_executed'] -ne $false -or
    $pair['build_flavor'] -cne 'DevelopmentTest' -or
    $pair['fault_injection_included'] -ne $true -or
    [version]$pair['base_version'] -ge [version]$pair['upgrade_version']) {
    throw 'Invalid linked upgrade-pair evidence.'
}
if ((Get-FileHash -LiteralPath $baseBundle -Algorithm SHA256).Hash -cne $pair['base_bundle_sha256'] -or
    (Get-FileHash -LiteralPath $upgradeBundle -Algorithm SHA256).Hash -cne $pair['upgrade_bundle_sha256'] -or
    (Get-FileHash -LiteralPath $stageManifest -Algorithm SHA256).Hash -cne $pair['stage_manifest_sha256'] -or
    (Get-FileHash -LiteralPath $registrar -Algorithm SHA256).Hash -cne
        (Get-FileHash -LiteralPath $stageRegistrar -Algorithm SHA256).Hash) {
    throw 'VM matrix test kit input hash does not match linked/staged evidence.'
}

New-Item -ItemType Directory -Path $output | Out-Null
$copies = [ordered]@{
    'mo-setup-base-unsigned.exe' = $baseBundle
    'mo-setup-upgrade-unsigned.exe' = $upgradeBundle
    'mo-tip-registrar.exe' = $registrar
    'mo-stage.json' = $stageManifest
    'vm-test-policy.ps1' = (Join-Path $PSScriptRoot 'vm-test-policy.ps1')
    'initialize-disposable-vm.ps1' = (Join-Path $PSScriptRoot 'initialize-disposable-vm.ps1')
    'run-vm-installer-matrix.ps1' = (Join-Path $PSScriptRoot 'run-vm-installer-matrix.ps1')
}
foreach ($name in $copies.Keys) {
    Copy-Item -LiteralPath $copies[$name] -Destination (Join-Path $output $name)
}
$manifest = [ordered]@{
    format = 1
    kind = 'mo-installer-vm-matrix-test-kit'
    development_only = $true
    redistributable = $false
    install_execution_authorized = $false
    wix_version = '4.0.6+73c89738'
    base_version = $pair['base_version']
    upgrade_version = $pair['upgrade_version']
    base_bundle_sha256 = $pair['base_bundle_sha256']
    upgrade_bundle_sha256 = $pair['upgrade_bundle_sha256']
    stage_manifest_sha256 = $pair['stage_manifest_sha256']
    msi_upgrade_code = $pair['msi_upgrade_code']
    bundle_upgrade_code = $pair['bundle_upgrade_code']
    base_product_code = $pair['base_product_code']
    upgrade_product_code = $pair['upgrade_product_code']
    files = Get-MoStageInventory $output
}
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (
    Join-Path $output 'vm-matrix-test-kit.json') -Encoding utf8NoBOM
Assert-MoInventory $output $manifest.files @('vm-matrix-test-kit.json')
Write-Host "Prepared hash-locked disposable-VM rollback/upgrade matrix kit: $output"
Write-Warning 'The kit does not authorize execution. Initialize a throwaway VM explicitly before running its matrix driver.'
