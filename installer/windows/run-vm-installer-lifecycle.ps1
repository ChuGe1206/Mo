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
    throw 'Refusing to execute an installer. Pass both explicit switches inside an initialized disposable VM.'
}
. (Join-Path $PSScriptRoot 'vm-test-policy.ps1')
$sentinel = Assert-MoDisposableVmSentinel $SentinelPath
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Lifecycle driver must start from a non-elevated user token so Burn can preserve the initiating user across elevation.'
}
if (-not [Environment]::Is64BitOperatingSystem -or -not [Environment]::Is64BitProcess) {
    throw 'Lifecycle driver requires 64-bit PowerShell on 64-bit Windows.'
}
if (-not (Test-MoVmAbsoluteDosPath $EvidenceDirectory) -or
    (Test-Path -LiteralPath $EvidenceDirectory)) {
    throw 'EvidenceDirectory must be an absolute path that does not yet exist.'
}

$bundle = Join-Path $PSScriptRoot 'mo-setup-development-unsigned.exe'
$registrar = Join-Path $PSScriptRoot 'mo-tip-registrar.exe'
$stageManifest = Join-Path $PSScriptRoot 'mo-stage.json'
$kit = Assert-MoVmTestKit $PSScriptRoot

$contract = Get-MoVmPayloadContract $stageManifest
$installRoot = Join-Path $env:ProgramFiles 'Mo'
New-Item -ItemType Directory -Path $EvidenceDirectory | Out-Null
$results = [Collections.Generic.List[object]]::new()
$completed = $false
$failure = $null

function Read-RegistrarState {
    $lines = @(& $registrar status)
    if ($LASTEXITCODE -ne 0) { throw "Registrar status failed: $LASTEXITCODE" }
    return ConvertFrom-MoVmRegistrarStatus $lines
}

function Invoke-BundlePhase([string]$Action) {
    $log = Join-Path $EvidenceDirectory ("bundle-{0}.log" -f $Action.ToLowerInvariant())
    # Burn is a GUI-subsystem process. Direct invocation can return before UAC completes.
    $arguments = @(("/$Action"), '/quiet', '/norestart', '/log', ('"{0}"' -f $log))
    $process = Start-Process -FilePath $bundle -ArgumentList $arguments -Wait -PassThru
    $exitCode = $process.ExitCode
    $results.Add([pscustomobject]@{
        action = $Action
        exit_code = $exitCode
        log = [IO.Path]::GetFileName($log)
    })
    if ($exitCode -ne 0) {
        throw "Bundle $Action failed or requested reboot; exit code: $exitCode"
    }
}

try {
    if (Test-Path -LiteralPath $installRoot) { throw "Clean VM already contains $installRoot" }
    Assert-MoVmMachineComState Absent $installRoot
    Assert-MoVmLifecycleState (Read-RegistrarState) Clean

    Invoke-BundlePhase Install
    Assert-MoVmLifecycleState (Read-RegistrarState) Installed
    Assert-MoVmMachineComState Installed $installRoot
    Assert-MoVmInstalledPayload $installRoot $contract
    Assert-MoVmInstalledSecurity $installRoot

    Invoke-BundlePhase Repair
    Assert-MoVmLifecycleState (Read-RegistrarState) Repaired
    Assert-MoVmMachineComState Installed $installRoot
    Assert-MoVmInstalledPayload $installRoot $contract
    Assert-MoVmInstalledSecurity $installRoot

    Invoke-BundlePhase Uninstall
    Assert-MoVmLifecycleState (Read-RegistrarState) Uninstalled
    Assert-MoVmMachineComState Absent $installRoot
    if (Test-Path -LiteralPath $installRoot) { throw 'Uninstall left the Mo installation root.' }
    $completed = $true
} catch {
    $failure = $_.Exception.Message
    throw
} finally {
    $finalState = $null
    try { $finalState = Read-RegistrarState } catch { $finalState = @{ error = $_.Exception.Message } }
    $logs = [ordered]@{}
    foreach ($file in @(Get-ChildItem -LiteralPath $EvidenceDirectory -File -ErrorAction SilentlyContinue)) {
        $logs[$file.Name] = [ordered]@{
            size = $file.Length
            sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
        }
    }
    $evidence = [ordered]@{
        format = 2
        kind = 'mo-installer-vm-lifecycle-evidence'
        development_only = $true
        completed = $completed
        install_default_unchanged = if ($completed) { $true } else { $null }
        install_tree_security_audited = if ($completed) { $true } else { $null }
        vm_id = $sentinel.vm_id
        computer_name = [Environment]::MachineName
        os_version = [Environment]::OSVersion.Version.ToString()
        bundle_sha256 = (Get-FileHash -LiteralPath $bundle -Algorithm SHA256).Hash
        stage_manifest_sha256 = (Get-FileHash -LiteralPath $stageManifest -Algorithm SHA256).Hash
        phases = @($results)
        final_state = $finalState
        failure = $failure
        logs = $logs
    }
    $evidence | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (
        Join-Path $EvidenceDirectory 'vm-lifecycle-evidence.json') -Encoding UTF8
}

Write-Host 'Disposable-VM install, repair and uninstall lifecycle passed. Mo was enabled for the test user without becoming active/default, then removed.'
Write-Warning 'Revert or delete the VM. The expected remove rollback receipt remains in HKCU until a later install overwrites it.'
