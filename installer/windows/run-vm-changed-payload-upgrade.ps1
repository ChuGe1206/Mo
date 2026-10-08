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
    throw 'Refusing changed-payload upgrade. Pass both switches inside an initialized disposable VM.'
}
. (Join-Path $PSScriptRoot 'changed-upgrade-policy.ps1')
$sentinel = Assert-MoDisposableVmSentinel $SentinelPath
$principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Upgrade driver requires a non-elevated initiating user.'
}
if (-not [Environment]::Is64BitOperatingSystem -or -not [Environment]::Is64BitProcess) {
    throw 'Upgrade driver requires 64-bit PowerShell on 64-bit Windows.'
}
$evidence = Assert-MoVmPlainDosPath $EvidenceDirectory -MayNotExist
if (Test-Path -LiteralPath $evidence) { throw 'Evidence directory must not already exist.' }
$kit = Assert-MoVmChangedUpgradeKit $PSScriptRoot
$baseContract = Get-MoVmPayloadContract (Join-Path $PSScriptRoot 'base-stage.json')
$upgradeContract = Get-MoVmPayloadContract (Join-Path $PSScriptRoot 'upgrade-stage.json')
$installRoot = Join-Path $env:ProgramFiles 'Mo'
$settingsPath = Join-Path $env:LOCALAPPDATA 'Mo\Profile\settings-v1.mo'
$registrar = Join-Path $PSScriptRoot 'mo-tip-registrar.exe'
$bundle = Join-Path $PSScriptRoot 'mo-setup-upgrade-unsigned.exe'
if ($null -eq ('MoChangedUpgradeMsiState' -as [type])) {
    Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public static class MoChangedUpgradeMsiState { [DllImport("msi.dll", CharSet=CharSet.Unicode)] public static extern int MsiQueryProductState(string code); }'
}
function Read-RegistrarState {
    $lines = @(& $registrar status)
    if ($LASTEXITCODE -ne 0) { throw 'Upgrade registrar status failed.' }
    return ConvertFrom-MoVmRegistrarStatus $lines
}
function Assert-ProductState([string]$Code, [int]$Expected) {
    if ([MoChangedUpgradeMsiState]::MsiQueryProductState($Code) -ne $Expected) {
        throw 'Unexpected changed-upgrade MSI product state.'
    }
}

New-Item -ItemType Directory -Path $evidence | Out-Null
$result = [ordered]@{
    format = 1; kind = 'mo-vm-changed-payload-upgrade-evidence'; development_only = $true
    vm_id = $sentinel.vm_id; completed = $false; installer_invocation_attempted = $false; exit_code = $null
    base_version = $kit.base_version; upgrade_version = $kit.upgrade_version
    base_bundle_sha256 = $kit.base_bundle_sha256; upgrade_bundle_sha256 = $kit.upgrade_bundle_sha256
    base_stage_manifest_sha256 = $kit.base_stage_manifest_sha256
    upgrade_stage_manifest_sha256 = $kit.upgrade_stage_manifest_sha256
    kit_manifest_sha256 = (Get-FileHash (Join-Path $PSScriptRoot 'vm-changed-upgrade-kit.json')).Hash
    base_payload_verified = $false; upgrade_payload_verified = $false
    install_tree_security_audited = $false; settings_preserved = $false; default_override_unchanged = $false
    loaded_tip_matrix_executed = $false; desktop_input_executed = $false; reboot_executed = $false
    failure = $null; final_state = $null; logs = $null
}
try {
    if (@(Get-Process -Name 'mo-broker', 'mo-settings' -ErrorAction SilentlyContinue).Count) {
        throw 'Close Mo Broker and settings before this unloaded-process upgrade test.'
    }
    if (Test-Path -LiteralPath ($settingsPath + '.write-lock')) {
        throw 'Settings writer marker exists; finish or review that writer before upgrading.'
    }
    Assert-ProductState $kit.base_product_code 5
    Assert-ProductState $kit.upgrade_product_code -1
    Assert-MoVmLifecycleState (Read-RegistrarState) Installed
    Assert-MoVmMachineComState Installed $installRoot
    Assert-MoVmInstalledPayload $installRoot $baseContract
    Assert-MoVmInstalledSecurity $installRoot
    $result.base_payload_verified = $true
    $beforeSettings = Get-MoVmSettingsFingerprint $settingsPath
    $beforeDefault = (Get-WinDefaultInputMethodOverride | Out-String).Trim()
    # Burn retains the non-elevated initiating user; the VM user handles UAC.
    $arguments = @('/install', '/quiet', '/norestart', '/log', ('"{0}"' -f (Join-Path $evidence 'upgrade.log')))
    $result.installer_invocation_attempted = $true
    $process = Start-Process -FilePath $bundle -ArgumentList $arguments -Wait -PassThru
    $result.exit_code = $process.ExitCode
    if ($process.ExitCode -ne 0) { throw "Upgrade failed or requested reboot: $($process.ExitCode)" }
    Assert-ProductState $kit.base_product_code -1
    Assert-ProductState $kit.upgrade_product_code 5
    Assert-MoVmLifecycleState (Read-RegistrarState) Installed
    Assert-MoVmMachineComState Installed $installRoot
    Assert-MoVmInstalledPayload $installRoot $upgradeContract
    $result.upgrade_payload_verified = $true
    Assert-MoVmInstalledSecurity $installRoot
    $result.install_tree_security_audited = $true
    Assert-MoVmSettingsPreserved $beforeSettings (Get-MoVmSettingsFingerprint $settingsPath)
    $result.settings_preserved = $true
    if ((Get-WinDefaultInputMethodOverride | Out-String).Trim() -cne $beforeDefault) {
        throw 'Upgrade changed the default input method override.'
    }
    $result.default_override_unchanged = $true
    $result.completed = $true
} catch {
    $result.failure = $_.Exception.Message
    throw
} finally {
    try { $result.final_state = Read-RegistrarState } catch { $result.final_state = @{ error = $_.Exception.Message } }
    $logs = [ordered]@{}
    foreach ($file in @(Get-ChildItem -LiteralPath $evidence -File -Force)) {
        $logs[$file.Name] = [ordered]@{ size = $file.Length; sha256 = (Get-FileHash $file.FullName).Hash }
    }
    $result.logs = $logs
    $result | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $evidence 'upgrade-state.json') -Encoding UTF8
}
Write-Host 'Changed-payload upgrade verified. Desktop, loaded TIP, rollback and reboot acceptance remain separate.'
