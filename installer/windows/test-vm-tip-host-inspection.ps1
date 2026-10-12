#Requires -Version 5.1
[CmdletBinding()]param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'changed-upgrade-policy.ps1')
$script:count = 0
$now = [DateTime]::Parse('2026-01-01T12:00:00Z').ToUniversalTime()
$nonce = 'a' * 32; $owner = 'S-1-5-21-1-2-3-1001'; $session = 17; $hash = 'B' * 64
function Fresh {
    return [pscustomobject]@{ format = 1; nonce = $nonce; owner_sid = $owner; session_id = $session
        checked_utc = $now.ToString('o'); elevated = $true; helper_sha256 = $hash
        scan = [pscustomobject]@{ scope = 'identified-current-user-current-session'; loaded_hosts = @()
            failed_process_ids = @(); unresolved_owner_process_ids = @(); protected_hosts = @() } }
}
function Pass([string]$Label, [object]$Report) {
    Assert-MoVmHostAuditReport $Report $nonce $owner $session $hash $now
    $script:count++; Write-Host "PASS $Label"
}
function Reject([string]$Label, [object]$Report) {
    $rejected = $false
    try { Assert-MoVmHostAuditReport $Report $nonce $owner $session $hash $now } catch { $rejected = $true }
    if (-not $rejected) { throw "Accepted $Label" }
    $script:count++; Write-Host "PASS $Label rejected"
}
Pass 'fresh' (Fresh)
$r = Fresh; $r.checked_utc = $now.AddSeconds(-30).ToString('o'); Pass 'freshness-boundary' $r
$r = Fresh; $r.checked_utc = $now.AddSeconds(-31).ToString('o'); Reject 'stale' $r
$r = Fresh; $r.checked_utc = $now.AddSeconds(2).ToString('o'); Pass 'future-tolerance' $r
$r = Fresh; $r.checked_utc = $now.AddSeconds(3).ToString('o'); Reject 'future' $r
$r = Fresh; $r.nonce = 'c' * 32; Reject 'replayed-nonce' $r
$r = Fresh; $r.owner_sid = 'S-1-5-21-1-2-3-1002'; Reject 'other-owner' $r
$r = Fresh; $r.session_id = 18; Reject 'other-session' $r
$r = Fresh; $r.helper_sha256 = 'C' * 64; Reject 'other-helper' $r
$r = Fresh; $r.elevated = $false; Reject 'non-elevated' $r
$r = Fresh; $r.elevated = 'true'; Reject 'wrong-bool-type' $r
$r = Fresh; $r.format = 99; Reject 'future-schema' $r
$r = Fresh; $r | Add-Member NoteProperty extra 'synthetic'; Reject 'extra-field' $r
$r = Fresh; $r.checked_utc = 'invalid'; Reject 'invalid-time' $r
$r = Fresh; $r.scan.loaded_hosts = @([pscustomobject]@{process_id=400001;process_name='synthetic';module_memory_bytes=8192}); Reject 'loaded-tip' $r
$r = Fresh; $r.scan.failed_process_ids = @(400001); Reject 'unresolved-module' $r
$r = Fresh; $r.scan.scope = 'wrong'; Reject 'wrong-scope' $r
$r = Fresh; $r.scan.protected_hosts = @([pscustomobject]@{process_id=400001;protection_level=3;installed_tips_unsigned=$true}); Pass 'os-verified-antimalware' $r
$r = Fresh; $r.scan.protected_hosts = @([pscustomobject]@{process_id=400001;protection_level=2;installed_tips_unsigned=$true}); Reject 'other-protection' $r
$r = Fresh; $r.scan.protected_hosts = @([pscustomobject]@{process_id=400001;protection_level=3;installed_tips_unsigned=$false}); Reject 'signed-tip' $r
function Run-AclCase([string]$Label, [string]$Owner, [bool]$UserWrite, [bool]$ShouldReject) {
    function Assert-MoVmPlainDosPath { param([string]$Path) return $Path }
    function Get-Acl {
        param([string]$LiteralPath)
        $acl = [pscustomobject]@{ Access = @(
            [pscustomobject]@{IdentityReference=[Security.Principal.SecurityIdentifier]::new('S-1-5-32-544');FileSystemRights=[Security.AccessControl.FileSystemRights]::FullControl;AccessControlType='Allow'},
            [pscustomobject]@{IdentityReference=[Security.Principal.SecurityIdentifier]::new('S-1-5-32-545');FileSystemRights=$(if($UserWrite){[Security.AccessControl.FileSystemRights]::Modify}else{[Security.AccessControl.FileSystemRights]::ReadAndExecute});AccessControlType='Allow'}) }
        $acl | Add-Member NoteProperty OwnerSid $Owner
        $acl | Add-Member ScriptMethod GetOwner { param($type) return [Security.Principal.SecurityIdentifier]::new($this.OwnerSid) }
        return $acl
    }
    $rejected=$false
    try { Assert-MoVmHostAuditAcl 'C:\synthetic-audit' } catch { $rejected=$true }
    if($rejected -ne $ShouldReject){throw "Wrong audit ACL result: $Label"}
    $script:count++; Write-Host "PASS $Label"
}
Run-AclCase 'protected-audit' 'S-1-5-32-544' $false $false
Run-AclCase 'caller-owned-audit' 'S-1-5-21-1-2-3-1001' $false $true
Run-AclCase 'user-writable-audit' 'S-1-5-32-544' $true $true
function Run-LaunchAdapter([string]$Mode, [bool]$ShouldReject) {
    $script:adapterCopied = $false
    function Assert-MoVmPlainDosPath { param([string]$Path) return $Path }
    function Assert-MoVmHostAuditAcl { param([string]$Path) }
    function Get-FileHash { param([string]$LiteralPath, [string]$Algorithm) return [pscustomobject]@{Hash=('B'*64)} }
    function Get-Process { param([int]$Id) return [pscustomobject]@{SessionId=17} }
    function Get-Item { param([string]$LiteralPath) return [pscustomobject]@{Length=256} }
    function Start-Process {
        [CmdletBinding()]param([string]$FilePath,[string[]]$ArgumentList,[string]$Verb,[string]$WindowStyle,[switch]$PassThru)
        if ($FilePath -cne (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe') -or
            $Verb -cne 'RunAs' -or $WindowStyle -cne 'Hidden' -or -not $PassThru -or
            $ArgumentList.Count -ne 12 -or $ArgumentList[5] -cne '"C:\synthetic kit\inspect-vm-tip-hosts.ps1"') {
            throw 'Incorrect read-only elevated worker invocation.'
        }
        if ($Mode -eq 'cancelled') { throw 'synthetic UAC cancelled' }
        $script:adapterReport = [pscustomobject]@{format=1;nonce=$ArgumentList[7];owner_sid=$ArgumentList[9];session_id=17
            checked_utc=[DateTime]::UtcNow.ToString('o');elevated=$true;helper_sha256=('B'*64)
            scan=[pscustomobject]@{scope='identified-current-user-current-session';loaded_hosts=@();failed_process_ids=@();unresolved_owner_process_ids=@();protected_hosts=@()} }
        $process=[pscustomobject]@{Done=($Mode -ne 'timeout')}
        $process|Add-Member ScriptMethod WaitForExit {param([int]$Milliseconds) if($Milliseconds -ne 120000){throw 'Wrong timeout'};return $this.Done}
        return $process
    }
    function Get-Content { param([string]$LiteralPath,[switch]$Raw) return $script:adapterReport|ConvertTo-Json -Depth 8 }
    function Copy-Item {param([string]$LiteralPath,[string]$Destination) $script:adapterCopied=$true}
    $rejected=$false
    try { $null=Invoke-MoVmElevatedHostInspection 'C:\synthetic kit\inspect-vm-tip-hosts.ps1' 'C:\synthetic evidence' } catch {
        if($_.Exception.Message -eq 'Incorrect read-only elevated worker invocation.'){throw}
        $rejected=$true
    }
    if($rejected -ne $ShouldReject -or $script:adapterCopied -eq $ShouldReject){throw "Wrong worker adapter result: $Mode"}
    $script:count++;Write-Host "PASS worker adapter $Mode"
}
Run-LaunchAdapter 'success' $false
Run-LaunchAdapter 'timeout' $true
Run-LaunchAdapter 'cancelled' $true
Write-Host "Read-only host inspection policy tests passed: $script:count."
