[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$ProbePath)
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
if(-not [IO.Path]::IsPathRooted($ProbePath) -or -not (Test-Path -LiteralPath $ProbePath -PathType Leaf)){
    throw 'Absolute existing deferred probe required'
}
$out=Join-Path $repo ('build/mo-deferred-check-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $out | Out-Null
$sha=(Get-FileHash -LiteralPath $ProbePath).Hash
function Run-Probe([string]$Mode,[int]$ExpectedExit){
    $start=[Diagnostics.ProcessStartInfo]::new()
    $start.FileName=$ProbePath
    $start.UseShellExecute=$false
    $start.CreateNoWindow=$true
    $start.RedirectStandardError=$true
    $start.RedirectStandardOutput=$true
    [void]$start.ArgumentList.Add($Mode)
    $child=[Diagnostics.Process]::Start($start)
    $stderr=$child.StandardError.ReadToEndAsync()
    $stdout=$child.StandardOutput.ReadToEndAsync()
    try{
        if(-not $child.WaitForExit(10000)){throw 'Owned deferred probe exceeded ceiling'}
        if($child.ExitCode -ne $ExpectedExit){throw "Deferred probe unexpected exit for $Mode"}
    }finally{
        if(-not $child.HasExited){$child.Kill($true)}
        if(-not $child.WaitForExit(3000) -or -not $stderr.Wait(3000) -or -not $stdout.Wait(3000)){
            throw 'Owned deferred probe cleanup/drain failed'
        }
        $log=$stdout.Result+$stderr.Result
        [IO.File]::WriteAllText((Join-Path $out ($Mode.TrimStart('-')+'.log')),$log,[Text.UTF8Encoding]::new($false))
        $child.Dispose()
    }
    return $log
}
$off=Run-Probe '--off' 0
$on=Run-Probe '--on' 0
$overflow=Run-Probe '--overflow' 0
$dispatch=Run-Probe '--dispatch' 0
$preprofile=Run-Probe '--preprofile' 0
$invalid=Run-Probe '--invalid' 2
if($off -notmatch 'MO_DEFER_PROBE disabled_pass' -or $off -match 'MO_DEFER_(READ|SCOPE|FLUSH)'){throw 'Deferred disabled guard failed'}
foreach($log in @($on,$overflow,$dispatch,$preprofile)){
    if($log -notmatch 'MO_DEFER_PROBE enabled_pass' -or [regex]::Matches($log,'MO_DEFER_READ sample=').Count -ne 512){throw 'Deferred read cap/data guard failed'}
    if($log.IndexOf('MO_DEFER_READ') -lt $log.IndexOf('MO_DEFER_PROBE armed_end') -or
        $log.IndexOf('MO_DEFER_SCOPE') -lt $log.IndexOf('MO_DEFER_PROBE armed_end')){throw 'Output emitted while armed'}
    if([regex]::Matches($log,'MO_DEFER_FLUSH').Count -ne 2 -or
        $log -notmatch 'MO_DEFER_FLUSH scopes=1 reads=0 scope_dropped=0 read_skipped=1 '){throw 'Nested ownership/reset/process cap guard failed'}
    if($log -match 'MO_READ sample=|MO_COMPONENT|MO_PREFETCH|start_(ns|us)=-|read_ns=-'){throw 'Deferred capture performed synchronous diagnostics'}
}
$threadRecords=@([regex]::Matches($dispatch,'MO_DEFER_THREAD enable_error=(\d+) first_error=(\d+) last_error=(\d+) disable_error=(\d+) valid=(\d+) context_switches=(\d+) wait_bitmap=(\d+) cycles=(\d+)'))
if($threadRecords.Count -ne 2){throw 'Missing paired thread dispatch records'}
# Availability is explicit. Unsupported dispatch must never be reported as valid.
foreach($thread in $threadRecords){if($thread.Groups[1].Value -eq '0'){if($thread.Groups[5].Value -ne '1'){throw 'Thread dispatch cleanup/read failed'}}elseif($thread.Groups[5].Value -ne '0'){throw 'Unsupported dispatch reported valid'}}
if($threadRecords[0].Groups[5].Value -eq '1' -and [int]$threadRecords[0].Groups[6].Value -lt 1){throw 'Owned synthetic wait not observed'}
if([regex]::Matches($preprofile,'MO_DEFER_THREAD enable_error=183 first_error=21 last_error=21 disable_error=21 valid=0 context_switches=0 wait_bitmap=0 cycles=0').Count -ne 2){throw 'Existing thread profiling ownership guard failed'}
if(($on+$overflow+$off) -match 'MO_DEFER_THREAD'){throw 'Disabled thread dispatch emitted records'}
if($on -notmatch 'MO_DEFER_FLUSH scopes=2 reads=512 scope_dropped=0 read_skipped=10 '){throw 'Deferred bounded capture counts incorrect'}
if($overflow -notmatch 'MO_DEFER_FLUSH scopes=2048 reads=512 scope_dropped=105 read_skipped=10 ' -or $overflow -notmatch 'truncated=1'){throw 'Scope overflow/truncation guard failed'}
$reads=@([regex]::Matches($on,'MO_DEFER_READ sample=(\d+) parent=(\d+) label=(\w+) start_ns=(\d+) read_ns=(\d+) truncated=0'))
for($i=0;$i -lt $reads.Count;$i++){if([int]$reads[$i].Groups[1].Value -ne $i){throw 'Scalar ordering changed'}}
$child=[regex]::Match($on,'MO_DEFER_SCOPE id=(\d+) parent=(\d+) category=probe label=nested ')
$root=[regex]::Match($on,'MO_DEFER_SCOPE id=(\d+) parent=0 category=engine label=ProcessKey ')
if(-not $child.Success -or -not $root.Success -or $child.Groups[2].Value -ne $root.Groups[1].Value -or $reads[0].Groups[2].Value -ne $child.Groups[1].Value){throw 'Deferred hierarchy not preserved'}
if($invalid -notmatch 'MO_DEFER_PROBE invalid_arguments' -or $invalid -match 'MO_DEFER_(READ|SCOPE|FLUSH)'){throw 'Invalid deferred arguments accepted'}
if(($on+$off+$overflow+$dispatch+$preprofile+$invalid) -match '0x|address=|value=|text=|key='){throw 'Deferred diagnostic exposed forbidden payload'}
if((Get-FileHash -LiteralPath $ProbePath).Hash -ne $sha){throw 'Deferred probe changed during check'}
[ordered]@{format=1;probe_sha256=$sha;synthetic_only=$true;off_exit=0;on_exit=0;overflow_exit=0;invalid_exit=2;dispatch_exit=0;existing_profile_preserved=$true;dispatch_available=($threadRecords[0].Groups[5].Value -eq '1');read_cap=512;scope_capacity=2048;scope_overflow=105;data_preserved=$true;deferred_output=$true;second_capture_resets_slots=$true}|
    ConvertTo-Json|Set-Content (Join-Path $out 'results.json') -Encoding utf8NoBOM
Write-Host "Deferred off/on/overflow/dispatch/preprofile/invalid cases passed; output $out"
$global:LASTEXITCODE=0
