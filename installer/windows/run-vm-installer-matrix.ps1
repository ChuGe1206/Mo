#Requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$DisposableVm,
    [switch]$AllowInstallerExecution,
    [string]$SentinelPath = (Join-Path $env:LOCALAPPDATA 'MoInstallerTest\disposable-vm.json'),
    [Parameter(Mandatory = $true)][string]$EvidenceDirectory
)

$ErrorActionPreference = 'Stop'
if (-not $DisposableVm -or -not $AllowInstallerExecution) {
    throw 'Refusing to execute installers. Pass both explicit switches inside an initialized disposable VM.'
}
. (Join-Path $PSScriptRoot 'vm-test-policy.ps1')
$sentinel = Assert-MoDisposableVmSentinel $SentinelPath
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Matrix driver must start from a non-elevated user token so Burn can preserve the initiating user.'
}
if (-not [Environment]::Is64BitOperatingSystem -or -not [Environment]::Is64BitProcess) {
    throw 'Matrix driver requires 64-bit PowerShell on 64-bit Windows.'
}
if (-not (Test-MoVmAbsoluteDosPath $EvidenceDirectory) -or
    (Test-Path -LiteralPath $EvidenceDirectory)) {
    throw 'EvidenceDirectory must be an absolute path that does not yet exist.'
}

$kit = Assert-MoVmMatrixTestKit $PSScriptRoot
$baseBundle = Join-Path $PSScriptRoot 'mo-setup-base-unsigned.exe'
$upgradeBundle = Join-Path $PSScriptRoot 'mo-setup-upgrade-unsigned.exe'
$registrar = Join-Path $PSScriptRoot 'mo-tip-registrar.exe'
$stageManifest = Join-Path $PSScriptRoot 'mo-stage.json'
$contract = Get-MoVmPayloadContract $stageManifest
$installRoot = Join-Path $env:ProgramFiles 'Mo'
$userSetupKey = 'Software\Classes\Local Settings\Software\Mo\InputMethod\Setup'
New-Item -ItemType Directory -Path $EvidenceDirectory | Out-Null
$results = [Collections.Generic.List[object]]::new()
$completed = $false
$failure = $null
$missingMarkerRepairFailedClosed = $false

