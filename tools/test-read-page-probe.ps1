[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$ProbePath)
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
if(-not [IO.Path]::IsPathRooted($ProbePath) -or -not (Test-Path -LiteralPath $ProbePath -PathType Leaf)){
    throw 'Absolute existing read-page probe required'
}
$out=Join-Path $repo ('build/mo-read-page-check-'+[Guid]::NewGuid().ToString('N'))
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
        if(-not $child.WaitForExit(10000)){throw 'Owned read-page probe exceeded ceiling'}
        if($child.ExitCode -ne $ExpectedExit){throw "Read-page probe unexpected exit for $Mode"}
    }finally{
        if(-not $child.HasExited){$child.Kill($true)}
        if(-not $child.WaitForExit(3000) -or -not $stderr.Wait(3000) -or -not $stdout.Wait(3000)){
            throw 'Owned read-page probe cleanup/drain failed'
        }
        $log=$stdout.Result+$stderr.Result
        [IO.File]::WriteAllText((Join-Path $out ($Mode.TrimStart('-')+'.log')),$log,[Text.UTF8Encoding]::new($false))
        $child.Dispose()
    }
    return $log
}
$off=Run-Probe '--trace-off' 0
$on=Run-Probe '--trace-on' 0
$prefetch=Run-Probe '--prefetch-on' 0
$invalid=Run-Probe '--invalid' 2
if($off -notmatch 'MO_READ_PROBE trace_off_pass' -or $off -match 'MO_READ sample='){throw 'Disabled sampling guard failed'}
if($on -notmatch 'MO_READ_PROBE trace_on_pass' -or [regex]::Matches($on,'MO_READ sample=').Count -ne 512){
    throw 'Sampling cap/value-preservation guard failed'
}
if($on -notmatch 'label=probe_private mapped=0 before_valid=1 before_resident=1 after_valid=1 after_resident=1' -or
    $on -notmatch 'label=probe_mapped mapped=1 before_valid=1 before_resident=0 after_valid=1 after_resident=1'){
    throw 'Synthetic page classification/residency transition failed'
}
if($prefetch -notmatch 'MO_READ_PROBE prefetch_on_pass' -or $prefetch -notmatch 'MO_PREFETCH target=mapped ranges=1 bytes=\d+ result=1 error=0' -or $prefetch -match 'MO_READ sample='){throw 'Prefetch hint/control failed'}
if($invalid -notmatch 'MO_READ_PROBE invalid_arguments' -or $invalid -match 'MO_READ sample='){throw 'Invalid probe arguments accepted'}
if(($on+$off+$prefetch+$invalid) -match '0x|address=|value=|text=|key='){throw 'Read diagnostic exposed forbidden payload'}
if((Get-FileHash -LiteralPath $ProbePath).Hash -ne $sha){throw 'Read-page probe changed during check'}
[ordered]@{format=1;probe_sha256=$sha;synthetic_only=$true;trace_off_exit=0;trace_on_exit=0;invalid_exit=2;prefetch_exit=0;prefetch_keeps_page_nonresident=$true;sample_cap=512;data_preserved=$true}|
    ConvertTo-Json|Set-Content (Join-Path $out 'results.json') -Encoding utf8NoBOM
Write-Host "Read-page disabled/enabled/prefetch/invalid cases passed; output $out"
$global:LASTEXITCODE=0
