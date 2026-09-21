#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$StageDirectory,
    [Parameter(Mandatory)][string]$RuntimeBuildDirectory,
    [Parameter(Mandatory)][string]$CargoRegistrySourceDirectory,
    [Parameter(Mandatory)][string]$BoostArchivePath,
    [Parameter(Mandatory)][string]$LuaArchivePath,
    [Parameter(Mandatory)][string]$ComplianceDirectory,
    [Parameter(Mandatory)][string]$MaterialsDirectory,
    [string]$PolicyPath = (Join-Path $PSScriptRoot 'release-materials-policy.json'),
    [string]$CompliancePolicyPath = (Join-Path $PSScriptRoot 'release-compliance-policy.json')
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'release-compliance.ps1')
. (Join-Path $PSScriptRoot 'release-materials.ps1')
$directory = Assert-MoPlainPath $MaterialsDirectory
$model = Get-MoReleaseMaterialsModel $StageDirectory $RuntimeBuildDirectory `
    $CargoRegistrySourceDirectory $BoostArchivePath $LuaArchivePath $ComplianceDirectory `
    $PolicyPath $CompliancePolicyPath
$expectedFiles = @('materials-manifest.json', 'release-materials-evidence.json') +
    @($model['records']['archives'] | ForEach-Object { 'sources/' + $_['file_name'] }) +
    @($model['records']['documents'] | ForEach-Object { 'licenses/' + $_['file_name'] })
$actualFiles = @(Get-MoStageFiles $directory)
if ($actualFiles.Count -ne $expectedFiles.Count -or
    @($actualFiles | Where-Object { $_ -cnotin $expectedFiles }).Count -or
    @($expectedFiles | Where-Object { $_ -cnotin $actualFiles }).Count) {
    throw 'Release materials output inventory mismatch.'
}
foreach ($group in @('archives', 'documents')) {
    $folder = if ($group -ceq 'archives') { 'sources' } else { 'licenses' }
    foreach ($record in $model['records'][$group]) {
        $path = Join-Path $directory "$folder/$($record['file_name'])"
        if ((Get-Item $path).Length -ne $record['size'] -or
            (Get-FileHash $path -Algorithm SHA256).Hash -cne $record['sha256']) {
            throw "Release material output hash mismatch: $($record['id'])"
        }
    }
}
$manifestPath = Join-Path $directory 'materials-manifest.json'
$expectedManifest = ConvertTo-MoReleaseMaterialsManifest $model | ConvertTo-Json -Depth 10
$actualManifest = [IO.File]::ReadAllText($manifestPath).TrimEnd("`r", "`n")
if ($actualManifest -cne $expectedManifest) { throw 'Release materials manifest mismatch.' }
$evidence = Read-MoStageJson (Join-Path $directory 'release-materials-evidence.json')
$required = @('format', 'kind', 'development_only', 'technical_materials_complete', 'legal_review_complete',
    'release_authorized', 'archive_count', 'document_count', 'gpl_corresponding_source_ids', 'manifest_sha256')
if ($evidence.Count -ne $required.Count -or @($required | Where-Object { -not $evidence.Contains($_) }).Count -or
    $evidence['format'] -ne 1 -or $evidence['kind'] -cne 'mo-release-materials-evidence' -or
    $evidence['development_only'] -ne $true -or $evidence['technical_materials_complete'] -ne $true -or
    $evidence['legal_review_complete'] -ne $false -or $evidence['release_authorized'] -ne $false -or
    $evidence['archive_count'] -ne 9 -or $evidence['document_count'] -ne 15 -or
    $evidence['gpl_corresponding_source_ids'].Count -ne 1 -or
    $evidence['gpl_corresponding_source_ids'][0] -cne 'rime-ice' -or
    $evidence['manifest_sha256'] -cne (Get-FileHash $manifestPath -Algorithm SHA256).Hash) {
    throw 'Release materials evidence mismatch.'
}
Write-Host 'Verified 9 source archives and 15 license/notice documents; release authorization remains false.'
