[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet('Register', 'Unregister')]
    [string]$Action
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Machine TSF profile/category mutation requires an elevated PowerShell.'
}

$binaryDirectory = Join-Path $repoRoot 'native\windows-tip\out\msbuild\x64\Release'
$registrar = Join-Path $binaryDirectory 'mo_tip_registrar.exe'
$tip = Join-Path $binaryDirectory 'mo_tip.dll'
foreach ($artifact in @($registrar, $tip)) {
    if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) {
        throw "Missing machine profile artifact: $artifact. Run native\windows-tip\build-probe.ps1 first."
    }
}

$status = (& $registrar status | Out-String)
if ($LASTEXITCODE -ne 0) { throw "Registrar status failed: $LASTEXITCODE" }
$statusLines = @($status -split "\r?\n" | ForEach-Object { $_.Trim() } | Where-Object { $_ })

if ($Action -eq 'Register') {
    if ($statusLines -notcontains 'profile.registered=false') {
        throw "Refusing to replace an existing Mo machine profile.`n$status"
    }
    & $registrar register-machine-profile $tip
    if ($LASTEXITCODE -ne 0) { throw "Machine profile registration failed: $LASTEXITCODE" }
    Write-Host 'Mo machine TSF profile/category registered, but not enabled or made default.'
    Write-Host 'Return to a normal PowerShell and run tools\tip-broker-smoke.ps1 -Registered.'
    Write-Host 'Always finish with this elevated cleanup: tools\machine-profile.ps1 -Action Unregister'
} else {
    if ($statusLines -contains 'profile.registered=false') {
        Write-Host 'Mo machine TSF profile is already absent; no mutation was needed.'
    } else {
        & $registrar unregister-machine-profile
        if ($LASTEXITCODE -ne 0) { throw "Machine profile unregister failed: $LASTEXITCODE" }
        Write-Host 'Mo machine TSF profile/category unregistered.'
    }
}

$finalStatus = (& $registrar status | Out-String)
if ($LASTEXITCODE -ne 0) { throw "Final registrar status failed: $LASTEXITCODE" }
$finalStatusLines = @($finalStatus -split "\r?\n" | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$expectedProfileState = if ($Action -eq 'Register') { 'profile.registered=true' } else { 'profile.registered=false' }
if ($finalStatusLines -notcontains $expectedProfileState) {
    throw "Machine profile postcondition failed; expected $expectedProfileState.`n$finalStatus"
}
Write-Host $finalStatus.TrimEnd()
