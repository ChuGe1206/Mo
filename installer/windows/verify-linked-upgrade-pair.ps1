#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$BaseEvidencePath,
    [Parameter(Mandatory = $true)][string]$UpgradeEvidencePath,
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$basePath = Assert-MoPlainPath $BaseEvidencePath
$upgradePath = Assert-MoPlainPath $UpgradeEvidencePath
$output = Assert-MoNewBuildOutput $OutputDirectory $repo
foreach ($path in @($basePath, $upgradePath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Linked upgrade evidence is missing: $path"
    }
}

function Read-LinkedEvidence([string]$Path) {
    $item = Read-MoStageJson $Path
    $required = @(
        'format', 'development_only', 'install_executed',
        'fault_injection_authoring_verified', 'wix_version', 'product_version',
        'msi_product_code', 'msi_upgrade_code', 'bundle_id', 'bundle_upgrade_code',
        'known_link_warning', 'msi_ice_validated', 'stage_manifest_sha256',
        'msi_sha256', 'bundle_sha256'
    )
    if ($item.Count -ne $required.Count -or
        @($required | Where-Object { -not $item.Contains($_) }).Count -or
        $item['format'] -ne 2 -or $item['development_only'] -ne $true -or
        $item['install_executed'] -ne $false -or
        $item['fault_injection_authoring_verified'] -ne $true -or
        $item['wix_version'] -cne '4.0.6+73c89738') {
        throw "Invalid linked installer evidence: $Path"
    }
    foreach ($name in @('msi_product_code', 'msi_upgrade_code', 'bundle_id', 'bundle_upgrade_code')) {
        if ($item[$name] -cnotmatch '^\{[A-F0-9]{8}-[A-F0-9]{4}-[A-F0-9]{4}-[A-F0-9]{4}-[A-F0-9]{12}\}$') {
            throw "Invalid GUID in linked installer evidence: $name"
        }
    }
    foreach ($name in @('stage_manifest_sha256', 'msi_sha256', 'bundle_sha256')) {
        if ($item[$name] -cnotmatch '^[A-F0-9]{64}$') {
            throw "Invalid hash in linked installer evidence: $name"
        }
    }
    return $item
}

$base = Read-LinkedEvidence $basePath
$upgrade = Read-LinkedEvidence $upgradePath
$baseVersion = [version]$base['product_version']
$upgradeVersion = [version]$upgrade['product_version']
if ($baseVersion -ge $upgradeVersion -or
    $base['stage_manifest_sha256'] -cne $upgrade['stage_manifest_sha256'] -or
    $base['msi_upgrade_code'] -cne $upgrade['msi_upgrade_code'] -or
    $base['bundle_upgrade_code'] -cne $upgrade['bundle_upgrade_code'] -or
    $base['msi_product_code'] -ceq $upgrade['msi_product_code'] -or
    $base['bundle_id'] -ceq $upgrade['bundle_id'] -or
    $base['msi_sha256'] -ceq $upgrade['msi_sha256'] -or
    $base['bundle_sha256'] -ceq $upgrade['bundle_sha256']) {
    throw 'Linked installer pair is not a valid same-payload major-upgrade pair.'
}

New-Item -ItemType Directory -Path $output | Out-Null
$result = [ordered]@{
    format = 1
    kind = 'mo-development-linked-upgrade-pair'
    development_only = $true
    install_executed = $false
    base_version = $base['product_version']
    upgrade_version = $upgrade['product_version']
    stage_manifest_sha256 = $base['stage_manifest_sha256']
    msi_upgrade_code = $base['msi_upgrade_code']
    bundle_upgrade_code = $base['bundle_upgrade_code']
    base_product_code = $base['msi_product_code']
    upgrade_product_code = $upgrade['msi_product_code']
    base_bundle_id = $base['bundle_id']
    upgrade_bundle_id = $upgrade['bundle_id']
    base_bundle_sha256 = $base['bundle_sha256']
    upgrade_bundle_sha256 = $upgrade['bundle_sha256']
}
$result | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (
    Join-Path $output 'linked-upgrade-pair-evidence.json') -Encoding utf8NoBOM
Write-Host "Verified linked major-upgrade pair $baseVersion -> $upgradeVersion without executing either installer."
