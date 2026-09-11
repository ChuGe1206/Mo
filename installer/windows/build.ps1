[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$BrokerExe,
    [Parameter(Mandatory = $true)]
    [string]$TipX64Dll,
    [Parameter(Mandatory = $true)]
    [string]$TipX86Dll,
    [Parameter(Mandatory = $true)]
    [string]$RegistrarExe,
    [ValidatePattern('^\d+\.\d+\.\d+\.\d+$')]
    [string]$ProductVersion = '0.0.1.0',
    [switch]$AllowPlaceholderBuild
)

$ErrorActionPreference = 'Stop'

# Fail before creating output or inspecting artifacts when WiX is unavailable.
$wix = Get-Command wix -ErrorAction SilentlyContinue
if ($null -eq $wix) {
    throw 'WiX v4 CLI (`wix`) was not found. Install WiX v4 explicitly; this script will not download tools.'
}
if (-not $AllowPlaceholderBuild) {
    throw 'Refusing to build a non-deployable installer. Pass -AllowPlaceholderBuild only for Phase 0 authoring validation.'
}

function Resolve-InputArtifact([string]$Path, [string]$Label) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Label does not exist: $Path"
    }
    return (Resolve-Path -LiteralPath $Path).Path
}

function Invoke-Wix([string[]]$Arguments) {
    & $wix.Source @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "WiX failed with exit code $LASTEXITCODE."
    }
}

$broker = Resolve-InputArtifact $BrokerExe 'Broker executable'
$tipX64 = Resolve-InputArtifact $TipX64Dll 'x64 TIP DLL'
$tipX86 = Resolve-InputArtifact $TipX86Dll 'x86 TIP DLL'
$registrar = Resolve-InputArtifact $RegistrarExe 'Registrar executable'
$outputDirectory = Join-Path $PSScriptRoot 'out'
$null = New-Item -ItemType Directory -Path $outputDirectory -Force
$msi = Join-Path $outputDirectory 'mo-phase0-placeholder.msi'
$bundle = Join-Path $outputDirectory 'mo-setup-phase0-placeholder.exe'

Invoke-Wix @(
    'build', (Join-Path $PSScriptRoot 'Package.wxs'), '-arch', 'x64',
    '-d', "ProductVersion=$ProductVersion", '-d', "BrokerExe=$broker",
    '-d', "TipX64Dll=$tipX64", '-d', "TipX86Dll=$tipX86",
    '-d', "RegistrarExe=$registrar", '-o', $msi
)
Invoke-Wix @(
    'build', (Join-Path $PSScriptRoot 'Bundle.wxs'),
    '-ext', 'WixToolset.Bal.wixext', '-d', "ProductVersion=$ProductVersion",
    '-d', "MsiPath=$msi", '-o', $bundle
)

Write-Warning 'Built unsigned Phase 0 placeholders. They do not register or enable the TSF profile and must not be distributed.'

