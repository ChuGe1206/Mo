# Read-only secondary host inspection for disposable VM upgrade tests.
$script:MoHostAuditFreshnessSeconds = 30
$script:MoHostAuditFutureToleranceSeconds = 2
$script:MoHostAuditMaxBytes = 262144
# Bounds completion after UAC returns; user prompt time is managed separately.
$script:MoHostAuditWorkerCompletionTimeoutMilliseconds = 120000
$script:MoAntimalwareProtectionLevel = [uint32]3

function Get-MoVmHostProtectionLevel([int]$ProcessId) {
    if ($null -eq ('MoVmHostProtection' -as [type])) {
        Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class MoVmHostProtection {
    public const uint QueryLimitedInformation = 0x1000;
    public const int ProtectionLevelInfo = 7;
    [DllImport("kernel32.dll", SetLastError=true)] public static extern IntPtr OpenProcess(uint rights, bool inherit, int pid);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern bool GetProcessInformation(IntPtr process, int kind, out uint level, uint size);
    [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
}
"@
    }
    $handle = [MoVmHostProtection]::OpenProcess([MoVmHostProtection]::QueryLimitedInformation, $false, $ProcessId)
    if ($handle -eq [IntPtr]::Zero) { throw 'Cannot query host protection level.' }
    try {
        $level = [uint32]0
        if (-not [MoVmHostProtection]::GetProcessInformation($handle, [MoVmHostProtection]::ProtectionLevelInfo, [ref]$level, [Runtime.InteropServices.Marshal]::SizeOf([uint32]0))) {
            throw 'Host protection query failed.'
        }
        return $level
    } finally { [void][MoVmHostProtection]::CloseHandle($handle) }
}

function Test-MoVmInstalledTipsUnsigned {
    foreach ($architecture in @('x64', 'x86')) {
        $path = Join-Path $env:ProgramFiles ("Mo\tip\{0}\mo-tip.dll" -f $architecture)
        $null = Assert-MoVmPlainDosPath $path
        if ((Get-AuthenticodeSignature -LiteralPath $path).Status -ne 'NotSigned') { return $false }
    }
    return $true
}

function Assert-MoVmHostAuditAcl([string]$Path) {
    $null = Assert-MoVmPlainDosPath $Path
    $acl = Get-Acl -LiteralPath $Path
    $administrators = 'S-1-5-32-544'; $system = 'S-1-5-18'
    $owner = $acl.GetOwner([Security.Principal.SecurityIdentifier]).Value
    if ($owner -notin @($administrators, $system)) { throw 'Host audit has an untrusted owner.' }
    $writeMask = [Security.AccessControl.FileSystemRights]::Write -bor [Security.AccessControl.FileSystemRights]::Delete -bor
        [Security.AccessControl.FileSystemRights]::DeleteSubdirectoriesAndFiles -bor
        [Security.AccessControl.FileSystemRights]::ChangePermissions -bor [Security.AccessControl.FileSystemRights]::TakeOwnership
    foreach ($rule in $acl.Access) {
        $sid = $rule.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value
        if ($rule.AccessControlType -eq 'Allow' -and ($rule.FileSystemRights -band $writeMask) -and
            $sid -notin @($administrators, $system)) { throw 'Host audit is writable by an untrusted principal.' }
    }
}

function Assert-MoVmHostAuditReport([object]$Report, [string]$Nonce, [string]$OwnerSid,
    [int]$SessionId, [string]$HelperHash, [DateTime]$NowUtc = [DateTime]::UtcNow) {
    $names = @('format', 'nonce', 'owner_sid', 'session_id', 'checked_utc', 'elevated', 'helper_sha256', 'scan')
    $actual = @($Report.PSObject.Properties.Name)
    if ($actual.Count -ne $names.Count -or @($names | Where-Object { $_ -cnotin $actual }).Count -or
        $Report.format -ne 1 -or $Report.nonce -cne $Nonce -or $Nonce -cnotmatch '^[a-f0-9]{32}$' -or
        $Report.owner_sid -cne $OwnerSid -or $Report.session_id -ne $SessionId -or
        $Report.elevated -isnot [bool] -or -not $Report.elevated -or
        $Report.helper_sha256 -cne $HelperHash -or $HelperHash -cnotmatch '^[A-F0-9]{64}$') { throw 'Host audit identity mismatch.' }
    $checked = if ($Report.checked_utc -is [DateTime]) { $Report.checked_utc } else {
        [DateTime]::ParseExact($Report.checked_utc, 'o', [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind)
    }
    $age = ($NowUtc - $checked).TotalSeconds
    if ($checked.Kind -ne [DateTimeKind]::Utc -or $age -gt $script:MoHostAuditFreshnessSeconds -or
        $age -lt -$script:MoHostAuditFutureToleranceSeconds) { throw 'Host audit is stale or from the future.' }
    Assert-MoVmNoObservedTipHosts $Report.scan
}

function Invoke-MoVmElevatedHostInspection([string]$HelperPath, [string]$EvidenceDirectory) {
    $helper = Assert-MoVmPlainDosPath $HelperPath
    $helperHash = (Get-FileHash -LiteralPath $helper -Algorithm SHA256).Hash
    $nonce = [Guid]::NewGuid().ToString('N')
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    try { $ownerSid = $identity.User.Value } finally { $identity.Dispose() }
    $sessionId = (Get-Process -Id $PID).SessionId
    $auditRoot = Join-Path $env:ProgramData ("MoInstallerTest\HostAudits\{0}" -f $nonce)
    $arguments = @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', ('"{0}"' -f $helper),
        '-Nonce', $nonce, '-OwnerSid', $ownerSid, '-SessionId', [string]$sessionId)
    # The user handles any UAC prompt. The elevated child reads process metadata;
    # installation still runs later from this non-elevated initiating process.
    $start = @{ FilePath = (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe')
        ArgumentList = $arguments; Verb = 'RunAs'; WindowStyle = 'Hidden'; PassThru = $true }
    $process = Start-Process @start
    try {
        # Retain the OS handle before waiting, including on Windows PowerShell 5.1.
        # This keeps exit status readable after a short-lived worker has exited.
        $null = $process.Handle
        if (-not $process.WaitForExit($script:MoHostAuditWorkerCompletionTimeoutMilliseconds)) {
            throw 'Read-only host inspection did not complete.'
        }
        if ($process.ExitCode -ne 0) { throw 'Read-only host inspection worker failed.' }
    } finally {
        # Dispose releases our process handle; it does not terminate the worker.
        $process.Dispose()
    }
    Assert-MoVmHostAuditAcl (Split-Path -Parent $auditRoot)
    Assert-MoVmHostAuditAcl $auditRoot
    $path = Join-Path $auditRoot 'audit.json'
    Assert-MoVmHostAuditAcl $path
    if ((Get-Item -LiteralPath $path).Length -gt $script:MoHostAuditMaxBytes) { throw 'Host audit is too large.' }
    $report = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    Assert-MoVmHostAuditReport $report $nonce $ownerSid $sessionId $helperHash
    Copy-Item -LiteralPath $path -Destination (Join-Path $EvidenceDirectory 'tip-host-elevated-audit.json')
    return $report
}