if ($null -eq ('MoVmMsiState' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class MoVmMsiState {
    [DllImport("msi.dll", CharSet = CharSet.Unicode)]
    public static extern int MsiQueryProductState(string productCode);
}
'@
}

function Read-RegistrarState {
    $lines = @(& $registrar status)
    if ($LASTEXITCODE -ne 0) { throw "Registrar status failed: $LASTEXITCODE" }
    return ConvertFrom-MoVmRegistrarStatus $lines
}

function Assert-ProductState([string]$ProductCode, [ValidateSet('Absent', 'Installed')][string]$Expected) {
    $actual = [MoVmMsiState]::MsiQueryProductState($ProductCode)
    $wanted = if ($Expected -eq 'Installed') { 5 } else { -1 }
    if ($actual -ne $wanted) {
        throw "MSI product $ProductCode state mismatch: expected $Expected ($wanted), found $actual."
    }
}

function Assert-CleanState {
    Assert-MoVmLifecycleState (Read-RegistrarState) Clean
    Assert-MoVmMachineComState Absent $installRoot
    if (Test-Path -LiteralPath $installRoot) { throw 'Rollback/uninstall left the Mo installation root.' }
    Assert-ProductState $kit.base_product_code Absent
    Assert-ProductState $kit.upgrade_product_code Absent
}

function Assert-InstalledState([string]$ProductCode, [string]$OtherProductCode) {
    Assert-MoVmLifecycleState (Read-RegistrarState) Installed
    Assert-MoVmMachineComState Installed $installRoot
    Assert-MoVmInstalledPayload $installRoot $contract
    Assert-MoVmInstalledSecurity $installRoot
    Assert-ProductState $ProductCode Installed
    Assert-ProductState $OtherProductCode Absent
}

function Assert-MissingMarkerInstalledState {
    $state = Read-RegistrarState
    $expected = [ordered]@{
        'com.x64' = 'missing'; 'com.x86' = 'missing'
        'profile.registered' = 'true'; 'profile.enabled' = 'true'; 'profile.active' = 'false'
        'user.finalizer' = 'missing'
        'user.finalizer.transaction' = 'mo-user-finalizer-install-v1-disabled'
    }
    foreach ($name in $expected.Keys) {
        if (-not $state.Contains($name) -or $state[$name] -cne $expected[$name]) {
            throw "Missing-marker state mismatch for ${name}: expected '$($expected[$name])'."
        }
    }
    Assert-MoVmMachineComState Installed $installRoot
    Assert-MoVmInstalledPayload $installRoot $contract
    Assert-MoVmInstalledSecurity $installRoot
    Assert-ProductState $kit.base_product_code Installed
    Assert-ProductState $kit.upgrade_product_code Absent
}

function Remove-UserFinalizerMarker {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($userSetupKey, $true)
    if ($null -eq $key) { throw 'Current-user setup key is missing before marker-removal test.' }
    try {
        if ($null -eq $key.GetValue('UserFinalizer', $null)) {
            throw 'Current-user finalizer marker is already missing.'
        }
        $key.DeleteValue('UserFinalizer', $true)
        $key.Flush()
    } finally { $key.Dispose() }
}

function Restore-UserFinalizerMarker {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($userSetupKey, $true)
    if ($null -eq $key) { throw 'Current-user setup key is missing before marker restoration.' }
    try {
        $key.SetValue('UserFinalizer', 'mo-user-finalizer-v1', [Microsoft.Win32.RegistryValueKind]::String)
        $key.Flush()
    } finally { $key.Dispose() }
}

function Invoke-BundlePhase(
    [string]$Bundle,
    [string]$Label,
    [string]$Action,
    [string[]]$ExtraArguments,
    [bool]$ExpectFailure
) {
    $log = Join-Path $EvidenceDirectory ("bundle-{0}.log" -f $Label)
    $arguments = @($ExtraArguments) + @(("/{0}" -f $Action), '/quiet', '/norestart', '/log', $log)
    & $Bundle @arguments
    $exitCode = $LASTEXITCODE
    $results.Add([pscustomobject]@{
        label = $Label
        action = $Action
        expected_failure = $ExpectFailure
        exit_code = $exitCode
        log = [IO.Path]::GetFileName($log)
    })
    if ($ExpectFailure) {
        if ($exitCode -in @(0, 1641, 3010)) {
            throw "Bundle $Label did not produce the required hard failure; exit code: $exitCode"
        }
    } elseif ($exitCode -ne 0) {
        throw "Bundle $Label failed or requested reboot; exit code: $exitCode"
    }
}

try {
    if (Test-Path -LiteralPath $installRoot) { throw "Clean VM already contains $installRoot" }
    Assert-CleanState

    Invoke-BundlePhase $baseBundle 'fault-machine' 'install' @(
        'MoTestFailAfterMachineProfile=1') $true
    Assert-CleanState

    Invoke-BundlePhase $baseBundle 'fault-user' 'install' @(
        'MoTestFailAfterUserFinalizer=1') $true
    Assert-CleanState

    Invoke-BundlePhase $baseBundle 'base-install' 'install' @() $false
    Assert-InstalledState $kit.base_product_code $kit.upgrade_product_code

    Remove-UserFinalizerMarker
    Assert-MissingMarkerInstalledState
    Invoke-BundlePhase $baseBundle 'missing-marker-repair' 'repair' @() $true
    Assert-MissingMarkerInstalledState
    $missingMarkerRepairFailedClosed = $true
    Restore-UserFinalizerMarker
    Assert-InstalledState $kit.base_product_code $kit.upgrade_product_code

    Invoke-BundlePhase $upgradeBundle 'major-upgrade' 'install' @() $false
    Assert-InstalledState $kit.upgrade_product_code $kit.base_product_code

    Invoke-BundlePhase $upgradeBundle 'upgrade-uninstall' 'uninstall' @() $false
    Assert-MoVmLifecycleState (Read-RegistrarState) Uninstalled
    Assert-MoVmMachineComState Absent $installRoot
    if (Test-Path -LiteralPath $installRoot) { throw 'Upgrade uninstall left the Mo installation root.' }
    Assert-ProductState $kit.base_product_code Absent
    Assert-ProductState $kit.upgrade_product_code Absent
    $completed = $true
} catch {
    $failure = $_.Exception.Message
    throw
} finally {
    $finalState = $null
    try { $finalState = Read-RegistrarState } catch { $finalState = @{ error = $_.Exception.Message } }
    $productStates = [ordered]@{
        base = [MoVmMsiState]::MsiQueryProductState($kit.base_product_code)
        upgrade = [MoVmMsiState]::MsiQueryProductState($kit.upgrade_product_code)
    }
    $logs = [ordered]@{}
    foreach ($file in @(Get-ChildItem -LiteralPath $EvidenceDirectory -File -ErrorAction SilentlyContinue)) {
        $logs[$file.Name] = [ordered]@{
            size = $file.Length
            sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
        }
    }
    $evidence = [ordered]@{
        format = 2
        kind = 'mo-installer-vm-rollback-upgrade-matrix-evidence'
        development_only = $true
        completed = $completed
        machine_failure_rolled_back = if ($completed) { $true } else { $null }
        user_failure_rolled_back = if ($completed) { $true } else { $null }
        missing_marker_repair_failed_closed = $missingMarkerRepairFailedClosed
        major_upgrade_completed = if ($completed) { $true } else { $null }
        default_input_unchanged = if ($completed) { $true } else { $null }
        install_tree_security_audited = if ($completed) { $true } else { $null }
        vm_id = $sentinel.vm_id
        computer_name = [Environment]::MachineName
        os_version = [Environment]::OSVersion.Version.ToString()
        base_bundle_sha256 = (Get-FileHash -LiteralPath $baseBundle -Algorithm SHA256).Hash
        upgrade_bundle_sha256 = (Get-FileHash -LiteralPath $upgradeBundle -Algorithm SHA256).Hash
        stage_manifest_sha256 = (Get-FileHash -LiteralPath $stageManifest -Algorithm SHA256).Hash
        phases = @($results)
        final_state = $finalState
        final_product_states = $productStates
        failure = $failure
        logs = $logs
    }
    $evidence | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (
        Join-Path $EvidenceDirectory 'vm-rollback-upgrade-matrix-evidence.json') -Encoding UTF8
}

Write-Host 'Disposable-VM rollback, fail-closed repair, major-upgrade and uninstall matrix passed.'
Write-Warning 'Revert or delete the VM. The expected remove rollback receipt remains in HKCU until a later install overwrites it.'
