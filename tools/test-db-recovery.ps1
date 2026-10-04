[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][ValidatePattern('^[A-Fa-f0-9]{64}$')][string]$ExpectedProbeSha256,
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9_-]*$')][string]$EvidenceName = 'win10-db-recovery-matrix-v1'
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$buildRoot = Join-Path $repoRoot 'build'
$out = Join-Path $buildRoot $EvidenceName
if (-not [IO.Path]::IsPathFullyQualified($ProbePath) -or -not (Test-Path -LiteralPath $ProbePath -PathType Leaf)) { throw 'An absolute compiled probe path is required' }
if ((Get-FileHash -LiteralPath $ProbePath -Algorithm SHA256).Hash -ne $ExpectedProbeSha256) { throw 'Probe hash mismatch' }
if (-not (Test-Path -LiteralPath $buildRoot -PathType Container) -or ((Get-Item -LiteralPath $buildRoot).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'A normal repository build directory is required' }
if (Test-Path -LiteralPath $out) { throw 'Fresh evidence directory required' }
New-Item -ItemType Directory -Path $out | Out-Null
$utf8 = [Text.UTF8Encoding]::new($false)
$rows = [Collections.Generic.List[object]]::new()
function Save-Results {
    $summary = [ordered]@{format=1;kind='mo-leveldb-recovery-matrix';synthetic_only=$true;power_loss_tested=$false;probe_sha256=$ExpectedProbeSha256;cases=@($rows.ToArray())}
    [IO.File]::WriteAllText((Join-Path $out 'results.json'),($summary | ConvertTo-Json -Depth 8)+"`n",$utf8)
}
function New-Fixture([string]$Name) {
    $path = Join-Path $out $Name
    New-Item -ItemType Directory -Path $path | Out-Null
    New-Item -ItemType File -Path (Join-Path $path 'mo-db-recovery-fixture') | Out-Null
    return $path
}
function Start-Probe([string]$Mode, [string]$Fixture, [int]$Reuse, [string[]]$Extra=@()) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $ProbePath
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($arg in @($Mode,$Fixture,[string]$Reuse)+$Extra) { [void]$start.ArgumentList.Add($arg) }
    return [Diagnostics.Process]::Start($start)
}
function Invoke-Probe([string]$Mode, [string]$Fixture, [int]$Reuse, [string]$Label, [int]$ExpectedExit=0, [string[]]$Extra=@()) {
    $child = Start-Probe $Mode $Fixture $Reuse $Extra
    $stdout = $child.StandardOutput.ReadToEndAsync()
    $stderr = $child.StandardError.ReadToEndAsync()
    try {
        if (-not $child.WaitForExit(60000) -or -not $stdout.Wait(3000) -or -not $stderr.Wait(3000)) { throw 'Owned recovery probe timeout' }
        [IO.File]::WriteAllText((Join-Path $Fixture "$Label-stdout.log"),$stdout.Result,$utf8)
        [IO.File]::WriteAllText((Join-Path $Fixture "$Label-stderr.log"),$stderr.Result,$utf8)
        if ($child.ExitCode -ne $ExpectedExit -or $stdout.Result -notmatch '^MO_DB_RECOVERY ' -or $stderr.Result -match 'assertion_failed=1') { throw "Recovery probe assertion failed: $Label" }
        return [ordered]@{mode=$Mode;reuse_logs=$Reuse;exit_code=$child.ExitCode;metadata=$stdout.Result.Trim()}
    } finally {
        if (-not $child.HasExited) { $child.Kill($true); [void]$child.WaitForExit(5000) }
        $child.Dispose()
    }
}
function Kill-DurableWriter([string]$Fixture, [int]$Reuse) {
    $child = Start-Probe '--crash-writer' $Fixture $Reuse
    $stderr = $child.StandardError.ReadToEndAsync()
    try {
        $line = $child.StandardOutput.ReadLineAsync()
        if (-not $line.Wait(30000) -or $line.Result -ne 'MO_DB_RECOVERY durable_ready=1' -or $child.HasExited) { throw 'Durable child handshake failed' }
        $child.Kill($true)
        if (-not $child.WaitForExit(5000) -or -not $stderr.Wait(3000) -or $child.ExitCode -eq 0) { throw 'Owned child termination failed' }
        [IO.File]::WriteAllText((Join-Path $Fixture 'crash-stdout.log'),$line.Result+"`n",$utf8)
        [IO.File]::WriteAllText((Join-Path $Fixture 'crash-stderr.log'),$stderr.Result,$utf8)
        return [ordered]@{mode='--crash-writer';reuse_logs=$Reuse;exit_code=$child.ExitCode;durable_handshake=$true;parent_terminated=$true}
    } finally {
        if (-not $child.HasExited) { $child.Kill($true); [void]$child.WaitForExit(5000) }
        $child.Dispose()
    }
}
foreach ($writer in 0..1) {
    foreach ($reader in 0..1) {
        $name = "crash-w$writer-r$reader"
        $fixture = New-Fixture $name
        $seed = Invoke-Probe '--seed' $fixture 0 'seed'
        $crash = Kill-DurableWriter $fixture $writer
        $verify = Invoke-Probe '--verify-crash' $fixture $reader 'verify'
        $rows.Add([ordered]@{case=$name;negative_evidence=$false;seed=$seed;crash=$crash;verify=$verify})
        Save-Results
        Write-Host "$name recovered synchronous updates/deletes and continued writing"
    }
}
foreach ($mode in @('--append-log','--append-manifest')) {
    $name = $mode.Substring(2)
    $fixture = New-Fixture $name
    $seed = Invoke-Probe '--seed' $fixture 0 'seed'
    $verify = Invoke-Probe $mode $fixture 1 'verify'
    $rows.Add([ordered]@{case=$name;negative_evidence=$false;seed=$seed;verify=$verify})
    Save-Results
    Write-Host "$name fallback preserved synthetic records"
}
foreach ($reuse in 0..1) {
    $name = "sync-error-r$reuse"
    $fixture = New-Fixture $name
    $seed = Invoke-Probe '--seed' $fixture 0 'seed'
    $verify = Invoke-Probe '--sync-error' $fixture $reuse 'verify'
    $rows.Add([ordered]@{case=$name;negative_evidence=$false;seed=$seed;verify=$verify})
    Save-Results
    Write-Host "$name returned and latched the injected error"
    foreach ($strict in 0..1) {
        $name = "read-error-r$reuse-strict$strict"
        $fixture = New-Fixture $name
        $seed = Invoke-Probe '--seed' $fixture 0 'seed'
        $exit = if ($strict) { 0 } else { 2 }
        $verify = Invoke-Probe '--read-error' $fixture $reuse 'verify' $exit @([string]$strict)
        $rows.Add([ordered]@{case=$name;negative_evidence=($strict -eq 0);seed=$seed;verify=$verify})
        Save-Results
        Write-Host "$name recorded recovery policy behavior"
    }
    $name = "large-r$reuse"
    $fixture = New-Fixture $name
    $verify = Invoke-Probe '--large' $fixture $reuse 'verify'
    $rows.Add([ordered]@{case=$name;negative_evidence=$false;verify=$verify})
    Save-Results
    Write-Host "$name recovered 32768 synthetic records through compaction"
}
Write-Host 'Recovery matrix completed; read-error negative evidence still blocks a product safety conclusion.'
