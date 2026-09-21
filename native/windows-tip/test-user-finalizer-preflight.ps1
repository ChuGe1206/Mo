[CmdletBinding()]
param(
    [string]$RegistrarPath = (Join-Path $PSScriptRoot 'out\msbuild\x64\Release\mo_tip_registrar.exe')
)

$ErrorActionPreference = 'Stop'
. (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'tools\registered-tip-state.ps1')

if (-not (Test-Path -LiteralPath $RegistrarPath -PathType Leaf)) {
    throw "Registrar was not found: $RegistrarPath. Run build-probe.ps1 first."
}

function Read-State {
    $lines = @(& $RegistrarPath status)
    if ($LASTEXITCODE -ne 0) { throw "Registrar status failed: $LASTEXITCODE" }
    return ConvertFrom-MoRegistrarStatus $lines
}

$before = Read-State
if ($before['profile.registered'] -ne 'false' -or
    $before['profile.enabled'] -ne 'false' -or
    $before['profile.active'] -ne 'false' -or
    $before['user.finalizer'] -ne 'missing' -or
    $before['user.finalizer.transaction'] -ne 'missing') {
    throw 'Negative finalizer preflight requires an unregistered, disabled profile with no production marker or undo journal; refusing to mutate or clean existing state.'
}

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
$isElevatedAdministrator = $principal.IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
$expectedStandardUserFailures = [ordered]@{
    'install-current-user-fixed' = '80070002'
    'repair-current-user-fixed' = '80070002'
    'remove-current-user-fixed' = '8007064e'
    'rollback-install-current-user-fixed' = '8007139f'
    'rollback-remove-current-user-fixed' = '80070002'
}
foreach ($command in $expectedStandardUserFailures.Keys) {
    $output = @(& $RegistrarPath $command 2>&1 | ForEach-Object { "$_" })
    if ($LASTEXITCODE -eq 0) { throw "Finalizer command unexpectedly succeeded without machine prerequisites: $command" }
    $expected = if ($isElevatedAdministrator) { '80070005' } else { $expectedStandardUserFailures[$command] }
    if (($output -join "`n") -cnotmatch "(?m)^Operation failed: 0x${expected}$") {
        throw "Finalizer command failed at an unexpected gate: $command. Expected HRESULT 0x$expected; output: $($output -join ' | ')"
    }
    $after = Read-State
    foreach ($name in $before.Keys) {
        if ($after[$name] -cne $before[$name]) {
            throw "Rejected finalizer command changed ${name}: $command"
        }
    }
}

$mode = if ($isElevatedAdministrator) { 'elevated-token boundary' } else { 'missing machine/marker prerequisites' }
Write-Host "Current-user finalizer fixed commands rejected the $mode at the expected HRESULTs with no state change."
