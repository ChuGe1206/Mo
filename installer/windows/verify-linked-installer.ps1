#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$WixPath,
    [Parameter(Mandatory = $true)]
    [string]$StageDirectory,
    [Parameter(Mandatory = $true)]
    [string]$MsiPath,
    [Parameter(Mandatory = $true)]
    [string]$BundlePath,
    [ValidatePattern('^\d+\.\d+\.\d+\.\d+$')]
    [string]$ProductVersion = '0.0.1.0',
    [Parameter(Mandatory = $true)]
    [string]$OutputDirectory,
    [switch]$ValidateMsi
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')

$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$wix = Assert-MoPlainPath $WixPath
$stage = Assert-MoPlainPath $StageDirectory
$msi = Assert-MoPlainPath $MsiPath
$bundle = Assert-MoPlainPath $BundlePath
$output = Assert-MoNewBuildOutput $OutputDirectory $repo
foreach ($file in @($wix, $msi, $bundle)) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Linked-installer input is missing: $file" }
}
$null = Assert-MoPreparedStage $stage
$version = (& $wix --version | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $version -cne '4.0.6+73c89738') {
    throw "Linked-installer verification requires WiX 4.0.6+73c89738, found '$version'."
}

New-Item -ItemType Directory -Path $output | Out-Null
$decompiled = Join-Path $output 'Package.decompiled.wxs'
$msiFiles = Join-Path $output 'msi-files'
& $wix msi decompile $msi -o $decompiled -x $msiFiles
if ($LASTEXITCODE -ne 0) { throw 'WiX failed to decompile the linked MSI.' }

$package = [xml](Get-Content -Raw -LiteralPath $decompiled)
$packageNamespace = [Xml.XmlNamespaceManager]::new($package.NameTable)
$packageNamespace.AddNamespace('w', 'http://wixtoolset.org/schemas/v4/wxs')
if (@($package.SelectNodes('//w:File', $packageNamespace)).Count -ne 131 -or
    @($package.SelectNodes('//w:Component', $packageNamespace)).Count -ne 132 -or
    @($package.SelectNodes('//w:CustomAction', $packageNamespace)).Count -ne 7) {
    throw 'Linked MSI payload/component/custom-action count mismatch.'
}
$launch = $package.SelectSingleNode('//w:Launch', $packageNamespace)
if ($null -eq $launch -or $launch.GetAttribute('Condition') -cne 'NOT RollbackDisabled') {
    throw 'Linked MSI lost the rollback-required launch condition.'
}
$embeddedRegistrar = $package.SelectSingleNode('//w:Binary[@Id="MoRegistrarCustomAction"]', $packageNamespace)
if ($null -eq $embeddedRegistrar) { throw 'Linked MSI registrar binary is missing.' }
$embeddedRegistrarPath = $embeddedRegistrar.GetAttribute('SourceFile')
$stageRegistrar = Join-Path $stage 'payload\Mo\bin\mo-tip-registrar.exe'
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $embeddedRegistrarPath).Hash -cne
    (Get-FileHash -Algorithm SHA256 -LiteralPath $stageRegistrar).Hash) {
    throw 'Linked MSI embedded registrar differs from the verified stage.'
}

