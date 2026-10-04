[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$LibrimeDistDir,
    [Parameter(Mandatory=$true)][string]$SharedDataDir,
    [Parameter(Mandatory=$true)][string]$BrokerPath,
    [Parameter(Mandatory=$true)][string]$NativeOutputDirectory,
    [Parameter(Mandatory=$true)][ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._-]*$')][string]$EvidenceName,
    [switch]$ReadPageTrace,
    [switch]$PrefetchRanges
)
# Development measurement with fixed synthetic probes in new owned profiles.
# A zero script exit means collection completed, NOT first-key acceptance.
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
$out=Join-Path $repo ('build/'+$EvidenceName)
if(Test-Path -LiteralPath $out){throw 'Fresh evidence directory required'}
if(@(Get-Process mo-broker -ErrorAction SilentlyContinue).Count){throw 'Existing Broker must remain untouched'}
function Require-Input([string]$Path,[string]$Type){
    if(-not [IO.Path]::IsPathRooted($Path) -or -not (Test-Path -LiteralPath $Path -PathType $Type)){throw 'Absolute existing diagnostic inputs required'}
    return (Resolve-Path -LiteralPath $Path).Path
}
$shared=Require-Input $SharedDataDir 'Container'
$runtime=Require-Input (Join-Path $LibrimeDistDir 'lib/rime.dll') 'Leaf'
$sourceBroker=Require-Input $BrokerPath 'Leaf'
$native=Require-Input $NativeOutputDirectory 'Container'
foreach($name in @('default.yaml','rime_ice.schema.yaml','build/default.yaml','build/rime_ice.schema.yaml')){[void](Require-Input (Join-Path $shared $name) 'Leaf')}
foreach($arch in @('x64','Win32')){foreach($name in @('mo_tip.dll','mo_tip_abi_probe.exe')){[void](Require-Input (Join-Path $native "$arch/Release/$name") 'Leaf')}}
function Invoke-OwnedProbe([string]$Probe,[string]$Tip,[string]$Log){
    $start=[Diagnostics.ProcessStartInfo]::new()
    $start.FileName=$Probe
    $start.UseShellExecute=$false
    $start.CreateNoWindow=$true
    $start.RedirectStandardOutput=$true
    $start.RedirectStandardError=$true
    foreach($arg in @($Tip,'--broker-rime-ice')){[void]$start.ArgumentList.Add($arg)}
    $child=[Diagnostics.Process]::Start($start)
    $stdout=$child.StandardOutput.ReadToEndAsync()
    $stderr=$child.StandardError.ReadToEndAsync()
    try{
        if(-not $child.WaitForExit(40000)){throw 'Owned probe exceeded collection ceiling'}
        return $child.ExitCode
    }finally{
        if(-not $child.HasExited){$child.Kill($true)}
        if(-not $child.WaitForExit(3000)){throw 'Owned probe did not exit'}
        if(-not $stdout.Wait(3000) -or -not $stderr.Wait(3000)){throw 'Owned probe output drain failed'}
        [IO.File]::WriteAllText($Log,$stdout.Result+$stderr.Result,[Text.UTF8Encoding]::new($false))
        $child.Dispose()
    }
}
$inputs=[ordered]@{}
foreach($file in @($runtime,$sourceBroker)+@(Get-ChildItem -LiteralPath $shared -File -Recurse|ForEach-Object FullName)+@(Get-ChildItem -LiteralPath (Join-Path $LibrimeDistDir 'lib/opencc') -File|ForEach-Object FullName)+@((Join-Path $native 'x64/Release/mo_tip.dll'),(Join-Path $native 'x64/Release/mo_tip_abi_probe.exe'),(Join-Path $native 'Win32/Release/mo_tip.dll'),(Join-Path $native 'Win32/Release/mo_tip_abi_probe.exe'))){$inputs[$file]=(Get-FileHash -LiteralPath $file).Hash}
New-Item -ItemType Directory -Path $out | Out-Null
$inputs|ConvertTo-Json -Depth 5|Set-Content (Join-Path $out 'inputs.json') -Encoding utf8NoBOM
$rows=@()
foreach($arch in @('x64','Win32')){
    $root=Join-Path $out $arch
    $bin=Join-Path $root 'Mo/bin'
    $tipDir=Join-Path $root ('Mo/tip/'+$(if($arch -eq 'Win32'){'x86'}else{'x64'}))
    $user=Join-Path $root 'user'
    New-Item -ItemType Directory -Path $bin,$tipDir,$user | Out-Null
    $broker=Join-Path $bin 'mo-broker.exe'
    $tip=Join-Path $tipDir 'mo-tip.dll'
    $probe=Join-Path $bin 'mo_tip_abi_probe.exe'
    Copy-Item -LiteralPath $sourceBroker -Destination $broker
    Copy-Item -LiteralPath (Join-Path $native "$arch/Release/mo_tip.dll") -Destination $tip
    Copy-Item -LiteralPath (Join-Path $native "$arch/Release/mo_tip_abi_probe.exe") -Destination $probe
    Copy-Item -LiteralPath (Join-Path $shared 'build') -Destination (Join-Path $user 'build') -Recurse
    New-Item -ItemType File -Path (Join-Path $user 'mo-latency-fixture') | Out-Null
    foreach($state in @('fresh','existing')){
        $case=Join-Path $root $state
        New-Item -ItemType Directory -Path $case | Out-Null
        $start=[Diagnostics.ProcessStartInfo]::new()
        $start.FileName=$broker
        $start.UseShellExecute=$false
        $start.CreateNoWindow=$true
        $start.RedirectStandardError=$true
        $start.Environment["MO_DIAG_READ_PAGES"]=$(if($ReadPageTrace){"1"}else{"0"})
        $start.Environment["MO_DIAG_PREFETCH_RANGES"]=$(if($PrefetchRanges){"1"}else{"0"})
        foreach($arg in @('--rime-prepared',$runtime,$shared,$user)){[void]$start.ArgumentList.Add($arg)}
        $watch=[Diagnostics.Stopwatch]::StartNew()
        $child=[Diagnostics.Process]::Start($start)
        $drain=$null
        $prefix=[Collections.Generic.List[string]]::new()
        try{
            do{
                $line=$child.StandardError.ReadLineAsync()
                $remaining=30000-[int]$watch.ElapsedMilliseconds
                if($remaining -le 0 -or -not $line.Wait($remaining) -or $null -eq $line.Result){throw 'Owned Broker readiness failed'}
                $prefix.Add($line.Result)
            }while($line.Result -cne 'Mo broker listening on 16 protected pipe slots')
            $readyMs=$watch.ElapsedMilliseconds
            $drain=$child.StandardError.ReadToEndAsync()
            $first=Invoke-OwnedProbe $probe $tip (Join-Path $case 'first-probe.log')
            Start-Sleep -Milliseconds 2000
            $second=Invoke-OwnedProbe $probe $tip (Join-Path $case 'second-probe.log')
            Start-Sleep -Milliseconds 500
        }finally{
            if(-not $child.HasExited){$child.Kill($true)}
            if(-not $child.WaitForExit(3000)){throw 'Owned Broker did not exit'}
            $log=$prefix -join [char]10
            if($drain -and $drain.Wait(3000)){$log+=[char]10+$drain.Result}
            [IO.File]::WriteAllText((Join-Path $case 'broker.log'),$log,[Text.UTF8Encoding]::new($false))
            $child.Dispose()
        }
        $dispatch=[regex]::Match($log,'MO_LATENCY op=dispatch queue_us=(\d+) engine_us=(\d+) dropped=(\d+)')
        if(-not $dispatch.Success){throw 'Missing completed dispatch evidence'}
        $row=[ordered]@{architecture=$arch;profile=$state;ready_ms=$readyMs;first_probe_exit=$first;second_probe_exit=$second;first_dispatch_queue_us=[long]$dispatch.Groups[1].Value;first_dispatch_engine_us=[long]$dispatch.Groups[2].Value;dropped=[long]$dispatch.Groups[3].Value;runtime_sha256=(Get-FileHash $runtime).Hash;broker_sha256=(Get-FileHash $broker).Hash;tip_sha256=(Get-FileHash $tip).Hash;probe_sha256=(Get-FileHash $probe).Hash}
        $rows+=$row
        [ordered]@{format=1;kind='mo-win10-tip-latency-diagnostic';synthetic_only=$true;deadline_changed=$false;read_page_trace=[bool]$ReadPageTrace;prefetch_ranges=[bool]$PrefetchRanges;input_free_preparation=$true;debug_plan='local-user-build';cases=$rows}|ConvertTo-Json -Depth 5|Set-Content (Join-Path $out 'results.json') -Encoding utf8NoBOM
        $row|ConvertTo-Json -Compress
    }
}
foreach($path in $inputs.Keys){if((Get-FileHash -LiteralPath $path).Hash -ne $inputs[$path]){throw 'Diagnostic source input changed'}}
if(@(Get-Process mo-broker -ErrorAction SilentlyContinue).Count){throw 'Residual Broker'}
Write-Host 'Measurement collection complete. Inspect both probe exits; this is not acceptance.'
$global:LASTEXITCODE=0
