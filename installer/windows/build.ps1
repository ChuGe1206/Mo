#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$StageDirectory,
    [ValidatePattern('^\d+\.\d+\.\d+\.\d+$')]
    [string]$ProductVersion = '0.0.1.0',
    [switch]$AllowDevelopmentBuild
)

$ErrorActionPreference = 'Stop'

# Fail before creating output or inspecting artifacts when WiX is unavailable.
$wix = Get-Command wix -ErrorAction SilentlyContinue
if ($null -eq $wix) {
    throw 'WiX v4 CLI (`wix`) was not found. Install WiX v4 explicitly; this script will not download tools.'
}
if (-not $AllowDevelopmentBuild) {
    throw 'Refusing to build a non-deployable installer. Pass -AllowDevelopmentBuild only for explicit local authoring validation.'
}

. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'wix-payload.ps1')
$stage = Assert-MoPlainPath $StageDirectory
$null = Assert-MoPreparedStage $stage
$payload = Join-Path $stage 'payload/Mo'

function Invoke-Wix([string[]]$Arguments) {
    & $wix.Source @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "WiX failed with exit code $LASTEXITCODE."
    }
}

$outputDirectory = Join-Path $PSScriptRoot 'out'
$null = New-Item -ItemType Directory -Path $outputDirectory -Force
$payloadWxs = Join-Path $outputDirectory 'Payload.generated.wxs'
$msi = Join-Path $outputDirectory 'mo-development-unsigned.msi'
$bundle = Join-Path $outputDirectory 'mo-setup-development-unsigned.exe'
$null = New-MoWixPayloadFragment $stage $payloadWxs

Invoke-Wix @(
    'build', (Join-Path $PSScriptRoot 'Package.wxs'), $payloadWxs, '-arch', 'x64',
    '-d', "ProductVersion=$ProductVersion", '-d', "StagePayload=$payload", '-o', $msi
)
Invoke-Wix @(
    'build', (Join-Path $PSScriptRoot 'Bundle.wxs'),
    '-ext', 'WixToolset.Bal.wixext', '-d', "ProductVersion=$ProductVersion",
    '-d', "MsiPath=$msi", '-o', $bundle
)

Write-Warning 'Built an unsigned development package with full payload and transactional machine-profile actions. It has no current-user finalizer or release authorization and must not be distributed.'
