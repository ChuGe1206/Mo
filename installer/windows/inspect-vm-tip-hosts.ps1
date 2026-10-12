#Requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory)][ValidatePattern('^[a-f0-9]{32}$')][string]$Nonce,
    [Parameter(Mandatory)][string]$OwnerSid, [Parameter(Mandatory)][int]$SessionId)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) -or
    $identity.User.Value -cne $OwnerSid -or (Get-Process -Id $PID).SessionId -ne $SessionId) {
    throw 'Read-only host worker must retain the initiating identity and session.'
}
. (Join-Path $PSScriptRoot 'changed-upgrade-policy.ps1')
$root = Join-Path $env:ProgramData 'MoInstallerTest\HostAudits'
foreach ($path in @((Split-Path -Parent $root), $root)) {
    $null = Assert-MoVmPlainDosPath $path -MayNotExist
    if (Test-Path -LiteralPath $path) { Assert-MoVmHostAuditAcl $path; continue }
    New-Item -ItemType Directory -Path $path | Out-Null
    $acl = [Security.AccessControl.DirectorySecurity]::new()
    $acl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
    $acl.SetAccessRuleProtection($true, $false)
    foreach ($sid in @('S-1-5-32-544', 'S-1-5-18', 'S-1-5-32-545')) {
        $rights = if ($sid -eq 'S-1-5-32-545') { 'ReadAndExecute' } else { 'FullControl' }
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
            [Security.Principal.SecurityIdentifier]::new($sid), $rights,
            'ContainerInherit,ObjectInherit', 'None', 'Allow'))
    }
    Set-Acl -LiteralPath $path -AclObject $acl
    Assert-MoVmHostAuditAcl $path
}
$output = Join-Path $root $Nonce
if (Test-Path -LiteralPath $output) { throw 'Fresh audit nonce already exists.' }
New-Item -ItemType Directory -Path $output | Out-Null
# A child inherits the trusted directory's grants; explicitly retain its owner.
$childAcl = Get-Acl -LiteralPath $output
$childAcl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
Set-Acl -LiteralPath $output -AclObject $childAcl
Assert-MoVmHostAuditAcl $output
$scan = Get-MoVmCurrentUserTipHostScan
$result = [ordered]@{ format = 1; nonce = $Nonce; owner_sid = $OwnerSid; session_id = $SessionId
    checked_utc = [DateTime]::UtcNow.ToString('o'); elevated = $true
    helper_sha256 = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash; scan = $scan }
$result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $output 'audit.json') -Encoding UTF8
$fileAcl = Get-Acl -LiteralPath (Join-Path $output 'audit.json')
$fileAcl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
Set-Acl -LiteralPath (Join-Path $output 'audit.json') -AclObject $fileAcl
Assert-MoVmHostAuditAcl (Join-Path $output 'audit.json')
