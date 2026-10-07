#Requires -Version 7.4
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'test-fixture.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $repo ('build/mo-changed-upgrade-policy-' + [Guid]::NewGuid().ToString('N'))
$null = Assert-MoNewBuildOutput $fixture $repo
New-Item -ItemType Directory -Path $fixture | Out-Null
$script:testCount = 0
$basePath = Join-Path $fixture 'base.json'
$upgradePath = Join-Path $fixture 'upgrade.json'
$verifier = Join-Path $PSScriptRoot 'verify-linked-upgrade-pair.ps1'

function Write-Inputs {
    $base | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $basePath -Encoding utf8NoBOM
    $upgrade | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $upgradePath -Encoding utf8NoBOM
}

function Verify([string]$Mode = 'Same') {
    Write-Inputs
    $output = Join-Path $fixture ('pair-' + [Guid]::NewGuid().ToString('N'))
    $parameters = @{
        BaseEvidencePath = $basePath; UpgradeEvidencePath = $upgradePath
        OutputDirectory = $output
    }
    # Exercise the backwards-compatible default as well as explicit Changed.
    if ($Mode -cne 'Same') { $parameters.PayloadMode = $Mode }
    & $verifier @parameters | Out-Host
    return Read-MoStageJson (Join-Path $output 'linked-upgrade-pair-evidence.json')
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

try {
    $base = [ordered]@{
        format = 3; development_only = $true; install_executed = $false
        build_flavor = 'DevelopmentTest'; fault_injection_included = $true
        wix_version = '4.0.6+73c89738'; product_version = '0.0.11.0'
        msi_product_code = '{11111111-1111-4111-8111-111111111111}'
        msi_upgrade_code = '{8245E1F5-8BC4-4D41-BA9C-65F30CEBCB32}'
        bundle_id = '{33333333-3333-4333-8333-333333333333}'
        bundle_upgrade_code = '{E08C0321-3F73-4D39-BC28-E7DF02C5134E}'
        known_link_warning = 'synthetic'; msi_ice_validated = $true
        stage_manifest_sha256 = ('A' * 64); msi_sha256 = ('B' * 64); bundle_sha256 = ('C' * 64)
    }
    $upgrade = [ordered]@{}
    foreach ($key in $base.Keys) { $upgrade[$key] = $base[$key] }
    $upgrade.product_version = '0.0.12.0'
    $upgrade.msi_product_code = '{22222222-2222-4222-8222-222222222222}'
    $upgrade.bundle_id = '{44444444-4444-4444-8444-444444444444}'
    $upgrade.msi_sha256 = ('D' * 64)
    $upgrade.bundle_sha256 = ('E' * 64)
    $same = Verify
    if ($same.format -ne 2 -or $same.kind -cne 'mo-linked-upgrade-pair' -or
        $same.Count -ne 17 -or $same.stage_manifest_sha256 -cne $base.stage_manifest_sha256) {
        throw 'Legacy same-payload receipt changed.'
    }
    $script:testCount++
    Write-Host 'PASS default same-payload receipt remains compatible'
    Reject 'same stage declared changed' { Verify Changed } 'requested Changed payload mode'
    $upgrade.stage_manifest_sha256 = ('F' * 64)
    Reject 'changed stage declared same by default' { Verify } 'requested Same payload mode'
    $changed = Verify Changed
    if ($changed.format -ne 3 -or $changed.kind -cne 'mo-linked-changed-payload-upgrade-pair' -or
        $changed.Count -ne 20 -or $changed.Contains('stage_manifest_sha256') -or
        $changed.base_stage_manifest_sha256 -cne $base.stage_manifest_sha256 -or
        $changed.upgrade_stage_manifest_sha256 -cne $upgrade.stage_manifest_sha256 -or
        $changed.base_linked_evidence_sha256 -cne (Get-FileHash -LiteralPath $basePath).Hash -or
        $changed.upgrade_linked_evidence_sha256 -cne (Get-FileHash -LiteralPath $upgradePath).Hash) {
        throw 'Changed-payload receipt lost independent input bindings.'
    }
    $script:testCount++
    Write-Host 'PASS changed payload binds both stages and source receipts'
    foreach ($case in @(
        @('fourth-only version', 'product_version', '0.0.11.1', 'first three'),
        @('different MSI family', 'msi_upgrade_code', '{55555555-5555-4555-8555-555555555555}', 'valid major-upgrade'),
        @('different Bundle family', 'bundle_upgrade_code', '{55555555-5555-4555-8555-555555555555}', 'valid major-upgrade'),
        @('same product code', 'msi_product_code', $base.msi_product_code, 'valid major-upgrade'),
        @('same Bundle identity', 'bundle_id', $base.bundle_id, 'valid major-upgrade'),
        @('same MSI bytes', 'msi_sha256', $base.msi_sha256, 'valid major-upgrade'),
        @('same Bundle bytes', 'bundle_sha256', $base.bundle_sha256, 'valid major-upgrade'),
        @('invalid stage hash', 'stage_manifest_sha256', 'unknown', 'Invalid hash'),
        @('already executed receipt', 'install_executed', $true, 'Invalid linked'),
        @('unsupported receipt version', 'format', 4, 'Invalid linked')
    )) {
        $saved = $upgrade[$case[1]]
        $upgrade[$case[1]] = $case[2]
        Reject $case[0] { Verify Changed } $case[3]
        $upgrade[$case[1]] = $saved
    }
    Write-Host "Upgrade-pair policy tests passed: $script:testCount. No installer execution."
} finally {
    $resolved = Assert-MoPlainPath $fixture
    if (-not $resolved.StartsWith((Join-Path $repo 'build') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Unsafe upgrade-pair fixture cleanup target.'
    }
    Assert-MoOwnedFixtureTree $resolved
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
