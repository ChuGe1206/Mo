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
    throw 'Pinned WiX 4.0.6 CLI (`wix`) was not found. Install it explicitly; this script will not download tools.'
}
$requiredWixVersion = '4.0.6'
$wixVersion = (& $wix.Source --version | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $wixVersion -notmatch '^4\.0\.6(?:\+.*)?$') {
    throw "Expected WiX $requiredWixVersion, found '$wixVersion'."
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
    '-ext', 'WixToolset.Bal.wixext/4.0.6',
    '-ext', 'WixToolset.Util.wixext/4.0.6',
    '-ext', 'WixToolset.Dependency.wixext/4.0.6',
    '-d', "ProductVersion=$ProductVersion", '-d', "MsiPath=$msi",
    '-d', "UserFinalizerExe=$(Join-Path $payload 'bin/mo-tip-registrar.exe')", '-o', $bundle
)

Write-Warning 'Built an unsigned development package with full payload, machine-profile rollback and an unelevated current-user finalizer. It has no release authorization and must not be distributed.'
