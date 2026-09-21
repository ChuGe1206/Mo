#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$StageDirectory,
    [Parameter(Mandatory)][string]$LinkedEvidencePath,
    [Parameter(Mandatory)][string]$MsiPath,
    [Parameter(Mandatory)][string]$BundlePath,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'release-signing.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$output = Assert-MoNewBuildOutput $OutputDirectory $repo
$model = Get-MoReleaseSigningModel $StageDirectory $LinkedEvidencePath $MsiPath $BundlePath
New-Item -ItemType Directory -Path $output | Out-Null
ConvertTo-MoReleaseSigningPlan $model | ConvertTo-Json -Depth 10 |
    Set-Content (Join-Path $output 'release-signing-plan.json') -Encoding utf8NoBOM
Write-Host 'Prepared hash-bound six-step Authenticode plan for five inner payloads, MSI and final Bundle.'
Write-Warning 'No signing occurred. The existing stage, MSI and Bundle remain unsigned development evidence.'
