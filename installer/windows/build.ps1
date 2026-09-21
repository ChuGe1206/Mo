#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$StageDirectory,
    [Parameter(Mandatory = $true)]
    [string]$WixToolchainDirectory,
    [Parameter(Mandatory = $true)]
    [string]$OutputDirectory,
    [ValidatePattern('^\d+\.\d+\.\d+\.\d+$')]
    [string]$ProductVersion = '0.0.1.0',
    [switch]$AllowDevelopmentBuild
)

$ErrorActionPreference = 'Stop'

if (-not $AllowDevelopmentBuild) {
    throw 'Refusing to build a non-deployable installer. Pass -AllowDevelopmentBuild only for explicit local authoring validation.'
}

. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'wix-payload.ps1')
. (Join-Path $PSScriptRoot 'wix-toolchain.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$toolchain = Assert-MoWixToolchain $WixToolchainDirectory
$wix = $toolchain.Wix
$stage = Assert-MoPlainPath $StageDirectory
$null = Assert-MoPreparedStage $stage
$payload = Join-Path $stage 'payload/Mo'
$outputDirectory = Assert-MoNewBuildOutput $OutputDirectory $repo

function Invoke-Wix([string[]]$Arguments) {
    & $wix @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "WiX failed with exit code $LASTEXITCODE."
    }
}

New-Item -ItemType Directory -Path $outputDirectory | Out-Null
$payloadWxs = Join-Path $outputDirectory 'Payload.generated.wxs'
$msi = Join-Path $outputDirectory 'mo-development-unsigned.msi'
$bundle = Join-Path $outputDirectory 'mo-setup-development-unsigned.exe'
$failureInjector = Join-Path $outputDirectory 'mo-development-failure-injection.exe'
$null = New-MoWixPayloadFragment $stage $payloadWxs
Copy-Item -LiteralPath (Join-Path $payload 'bin/mo-tip-registrar.exe') -Destination $failureInjector

Invoke-Wix @(
    'build', (Join-Path $PSScriptRoot 'Package.wxs'), $payloadWxs, '-arch', 'x64',
    '-d', "ProductVersion=$ProductVersion", '-d', "StagePayload=$payload", '-o', $msi
)
Invoke-Wix @(
    'build', (Join-Path $PSScriptRoot 'Bundle.wxs'),
    '-ext', $toolchain.Extensions.Bal,
    '-ext', $toolchain.Extensions.Util,
    '-ext', $toolchain.Extensions.Dependency,
    '-sw1140',
    '-d', "ProductVersion=$ProductVersion", '-d', "MsiPath=$msi",
    '-d', "UserFinalizerExe=$(Join-Path $payload 'bin/mo-tip-registrar.exe')",
    '-d', "FailureInjectorExe=$failureInjector", '-o', $bundle
)

& (Join-Path $PSScriptRoot 'verify-linked-installer.ps1') `
    -WixPath $wix `
    -StageDirectory $stage `
    -MsiPath $msi `
    -BundlePath $bundle `
    -ProductVersion $ProductVersion `
    -OutputDirectory (Join-Path $outputDirectory 'verification')
if ($LASTEXITCODE -ne 0) { throw 'Linked installer verification failed.' }

Write-Warning 'Built and structurally verified an unsigned development package. WIX1140 is intentionally suppressed for the mixed-scope chain; MSI ICE and execution remain VM gates. Do not distribute.'
