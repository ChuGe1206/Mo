[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$TestExe,
    [Parameter(Mandatory)][ValidatePattern('^[A-Fa-f0-9]{64}$')][string]$ExpectedSha256,
    [Parameter(Mandatory)][ValidateSet('legacy-abort','current-terminate')][string]$Variant,
    [ValidateRange(1,3)][int]$Repetitions=1,
    [Parameter(Mandatory)][ValidatePattern('^[A-Za-z0-9][A-Za-z0-9_-]*$')][string]$EvidenceName
)
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
$out=Join-Path $repo "build/$EvidenceName"
if(-not [IO.Path]::IsPathFullyQualified($TestExe) -or -not (Test-Path -LiteralPath $TestExe -PathType Leaf)){throw 'Absolute test executable required'}
if((Get-FileHash -LiteralPath $TestExe).Hash -ne $ExpectedSha256){throw 'Test executable hash mismatch'}
foreach($path in @($TestExe,(Join-Path $repo 'build'))) {
    $item=Get-Item -LiteralPath $path
    while($null -ne $item) {
        if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Reparse path rejected'}
        $item=if($item -is [IO.DirectoryInfo]){$item.Parent}else{$item.Directory}
    }
}
if(Test-Path -LiteralPath $out){throw 'Fresh evidence directory required'}
New-Item -ItemType Directory -Path $out | Out-Null
$utf8=[Text.UTF8Encoding]::new($false)
$rows=[Collections.Generic.List[object]]::new()
foreach($trial in 1..$Repetitions) {
    foreach($phase in @('startup','create','apply','destroy','finalize','panic','worker-panic','pool-shutdown')) {
        $start=[Diagnostics.ProcessStartInfo]::new()
        $start.FileName=$TestExe; $start.UseShellExecute=$false; $start.CreateNoWindow=$true
        $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
        foreach($arg in @('--exact','engine_service::tests::subprocess_fault_fixture','--nocapture')){[void]$start.ArgumentList.Add($arg)}
        $start.Environment['MO_TEST_ENGINE_FAULT']=$phase
        $child=[Diagnostics.Process]::Start($start)
        $watch=[Diagnostics.Stopwatch]::StartNew()
        $stderr=$child.StandardError.ReadToEndAsync()
        $line=$child.StandardOutput.ReadLineAsync()
        $lines=[Collections.Generic.List[string]]::new()
        $faultMs=$null; $stopMs=$null; $exitMs=$null; $reporterMatched=$false; $killed=$false
        try {
            while($watch.ElapsedMilliseconds -lt 8000) {
                if($null -eq $exitMs -and $child.HasExited){$exitMs=$watch.ElapsedMilliseconds}
                if($line.Wait(10)) {
                    if($null -eq $line.Result){break}
                    $lines.Add($line.Result)
                    if($line.Result.Contains("MO_TEST_FAULT:$phase")){$faultMs=$watch.ElapsedMilliseconds}
                    if($line.Result.Contains('MO_TEST_FAIL_STOP_ENTER')){$stopMs=$watch.ElapsedMilliseconds}
                    $line=$child.StandardOutput.ReadLineAsync()
                }
                if($Variant -eq 'legacy-abort' -and -not $reporterMatched -and @([Diagnostics.Process]::GetProcessesByName('WerFault')).Count) {
                    foreach($reporter in Get-CimInstance Win32_Process -Filter "Name='WerFault.exe'") {
                        if($reporter.CommandLine -match ("(?:^|\s)-p\s+"+$child.Id+"(?:\s|$)")){$reporterMatched=$true}
                    }
                }
            }
            if(-not $child.HasExited){$child.Kill($true); $killed=$true}
            if(-not $child.WaitForExit(5000) -or -not $stderr.Wait(3000)){throw 'Owned child cleanup failed'}
            if($null -eq $exitMs -and -not $killed){$exitMs=$watch.ElapsedMilliseconds}
            $label="$Variant-$phase-$trial"
            [IO.File]::WriteAllText((Join-Path $out "$label-stdout.log"),($lines -join "`n")+"`n",$utf8)
            [IO.File]::WriteAllText((Join-Path $out "$label-stderr.log"),$stderr.Result,$utf8)
            $expectedExit=if($Variant -eq 'legacy-abort'){0xc0000409L}else{0xe04d4f01L}
            $actualExit=[long]$child.ExitCode -band 0xffffffffL
            $row=[ordered]@{phase=$phase;trial=$trial;fault_ms=$faultMs;fail_stop_entry_ms=$stopMs;exit_ms=$exitMs;exit_code_hex=('{0:X8}' -f $actualExit);parent_terminated=$killed;matching_wer_reporter_observed=$reporterMatched;within_original_three_seconds=($null -ne $exitMs -and $exitMs -lt 3000)}
            $rows.Add($row)
            $summary=[ordered]@{format=1;kind='mo-watchdog-exit-diagnostic';variant=$Variant;synthetic_only=$true;exe_sha256=$ExpectedSha256;measurement='parent receipt and process polling; not native CPU time';cases=@($rows.ToArray())}
            [IO.File]::WriteAllText((Join-Path $out 'results.json'),($summary | ConvertTo-Json -Depth 7)+"`n",$utf8)
            Write-Host ($row | ConvertTo-Json -Compress)
            if($null -eq $faultMs -or $killed -or $actualExit -ne $expectedExit -or ($Variant -eq 'legacy-abort' -and $null -eq $stopMs)){throw 'Fault fixture or expected termination marker/status failed'}
            if($Variant -eq 'current-terminate' -and -not $row.within_original_three_seconds){throw 'Current termination exceeded original budget'}
        } finally {
            if(-not $child.HasExited){$child.Kill($true);[void]$child.WaitForExit(5000)}
            $child.Dispose()
        }
    }
}
