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
    [ValidateSet('DevelopmentTest', 'ProductionShape')]
    [string]$BuildFlavor = 'DevelopmentTest',
    [switch]$AllowDevelopmentBuild,
    [switch]$AllowProductionShapeBuild,
    [switch]$ValidateMsi
)

$ErrorActionPreference = 'Stop'

if (-not $AllowDevelopmentBuild) {
    throw 'Refusing to build a non-deployable installer. Pass -AllowDevelopmentBuild only for explicit local authoring validation.'
}
if ($BuildFlavor -eq 'ProductionShape' -and -not $AllowProductionShapeBuild) {
    throw 'Production-shape authoring requires -AllowProductionShapeBuild; output remains unsigned and non-deployable.'
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
$includeFaultInjection = if ($BuildFlavor -eq 'DevelopmentTest') { '1' } else { '0' }
$registrar = Join-Path $payload 'bin/mo-tip-registrar.exe'
$beforeFaultProbe = @(& $registrar status)
if ($LASTEXITCODE -ne 0) { throw 'Staged registrar status probe failed.' }
$faultProbe = @(& $registrar development-test-fail-fixed 2>&1)
$faultExitCode = $LASTEXITCODE
$faultText = $faultProbe -join "`n"
$afterFaultProbe = @(& $registrar status)
if ($LASTEXITCODE -ne 0 -or ($beforeFaultProbe -join "`n") -cne ($afterFaultProbe -join "`n")) {
    throw 'Staged registrar flavor probe changed observable state.'
}
if ($BuildFlavor -eq 'DevelopmentTest') {
    if ($faultExitCode -eq 0 -or $faultText -cnotmatch '(?m)^Operation failed: 0x80004005$' -or
        $faultText -match '(?m)^Usage:$') {
        throw 'DevelopmentTest requires a staged registrar with deterministic fault injection.'
    }
} elseif ($faultExitCode -eq 0 -or $faultText -cnotmatch '(?m)^Usage:$' -or
    $faultText -cnotmatch '(?m)^Operation failed: 0x80070057$' -or
    $faultText -match '0x80004005') {
    throw 'ProductionShape requires a staged registrar with fault injection physically absent.'
}

function Invoke-Wix([string[]]$Arguments) {
    & $wix @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "WiX failed with exit code $LASTEXITCODE."
    }
}

New-Item -ItemType Directory -Path $outputDirectory | Out-Null
$payloadWxs = Join-Path $outputDirectory 'Payload.generated.wxs'
$artifactStem = if ($BuildFlavor -eq 'DevelopmentTest') { 'development' } else { 'production-shape' }
$msi = Join-Path $outputDirectory "mo-$artifactStem-unsigned.msi"
$bundle = Join-Path $outputDirectory "mo-setup-$artifactStem-unsigned.exe"
$failureInjector = Join-Path $outputDirectory 'mo-development-failure-injection.exe'
$null = New-MoWixPayloadFragment $stage $payloadWxs
if ($BuildFlavor -eq 'DevelopmentTest') {
    Copy-Item -LiteralPath $registrar -Destination $failureInjector
}

Invoke-Wix @(
    'build', (Join-Path $PSScriptRoot 'Package.wxs'), $payloadWxs, '-arch', 'x64',
    '-d', "ProductVersion=$ProductVersion", '-d', "StagePayload=$payload",
    '-d', "IncludeFaultInjection=$includeFaultInjection", '-o', $msi
)
$bundleArguments = @(
    'build', (Join-Path $PSScriptRoot 'Bundle.wxs'),
    '-ext', $toolchain.Extensions.Bal,
    '-ext', $toolchain.Extensions.Util,
    '-ext', $toolchain.Extensions.Dependency,
    '-sw1140',
    '-d', "ProductVersion=$ProductVersion", '-d', "MsiPath=$msi",
    '-d', "UserFinalizerExe=$registrar",
    '-d', "IncludeFaultInjection=$includeFaultInjection"
)
if ($BuildFlavor -eq 'DevelopmentTest') {
    $bundleArguments += @('-d', "FailureInjectorExe=$failureInjector")
}
$bundleArguments += @('-o', $bundle)
Invoke-Wix $bundleArguments

& (Join-Path $PSScriptRoot 'verify-linked-installer.ps1') `
    -WixPath $wix `
    -StageDirectory $stage `
    -MsiPath $msi `
    -BundlePath $bundle `
    -ProductVersion $ProductVersion `
    -BuildFlavor $BuildFlavor `
    -OutputDirectory (Join-Path $outputDirectory 'verification') `
    -ValidateMsi:$ValidateMsi
if ($LASTEXITCODE -ne 0) { throw 'Linked installer verification failed.' }

$iceStatus = if ($ValidateMsi) { 'MSI ICE passed;' } else { 'MSI ICE was not run;' }
Write-Warning "Built and structurally verified an unsigned, non-deployable $BuildFlavor package. WIX1140 is intentionally suppressed for the mixed-scope chain; $iceStatus signatures and execution remain release gates. Do not distribute."
