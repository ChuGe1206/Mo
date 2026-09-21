#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$StageDirectory,
    [Parameter(Mandatory)][string]$RuntimeBuildDirectory,
    [Parameter(Mandatory)][string]$CargoRegistrySourceDirectory,
    [Parameter(Mandatory)][string]$BoostArchivePath,
    [Parameter(Mandatory)][string]$LuaArchivePath,
    [Parameter(Mandatory)][string]$ComplianceDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$PolicyPath = (Join-Path $PSScriptRoot 'release-materials-policy.json'),
    [string]$CompliancePolicyPath = (Join-Path $PSScriptRoot 'release-compliance-policy.json')
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'release-compliance.ps1')
. (Join-Path $PSScriptRoot 'release-materials.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$output = Assert-MoNewBuildOutput $OutputDirectory $repo
$model = Get-MoReleaseMaterialsModel $StageDirectory $RuntimeBuildDirectory `
    $CargoRegistrySourceDirectory $BoostArchivePath $LuaArchivePath $ComplianceDirectory `
    $PolicyPath $CompliancePolicyPath
New-Item -ItemType Directory -Path (Join-Path $output 'sources'), (Join-Path $output 'licenses') | Out-Null
foreach ($group in @('archives', 'documents')) {
    $folder = if ($group -ceq 'archives') { 'sources' } else { 'licenses' }
    foreach ($record in $model['records'][$group]) {
        Copy-Item -LiteralPath $record['source'] -Destination (Join-Path $output "$folder/$($record['file_name'])")
    }
}
$manifestPath = Join-Path $output 'materials-manifest.json'
ConvertTo-MoReleaseMaterialsManifest $model | ConvertTo-Json -Depth 10 | Set-Content $manifestPath -Encoding utf8NoBOM
$evidence = [ordered]@{
    format = 1; kind = 'mo-release-materials-evidence'; development_only = $true
    technical_materials_complete = $true; legal_review_complete = $false; release_authorized = $false
    archive_count = $model['records']['archives'].Count; document_count = $model['records']['documents'].Count
    gpl_corresponding_source_ids = @('rime-ice')
    manifest_sha256 = (Get-FileHash $manifestPath -Algorithm SHA256).Hash
}
$evidence | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $output 'release-materials-evidence.json') -Encoding utf8NoBOM
Write-Host "Prepared $($evidence['archive_count']) source archives and $($evidence['document_count']) license/notice documents."
Write-Warning 'Technical materials are complete; formal legal review and release authorization remain false.'
