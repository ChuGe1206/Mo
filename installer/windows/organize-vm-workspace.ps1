#Requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$DisposableVm,
    [switch]$Apply,
    [string]$SentinelPath = (Join-Path $env:LOCALAPPDATA 'MoInstallerTest/disposable-vm.json')
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'vm-workspace-policy.ps1')
. (Join-Path $PSScriptRoot 'vm-test-policy.ps1')
if (-not $DisposableVm) { throw 'Use this organizer only in the Mo disposable test VM.' }
$null = Assert-MoDisposableVmSentinel $SentinelPath
$sourceRoot = $env:SystemDrive + '\'
$workspace = Join-Path $sourceRoot 'MoDev'
$plan = @(Get-MoWorkspacePlan $sourceRoot $workspace)
$plan | Format-Table source, destination, state -AutoSize
if (-not $Apply) {
    Write-Host 'Preview only. Add -Apply after checking the listed paths.'
    return
}
$journal = Invoke-MoWorkspaceMigration $sourceRoot $workspace $plan
Write-Host "Organized $($plan.Count) legacy folders under $workspace. No deletion performed."
$share = '\\vboxsvr\MoEvidence\WorkspaceGuest-v1'
try {
    if (Test-Path -LiteralPath '\\vboxsvr\MoEvidence') {
        New-Item -ItemType Directory -Path $share -Force | Out-Null
        Copy-Item -LiteralPath $journal -Destination (Join-Path $share (Split-Path -Leaf $journal)) -ErrorAction Stop
        Write-Host "Receipt returned to shared evidence: $share"
    }
} catch { Write-Warning "Migration finished; shared receipt copy failed: $($_.Exception.Message)" }
Write-Host 'Keep installed binaries in Program Files/Mo and user settings in LocalAppData/Mo.'