$bundlePayload = Join-Path $output 'bundle-payload'
$bundleBa = Join-Path $output 'bundle-ba'
& $wix burn extract $bundle -o $bundlePayload -oba $bundleBa
if ($LASTEXITCODE -ne 0) { throw 'WiX failed to extract the linked Bundle.' }
$manifestPath = Join-Path $bundleBa 'manifest.xml'
$manifest = [xml](Get-Content -Raw -LiteralPath $manifestPath)
$burnNamespace = [Xml.XmlNamespaceManager]::new($manifest.NameTable)
$burnNamespace.AddNamespace('b', 'http://wixtoolset.org/schemas/v4/2008/Burn')
$registration = $manifest.SelectSingleNode('/b:BurnManifest/b:Registration', $burnNamespace)
$machinePackage = $manifest.SelectSingleNode('//b:MsiPackage[@Id="MoMachinePackage"]', $burnNamespace)
$finalizer = $manifest.SelectSingleNode('//b:ExePackage[@Id="MoCurrentUserFinalizer"]', $burnNamespace)
if ($manifest.DocumentElement.GetAttribute('EngineVersion') -cne '4.0.6.0' -or
    $null -eq $registration -or $registration.GetAttribute('PerMachine') -cne 'no' -or
    $null -eq $machinePackage -or $machinePackage.GetAttribute('PerMachine') -cne 'yes' -or
    $null -eq $finalizer -or $finalizer.GetAttribute('PerMachine') -cne 'no' -or
    $finalizer.GetAttribute('InstallArguments') -cne 'burn-user-finalizer' -or
    $finalizer.GetAttribute('RepairArguments') -cne 'burn-user-finalizer' -or
    $finalizer.GetAttribute('UninstallArguments') -cne 'burn-user-finalizer' -or
    $finalizer.GetAttribute('Repairable') -cne 'yes' -or
    $finalizer.GetAttribute('Uninstallable') -cne 'yes') {
    throw 'Linked Bundle scope or finalizer base protocol mismatch.'
}
$expectedCommands = [ordered]@{
    'WixBundleAction = 4' = @('rollback-remove-current-user-fixed', '', 'remove-current-user-fixed')
    'WixBundleAction = 6' = @('install-current-user-fixed', '', 'rollback-install-current-user-fixed')
    'WixBundleAction = 8' = @('repair-current-user-fixed', 'repair-current-user-fixed', '')
}
$commands = @($finalizer.SelectNodes('b:CommandLine', $burnNamespace))
if ($commands.Count -ne $expectedCommands.Count) { throw 'Linked Bundle command count mismatch.' }
foreach ($command in $commands) {
    $expected = $expectedCommands[$command.GetAttribute('Condition')]
    if ($null -eq $expected -or
        $command.GetAttribute('InstallArgument') -cne $expected[0] -or
        $command.GetAttribute('RepairArgument') -cne $expected[1] -or
        $command.GetAttribute('UninstallArgument') -cne $expected[2]) {
        throw "Linked Bundle command mismatch: $($command.GetAttribute('Condition'))"
    }
}
$provider = $finalizer.SelectSingleNode('b:Provides', $burnNamespace)
if ($null -eq $provider -or $provider.GetAttribute('Key') -cne 'Mo.CurrentUserFinalizer.v1' -or
    $provider.GetAttribute('Version') -cne $ProductVersion) {
    throw 'Linked Bundle finalizer dependency provider mismatch.'
}

$payloads = @($manifest.SelectNodes('/b:BurnManifest/b:Payload', $burnNamespace))
$machinePayload = $payloads | Where-Object { $_.GetAttribute('Id') -ceq 'MoMachinePackage' }
$finalizerPayload = $payloads | Where-Object { $_.GetAttribute('Id') -ceq 'MoCurrentUserFinalizer' }
if (@($machinePayload).Count -ne 1 -or @($finalizerPayload).Count -ne 1) {
    throw 'Linked Bundle payload identity mismatch.'
}
if ($machinePayload.GetAttribute('Container') -cne 'WixAttachedContainer' -or
    $finalizerPayload.GetAttribute('Container') -cne 'WixAttachedContainer' -or
    $machinePayload.GetAttribute('FilePath') -cne 'mo-development-unsigned.msi' -or
    $finalizerPayload.GetAttribute('FilePath') -cne 'mo-tip-registrar.exe') {
    throw 'Linked Bundle attached-container payload path mismatch.'
}
$attachedContainer = Join-Path $bundlePayload 'WixAttachedContainer'
$extractedMsi = Join-Path $attachedContainer $machinePayload.GetAttribute('FilePath')
$extractedFinalizer = Join-Path $attachedContainer $finalizerPayload.GetAttribute('FilePath')
if (@(Get-ChildItem -LiteralPath $attachedContainer -File).Count -ne 2) {
    throw 'Linked Bundle attached-container file count mismatch.'
}
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $extractedMsi).Hash -cne
        (Get-FileHash -Algorithm SHA256 -LiteralPath $msi).Hash -or
    (Get-FileHash -Algorithm SHA256 -LiteralPath $extractedFinalizer).Hash -cne
        (Get-FileHash -Algorithm SHA256 -LiteralPath $stageRegistrar).Hash) {
    throw 'Linked Bundle embedded payload differs from its verified input.'
}

$iceValidated = $false
if ($ValidateMsi) {
    & $wix msi validate $msi
    if ($LASTEXITCODE -ne 0) { throw 'MSI ICE validation failed.' }
    $iceValidated = $true
}
$evidence = [ordered]@{
    format = 1
    development_only = $true
    install_executed = $false
    wix_version = $version
    known_link_warning = 'WIX1140: per-user Bundle does not register a dependency on its per-machine MSI'
    msi_ice_validated = $iceValidated
    stage_manifest_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $stage 'mo-stage.json')).Hash
    msi_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $msi).Hash
    bundle_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $bundle).Hash
}
$json = $evidence | ConvertTo-Json -Depth 4
[IO.File]::WriteAllText(
    (Join-Path $output 'linked-installer-evidence.json'),
    $json + "`n",
    [Text.UTF8Encoding]::new($false))
Write-Host 'Linked MSI and Burn manifest verification passed without executing either installer.'
if (-not $ValidateMsi) {
    Write-Warning 'MSI ICE validation was not requested; it remains a VM/release gate.'
}
