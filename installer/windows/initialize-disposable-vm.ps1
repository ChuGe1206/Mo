#Requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$DisposableVm,
    [switch]$AllowDestructiveInstallerTests,
    [string]$SentinelPath = (Join-Path $env:LOCALAPPDATA 'MoInstallerTest\disposable-vm.json')
)

$ErrorActionPreference = 'Stop'
if (-not $DisposableVm -or -not $AllowDestructiveInstallerTests) {
    throw 'Refusing to authorize installer execution. Pass both explicit disposable-VM switches inside a throwaway VM.'
}
. (Join-Path $PSScriptRoot 'vm-test-policy.ps1')
if (Test-Path -LiteralPath $SentinelPath) { throw 'Disposable VM sentinel already exists.' }
$identity = Get-MoVmMachineIdentity
if (("$($identity.manufacturer) $($identity.model)") -notmatch '(?i)virtual|vmware|virtualbox|hyper-v|kvm|sandbox') {
    throw "Machine does not identify as virtual; refusing to create sentinel: $($identity.manufacturer) / $($identity.model)"
}
$parent = Split-Path -Parent $SentinelPath
if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent | Out-Null }
$sentinel = [ordered]@{
    format = 1
    purpose = 'mo-destructive-installer-test-vm'
    vm_id = [Guid]::NewGuid().ToString('N')
    created_utc = [DateTime]::UtcNow.ToString('o')
    computer_name = $identity.computer_name
    user_sid = $identity.user_sid
    machine_guid = $identity.machine_guid
    system_drive = $identity.system_drive
    system_drive_serial = $identity.system_drive_serial
    manufacturer = $identity.manufacturer
    model = $identity.model
}
$sentinel | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $SentinelPath -Encoding UTF8
$null = Assert-MoDisposableVmSentinel $SentinelPath
Write-Host "Disposable VM sentinel created: $SentinelPath"
Write-Warning 'This authorizes destructive Mo installer tests on this VM only. Revert or delete the VM after collecting evidence.'
