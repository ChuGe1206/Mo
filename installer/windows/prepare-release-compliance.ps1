#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$StageDirectory,
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [string]$PolicyPath = (Join-Path $PSScriptRoot 'release-compliance-policy.json')
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'release-compliance.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$output = Assert-MoNewBuildOutput $OutputDirectory $repo
$model = Get-MoReleaseComplianceModel $StageDirectory $PolicyPath
$stageManifestHash = (Get-FileHash -LiteralPath (Join-Path $model['stage'] 'mo-stage.json') -Algorithm SHA256).Hash
$policyHash = (Get-FileHash -LiteralPath (Assert-MoPlainPath $PolicyPath) -Algorithm SHA256).Hash
$spdx = ConvertTo-MoSpdxDocument $model $stageManifestHash

New-Item -ItemType Directory -Path $output | Out-Null
$spdxPath = Join-Path $output 'mo-windows-payload.spdx.json'
$spdx | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $spdxPath -Encoding utf8NoBOM

$noticePath = Join-Path $output 'THIRD_PARTY_NOTICES.draft.txt'
Get-MoThirdPartyNotices $model | Set-Content -LiteralPath $noticePath -Encoding utf8NoBOM

$evidence = [ordered]@{
    format = 1
    kind = 'mo-release-compliance-draft'
    development_only = $true
    release_authorized = $false
    legal_review_complete = $false
    stage_manifest_sha256 = $stageManifestHash
    policy_sha256 = $policyHash
    payload_file_count = $model['files'].Count
    package_count = $model['packages'].Count
    component_file_counts = $model['component_counts']
    release_blockers = $model['policy']['release_blockers']
    spdx_sha256 = (Get-FileHash -LiteralPath $spdxPath -Algorithm SHA256).Hash
    notices_sha256 = (Get-FileHash -LiteralPath $noticePath -Algorithm SHA256).Hash
}
$evidence | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (
    Join-Path $output 'release-compliance-evidence.json') -Encoding utf8NoBOM
Write-Host "Prepared deterministic SPDX 2.3 and notices drafts for $($model['files'].Count) payload files."
Write-Warning 'Release authorization remains false: legal review, license texts, GPL source bundle, signatures, MSI ICE and disposable-VM execution are still required.'
