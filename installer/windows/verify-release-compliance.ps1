#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$StageDirectory,
    [Parameter(Mandatory = $true)][string]$ComplianceDirectory,
    [string]$PolicyPath = (Join-Path $PSScriptRoot 'release-compliance-policy.json')
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'release-compliance.ps1')
$directory = Assert-MoPlainPath $ComplianceDirectory
$model = Get-MoReleaseComplianceModel $StageDirectory $PolicyPath
$expectedFiles = @(
    'mo-windows-payload.spdx.json',
    'release-compliance-evidence.json',
    'THIRD_PARTY_NOTICES.draft.txt'
)
$actualFiles = @(Get-MoStageFiles $directory)
if ($actualFiles.Count -ne $expectedFiles.Count -or
    @($actualFiles | Where-Object { $_ -cnotin $expectedFiles }).Count -or
    @($expectedFiles | Where-Object { $_ -cnotin $actualFiles }).Count) {
    throw 'Release compliance output inventory mismatch.'
}

$stageManifestHash = (Get-FileHash -LiteralPath (Join-Path $model['stage'] 'mo-stage.json') -Algorithm SHA256).Hash
$policyHash = (Get-FileHash -LiteralPath (Assert-MoPlainPath $PolicyPath) -Algorithm SHA256).Hash
$spdxPath = Join-Path $directory 'mo-windows-payload.spdx.json'
$noticePath = Join-Path $directory 'THIRD_PARTY_NOTICES.draft.txt'
$evidencePath = Join-Path $directory 'release-compliance-evidence.json'

$expectedSpdx = ConvertTo-MoSpdxDocument $model $stageManifestHash | ConvertTo-Json -Depth 20
$actualSpdx = [IO.File]::ReadAllText($spdxPath).TrimEnd("`r", "`n")
if ($actualSpdx -cne $expectedSpdx) { throw 'SPDX draft does not match the staged payload and policy.' }
$expectedNotice = ((Get-MoThirdPartyNotices $model) -join [Environment]::NewLine).TrimEnd("`r", "`n")
$actualNotice = [IO.File]::ReadAllText($noticePath).TrimEnd("`r", "`n")
if ($actualNotice -cne $expectedNotice) { throw 'Third-party notices draft does not match policy.' }

$evidence = Read-MoStageJson $evidencePath
$required = @(
    'format', 'kind', 'development_only', 'release_authorized',
    'legal_review_complete', 'stage_manifest_sha256', 'policy_sha256',
    'payload_file_count', 'package_count', 'component_file_counts',
    'release_blockers', 'spdx_sha256', 'notices_sha256'
)
if ($evidence.Count -ne $required.Count -or
    @($required | Where-Object { -not $evidence.Contains($_) }).Count -or
    $evidence['format'] -ne 1 -or $evidence['kind'] -cne 'mo-release-compliance-draft' -or
    $evidence['development_only'] -ne $true -or $evidence['release_authorized'] -ne $false -or
    $evidence['legal_review_complete'] -ne $false -or
    $evidence['stage_manifest_sha256'] -cne $stageManifestHash -or
    $evidence['policy_sha256'] -cne $policyHash -or
    $evidence['payload_file_count'] -ne $model['files'].Count -or
    $evidence['package_count'] -ne $model['packages'].Count -or
    $evidence['spdx_sha256'] -cne (Get-FileHash -LiteralPath $spdxPath -Algorithm SHA256).Hash -or
    $evidence['notices_sha256'] -cne (Get-FileHash -LiteralPath $noticePath -Algorithm SHA256).Hash) {
    throw 'Release compliance evidence header mismatch.'
}
if ($evidence['component_file_counts'].Count -ne $model['component_counts'].Count -or
    @($model['component_counts'].Keys | Where-Object {
        -not $evidence['component_file_counts'].Contains($_) -or
        $evidence['component_file_counts'][$_] -ne $model['component_counts'][$_]
    }).Count) {
    throw 'Release compliance component coverage mismatch.'
}
if ($evidence['release_blockers'].Count -ne $model['policy']['release_blockers'].Count -or
    @($model['policy']['release_blockers'] | Where-Object { $_ -cnotin $evidence['release_blockers'] }).Count) {
    throw 'Release compliance blocker list mismatch.'
}
Write-Host "Verified deterministic compliance drafts for $($model['files'].Count) payload files; release authorization remains false."
