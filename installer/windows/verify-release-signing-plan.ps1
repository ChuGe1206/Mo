#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$StageDirectory,
    [Parameter(Mandatory)][string]$LinkedEvidencePath,
    [Parameter(Mandatory)][string]$MsiPath,
    [Parameter(Mandatory)][string]$BundlePath,
    [Parameter(Mandatory)][string]$SigningPlanDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'release-signing.ps1')
$directory = Assert-MoPlainPath $SigningPlanDirectory
$files = @(Get-MoStageFiles $directory)
if ($files.Count -ne 1 -or $files[0] -cne 'release-signing-plan.json') {
    throw 'Release signing plan output inventory mismatch.'
}
$model = Get-MoReleaseSigningModel $StageDirectory $LinkedEvidencePath $MsiPath $BundlePath
$expected = ConvertTo-MoReleaseSigningPlan $model | ConvertTo-Json -Depth 10
$actual = [IO.File]::ReadAllText((Join-Path $directory 'release-signing-plan.json')).TrimEnd("`r", "`n")
if ($actual -cne $expected) { throw 'Release signing plan mismatch.' }
Write-Host 'Verified unsigned baselines and mandatory inner-to-outer Authenticode order; no signing occurred.'
