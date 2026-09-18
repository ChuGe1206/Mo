#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$StageDirectory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
$metadata = Assert-MoPreparedStage $StageDirectory
Write-Host "Development staging inventory verified ($($metadata['files'].Count) files). NOT installable or redistributable."
