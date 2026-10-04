[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$RuntimePath,
    [Parameter(Mandatory)][string]$ActorPath,
    [Parameter(Mandatory)][string]$BrokerPath,
    [Parameter(Mandatory)][string]$FixtureProbePath,
    [Parameter(Mandatory)][hashtable]$ExpectedHashes,
    [Parameter(Mandatory)][string]$SharedDataDir,
    [Parameter(Mandatory)][string]$CompiledDataDir,
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9_-]*$')][string]$EvidenceName='win10-userdb-errors-v1'
)
$ErrorActionPreference='Stop'
$repoRoot=Split-Path -Parent $PSScriptRoot
$buildRoot=Join-Path $repoRoot 'build'
$out=Join-Path $buildRoot $EvidenceName
function Assert-NormalPath([string]$Path) {
    if (-not [IO.Path]::IsPathFullyQualified($Path) -or -not (Test-Path -LiteralPath $Path)) { throw 'Existing absolute input required' }
    $item=Get-Item -LiteralPath $Path
    while ($null -ne $item) {
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse input rejected' }
        $item=if ($item -is [IO.DirectoryInfo]) { $item.Parent } else { $item.Directory }
    }
}
$paths=@{runtime=$RuntimePath;actor=$ActorPath;broker=$BrokerPath;fixture=$FixtureProbePath}
foreach ($entry in $paths.GetEnumerator()) {
    Assert-NormalPath $entry.Value
    if ($ExpectedHashes[$entry.Key] -notmatch '^[A-Fa-f0-9]{64}$' -or (Get-FileHash -LiteralPath $entry.Value).Hash -ne $ExpectedHashes[$entry.Key]) { throw 'Input hash mismatch' }
}
foreach ($path in @($SharedDataDir,$CompiledDataDir,$buildRoot)) { Assert-NormalPath $path }
foreach ($name in @('default.yaml','rime_ice.schema.yaml')) {
    if (-not (Test-Path -LiteralPath (Join-Path $CompiledDataDir $name) -PathType Leaf)) { throw 'Compiled schema required' }
}
foreach ($file in Get-ChildItem -LiteralPath $CompiledDataDir -Recurse -Force) {
    if ($file.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Compiled fixture reparse rejected' }
}
if (Test-Path -LiteralPath $out) { throw 'Fresh evidence directory required' }
New-Item -ItemType Directory -Path $out | Out-Null
$utf8=[Text.UTF8Encoding]::new($false)
$rows=[Collections.Generic.List[object]]::new()
function Save-Results {
    $summary=[ordered]@{format=1;kind='mo-real-userdb-errors';synthetic_only=$true;runtime_sha256=$ExpectedHashes.runtime;actor_sha256=$ExpectedHashes.actor;broker_sha256=$ExpectedHashes.broker;fixture_sha256=$ExpectedHashes.fixture;cases=@($rows.ToArray())}
    [IO.File]::WriteAllText((Join-Path $out 'results.json'),($summary | ConvertTo-Json -Depth 10)+"`n",$utf8)
}
function New-Profile([string]$Name) {
    $path=Join-Path $out $Name
    New-Item -ItemType Directory -Path $path | Out-Null
    foreach ($marker in @('mo-latency-fixture','mo-userdb-fixture')) { New-Item -ItemType File -Path (Join-Path $path $marker) | Out-Null }
    Copy-Item -LiteralPath $CompiledDataDir -Destination (Join-Path $path 'build') -Recurse
    return $path
}
function Invoke-Owned([string]$Kind,[string]$User,[string]$Label,[bool]$Failure=$false,[string]$FixtureMode='--verify') {
    $start=[Diagnostics.ProcessStartInfo]::new()
    $start.FileName=$paths[$Kind]
    $start.UseShellExecute=$false; $start.CreateNoWindow=$true
    $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    $arguments=switch ($Kind) {
        actor { @($RuntimePath,$SharedDataDir,$User,'--broker-plan') }
        broker { @('--rime-prepared',$RuntimePath,$SharedDataDir,$User) }
        fixture { @($FixtureMode,$User) }
    }
    foreach ($arg in $arguments) { [void]$start.ArgumentList.Add($arg) }
    $child=[Diagnostics.Process]::Start($start)
    $stdout=$child.StandardOutput.ReadToEndAsync()
    $stderr=if ($Kind -eq 'broker' -and -not $Failure) { $null } else { $child.StandardError.ReadToEndAsync() }
    $ready=$false; $lines=[Collections.Generic.List[string]]::new()
    try {
        if ($Kind -eq 'broker' -and -not $Failure) {
            $watch=[Diagnostics.Stopwatch]::StartNew()
            while (-not $ready) {
                $line=$child.StandardError.ReadLineAsync()
                $remaining=30000-[int]$watch.ElapsedMilliseconds
                if ($remaining -le 0 -or -not $line.Wait($remaining) -or $null -eq $line.Result) { throw 'Owned Broker readiness timeout' }
                $lines.Add($line.Result)
                $ready=$line.Result.Contains('Mo broker listening')
            }
            $child.Kill($true)
            $stderr=$child.StandardError.ReadToEndAsync()
        }
        if (-not $child.WaitForExit(30000) -or -not $stdout.Wait(3000) -or -not $stderr.Wait(3000)) { throw 'Owned child timeout' }
        $err=($lines -join "`n")+"`n"+$stderr.Result
        [IO.File]::WriteAllText((Join-Path $out "$Label-stdout.log"),$stdout.Result,$utf8)
        [IO.File]::WriteAllText((Join-Path $out "$Label-stderr.log"),$err,$utf8)
        if ($Failure) {
            if ($child.ExitCode -eq 0 -or $err -notmatch 'mo_rime_prepare_resources_v2' -or $stdout.Result -match 'MO_ACTOR_READY|MO_ACTOR trial=' -or $err -match 'Mo broker listening') { throw 'Preparation failure contract not met' }
        } elseif ($Kind -eq 'actor') {
            if ($child.ExitCode -ne 0 -or $stdout.Result -notmatch 'MO_ACTOR_READY' -or [regex]::Matches($stdout.Result,'MO_ACTOR trial=\d broker_plan=true .*candidate_count=5').Count -ne 2) { throw 'Actor recovery assertions failed' }
        } elseif ($Kind -eq 'fixture') {
            if ($child.ExitCode -ne 0 -or $stdout.Result.Trim() -ne 'MO_USERDB_FIXTURE records_verified=32') { throw 'Synthetic records lost' }
        }
        $rows.Add([ordered]@{case=$Label;kind=$Kind;exit_code=$child.ExitCode;expected_failure=$Failure;ready=$ready;parent_terminated_after_ready=$ready;stdout=$stdout.Result.Trim();stderr=$err.Trim()})
        Save-Results
        Write-Host "$Label passed"
    } finally {
        if (-not $child.HasExited) { $child.Kill($true); [void]$child.WaitForExit(5000) }
        $child.Dispose()
    }
}
function Db-State([string]$User) {
    $db=Join-Path $User 'rime_ice.userdb'
    if (-not (Test-Path -LiteralPath $db -PathType Container) -or (Test-Path -LiteralPath ($db+'.old'))) { throw 'Original DB moved or removed' }
    $state=[ordered]@{}
    foreach ($file in Get-ChildItem -LiteralPath $db -Force | Sort-Object Name) {
        if ($file.PSIsContainer -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Unexpected DB entry' }
        if ($file.Name -notin @('LOCK','LOG','LOG.old')) { $state[$file.Name]=(Get-FileHash -LiteralPath $file.FullName).Hash }
    }
    return ($state | ConvertTo-Json -Compress)
}
$seed=New-Profile 'seed'
Invoke-Owned actor $seed 'seed-actor'
Invoke-Owned fixture $seed 'seed-records' $false '--seed'
foreach ($fault in @('read-sharing-error','wal-checksum-error')) {
    $user=New-Profile $fault
    Copy-Item -LiteralPath (Join-Path $seed 'rime_ice.userdb') -Destination (Join-Path $user 'rime_ice.userdb') -Recurse
    $logs=@(Get-ChildItem -LiteralPath (Join-Path $user 'rime_ice.userdb') -Filter '*.log' -File)
    if ($logs.Count -ne 1 -or $logs[0].Length -lt 7) { throw 'One nonempty owned WAL required' }
    $wal=$logs[0].FullName
    $original=[IO.File]::ReadAllBytes($wal)
    $lock=$null
    if ($fault -eq 'wal-checksum-error') { $damaged=$original.Clone(); $damaged[0]=$damaged[0] -bxor 1; [IO.File]::WriteAllBytes($wal,$damaged) }
    $before=Db-State $user
    [IO.File]::WriteAllText((Join-Path $out "$fault-before.json"),$before+"`n",$utf8)
    try {
        if ($fault -eq 'read-sharing-error') { $lock=[IO.File]::Open($wal,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::None) }
        Invoke-Owned actor $user "$fault-actor-rejected" $true
        Invoke-Owned broker $user "$fault-broker-rejected" $true
    } finally { if ($null -ne $lock) { $lock.Dispose() } }
    $after=Db-State $user
    [IO.File]::WriteAllText((Join-Path $out "$fault-after.json"),$after+"`n",$utf8)
    if ($before -cne $after) { throw 'Failed preparation changed original data files' }
    if ($fault -eq 'wal-checksum-error') { [IO.File]::WriteAllBytes($wal,$original) }
    Invoke-Owned actor $user "$fault-actor-recovered"
    Invoke-Owned broker $user "$fault-broker-recovered"
    Invoke-Owned fixture $user "$fault-records-preserved"
}
Write-Host 'Actual librime/Actor/Broker fault matrix passed; 32 synthetic records preserved per case.'
