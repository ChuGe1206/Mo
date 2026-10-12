#Requires -Version 5.1
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'changed-upgrade-policy.ps1')
$script:testCount = 0
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
try { $script:fixtureOwnerSid = $identity.User.Value } finally { $identity.Dispose() }
$script:fixtureProcessId = 400001
$script:fixtureSessionId = 17

function Run-ScanCase([string]$Mode, [int]$Loaded, [int]$Failed, [int]$Unknown, [bool]$Reject) {
    $script:fixtureMode = $Mode
    $script:moduleQueries = 0
    $script:disposed = 0
    function Get-CimInstance {
        [CmdletBinding()]param([string]$ClassName, [string]$Filter)
        if ($ClassName -cne 'Win32_Process' -or $Filter -cne 'SessionId=17') { throw 'Wrong scan scope.' }
        if ($script:fixtureMode -ceq 'enumeration-error') { throw 'synthetic inventory unavailable' }
        return [pscustomobject]@{ ProcessId = $script:fixtureProcessId }
    }
    function Invoke-CimMethod {
        [CmdletBinding()]param([object]$InputObject, [string]$MethodName)
        if ($InputObject.ProcessId -ne $script:fixtureProcessId -or $MethodName -cne 'GetOwnerSid') {
            throw 'Wrong ownership query.'
        }
        if ($script:fixtureMode -ceq 'owner-error') { throw 'synthetic owner unavailable' }
        $sid = $script:fixtureOwnerSid
        $code = 0
        if ($script:fixtureMode -ceq 'other-owner') { $sid = 'synthetic-other-account' }
        if ($script:fixtureMode -ceq 'unknown-owner') { $sid = $null; $code = 2 }
        return [pscustomobject]@{ Sid = $sid; ReturnValue = $code }
    }
    function Get-Process {
        [CmdletBinding()]param([int]$Id)
        if ($Id -eq $PID) { return [pscustomobject]@{ SessionId = $script:fixtureSessionId } }
        if ($Id -ne $script:fixtureProcessId) { throw 'Unexpected module query.' }
        $script:moduleQueries++
        if ($script:fixtureMode -ceq 'module-error') { throw 'synthetic modules unavailable' }
        $modules = @([pscustomobject]@{ ModuleName = 'unrelated.dll'; ModuleMemorySize = 4096 })
        if ($script:fixtureMode -ceq 'empty-modules') { $modules = @() }
        if ($script:fixtureMode -cin @('loaded-x64', 'loaded-x86')) {
            $modules += [pscustomobject]@{ ModuleName = 'MO-TIP.DLL'; ModuleMemorySize = 8192 }
        }
        $name = if ($script:fixtureMode -ceq 'loaded-x86') { 'synthetic-x86' } else { 'synthetic-x64' }
        $process = [pscustomobject]@{ Id = $Id; ProcessName = $name; Modules = $modules }
        $process | Add-Member -MemberType ScriptMethod -Name Dispose -Value { $script:disposed++ }
        return $process
    }
    if ($Mode -ceq 'enumeration-error') {
        try { Get-MoVmCurrentUserTipHostScan; throw 'Unavailable inventory accepted.' } catch {
            if ($_.Exception.Message -cne 'synthetic inventory unavailable') { throw }
        }
        $script:testCount++
        Write-Host 'PASS enumeration-error'
        return
    }
    $scan = Get-MoVmCurrentUserTipHostScan
    if ($scan.scope -cne 'identified-current-user-current-session' -or
        @($scan.loaded_hosts).Count -ne $Loaded -or @($scan.failed_process_ids).Count -ne $Failed -or
        @($scan.unresolved_owner_process_ids).Count -ne $Unknown) { throw "Wrong scan result: $Mode" }
    $rejected = $false
    try { Assert-MoVmNoObservedTipHosts $scan } catch { $rejected = $true }
    if ($rejected -ne $Reject) { throw "Wrong upgrade refusal: $Mode" }
    if ($Mode -cin @('other-owner', 'unknown-owner', 'owner-error') -and $script:moduleQueries -ne 0) {
        throw 'Modules queried outside confirmed ownership scope.'
    }
    if ($Mode -cin @('clean', 'loaded-x64', 'loaded-x86', 'empty-modules') -and $script:disposed -ne 1) {
        throw 'Process handle not disposed.'
    }
    if ($Loaded -and (@($scan.loaded_hosts[0].PSObject.Properties.Name) -join ',') -cne
        'process_id,process_name,module_memory_bytes') { throw 'Unexpected host telemetry fields.' }
    $script:testCount++
    Write-Host "PASS $Mode"
}
Run-ScanCase 'enumeration-error' 0 0 0 $true
Run-ScanCase 'clean' 0 0 0 $false
Run-ScanCase 'loaded-x64' 1 0 0 $true
Run-ScanCase 'loaded-x86' 1 0 0 $true
Run-ScanCase 'module-error' 0 1 0 $true
Run-ScanCase 'empty-modules' 0 1 0 $true
Run-ScanCase 'other-owner' 0 0 0 $false
Run-ScanCase 'unknown-owner' 0 0 1 $false
Run-ScanCase 'owner-error' 0 0 1 $false
$invalidScope = [pscustomobject]@{ scope = 'synthetic-invalid' }
try { Assert-MoVmNoObservedTipHosts $invalidScope; throw 'Invalid scope accepted.' } catch {
    if ($_.Exception.Message -cne 'Unexpected TIP host scan scope.') { throw }
}
$script:testCount++
Write-Host "TIP host preflight tests passed: $script:testCount. Synthetic metadata only; no processes stopped."
