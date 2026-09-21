#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$BundlePath,
    [Parameter(Mandatory = $true)][string]$ProbeRegistrarPath,
    [Parameter(Mandatory = $true)][string]$StageDirectory,
    [Parameter(Mandatory = $true)][string]$LinkedEvidencePath,
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$output = Assert-MoNewBuildOutput $OutputDirectory $repo
$bundle = Assert-MoPlainPath $BundlePath
$registrar = Assert-MoPlainPath $ProbeRegistrarPath
$stage = Assert-MoPlainPath $StageDirectory
$linkedEvidence = Assert-MoPlainPath $LinkedEvidencePath
foreach ($file in @($bundle, $registrar, $linkedEvidence)) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "VM test kit input is missing: $file" }
}
$null = Assert-MoPreparedStage $stage
$stageManifest = Join-Path $stage 'mo-stage.json'
$stageRegistrar = Join-Path $stage 'payload\Mo\bin\mo-tip-registrar.exe'
$evidence = Read-MoStageJson $linkedEvidence
if ($evidence.Count -ne 16 -or $evidence['format'] -ne 3 -or
    $evidence['development_only'] -ne $true -or $evidence['install_executed'] -ne $false -or
    $evidence['build_flavor'] -cne 'DevelopmentTest' -or
    $evidence['fault_injection_included'] -ne $true -or
    $evidence['wix_version'] -cne '4.0.6+73c89738' -or
    $evidence['bundle_sha256'] -cnotmatch '^[A-F0-9]{64}$' -or
    $evidence['stage_manifest_sha256'] -cnotmatch '^[A-F0-9]{64}$') {
    throw 'Linked installer evidence is not the expected development contract.'
}
if ((Get-FileHash -LiteralPath $bundle -Algorithm SHA256).Hash -cne $evidence['bundle_sha256'] -or
    (Get-FileHash -LiteralPath $stageManifest -Algorithm SHA256).Hash -cne $evidence['stage_manifest_sha256'] -or
    (Get-FileHash -LiteralPath $registrar -Algorithm SHA256).Hash -cne
        (Get-FileHash -LiteralPath $stageRegistrar -Algorithm SHA256).Hash) {
    throw 'VM test kit input hash does not match linked/staged evidence.'
}

New-Item -ItemType Directory -Path $output | Out-Null
$copies = [ordered]@{
    'mo-setup-development-unsigned.exe' = $bundle
    'mo-tip-registrar.exe' = $registrar
    'mo-stage.json' = $stageManifest
    'vm-test-policy.ps1' = (Join-Path $PSScriptRoot 'vm-test-policy.ps1')
    'initialize-disposable-vm.ps1' = (Join-Path $PSScriptRoot 'initialize-disposable-vm.ps1')
    'run-vm-installer-lifecycle.ps1' = (Join-Path $PSScriptRoot 'run-vm-installer-lifecycle.ps1')
}
foreach ($name in $copies.Keys) {
    Copy-Item -LiteralPath $copies[$name] -Destination (Join-Path $output $name)
}
$manifest = [ordered]@{
    format = 1
    kind = 'mo-installer-vm-test-kit'
    development_only = $true
    redistributable = $false
    install_execution_authorized = $false
    wix_version = $evidence['wix_version']
    bundle_sha256 = $evidence['bundle_sha256']
    stage_manifest_sha256 = $evidence['stage_manifest_sha256']
    files = Get-MoStageInventory $output
}
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (
    Join-Path $output 'vm-test-kit.json') -Encoding utf8NoBOM
Assert-MoInventory $output $manifest.files @('vm-test-kit.json')
Write-Host "Prepared hash-locked disposable-VM installer test kit: $output"
Write-Warning 'The kit does not authorize execution. Initialize a throwaway VM explicitly before running its lifecycle driver.'
