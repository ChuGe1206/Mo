[CmdletBinding()]
param([Parameter(Mandatory)][string]$ProbePath)
$ErrorActionPreference='Stop'
$repoRoot=Split-Path -Parent $PSScriptRoot
$script=Join-Path $PSScriptRoot 'test-db-recovery.ps1'
$absolute=(Resolve-Path -LiteralPath $ProbePath).Path
$hash=(Get-FileHash -LiteralPath $absolute).Hash
$tag='db-recovery-policy-'+[Guid]::NewGuid().ToString('N')
$existing=Join-Path $repoRoot "build/$tag"
$errors=$null;$tokens=$null
[void][Management.Automation.Language.Parser]::ParseFile($script,[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'Recovery harness AST invalid'}
function Reject([string]$Label,[hashtable]$Arguments,[string]$Message){
    $rejected=$false
    try { & $script @Arguments }
    catch { if($_.Exception.Message -notmatch $Message){throw};$rejected=$true }
    if(-not $rejected){throw "$Label unexpectedly accepted"}
    if($Label -ne 'existing evidence' -and (Test-Path -LiteralPath $existing)){throw 'Rejected invocation created output'}
    Write-Host "$Label rejected before starting a child"
}
Reject 'relative probe' @{ProbePath='relative.exe';ExpectedProbeSha256=$hash;EvidenceName=$tag} 'absolute compiled probe'
Reject 'missing probe' @{ProbePath=(Join-Path $repoRoot 'build/missing-db-recovery-policy.exe');ExpectedProbeSha256=$hash;EvidenceName=$tag} 'absolute compiled probe'
Reject 'hash mismatch' @{ProbePath=$absolute;ExpectedProbeSha256=('0'*64);EvidenceName=$tag} 'hash mismatch'
Reject 'escaping name' @{ProbePath=$absolute;ExpectedProbeSha256=$hash;EvidenceName='../escape'} 'EvidenceName'
New-Item -ItemType Directory -Path $existing | Out-Null
$sentinel=Join-Path $existing 'policy-sentinel'
[IO.File]::WriteAllText($sentinel,'synthetic-sentinel',[Text.UTF8Encoding]::new($false))
try{
    Reject 'existing evidence' @{ProbePath=$absolute;ExpectedProbeSha256=$hash;EvidenceName=$tag} 'Fresh evidence directory'
    if([IO.File]::ReadAllText($sentinel) -ne 'synthetic-sentinel' -or @(Get-ChildItem -LiteralPath $existing).Count -ne 1){throw 'Existing evidence modified'}
}finally{
    Remove-Item -LiteralPath $sentinel
    Remove-Item -LiteralPath $existing
}
$nativeRoot=Join-Path $repoRoot ('build/db-recovery-native-policy-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $nativeRoot | Out-Null
$marker=Join-Path $nativeRoot 'mo-db-recovery-fixture'
try {
    $cases=@(
        @{name='missing marker';args=@('--seed',$nativeRoot,'0')},
        @{name='relative fixture';args=@('--seed','relative-fixture','0')},
        @{name='unknown mode';args=@('--unknown',$nativeRoot,'0')},
        @{name='invalid reuse flag';args=@('--seed',$nativeRoot,'2')},
        @{name='extra seed argument';args=@('--seed',$nativeRoot,'0','extra')}
    )
    foreach($case in $cases){
        if($case.name -eq 'unknown mode'){New-Item -ItemType File -Path $marker | Out-Null}
        $start=[Diagnostics.ProcessStartInfo]::new()
        $start.FileName=$absolute;$start.UseShellExecute=$false;$start.CreateNoWindow=$true
        $start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
        foreach($arg in $case.args){[void]$start.ArgumentList.Add($arg)}
        $child=[Diagnostics.Process]::Start($start)
        $stdout=$child.StandardOutput.ReadToEndAsync();$stderr=$child.StandardError.ReadToEndAsync()
        try {
            if(-not $child.WaitForExit(5000) -or -not $stdout.Wait(1000) -or -not $stderr.Wait(1000) -or $child.ExitCode -ne 1 -or $stdout.Result.Length -ne 0 -or $stderr.Result.Trim() -ne 'MO_DB_RECOVERY assertion_failed=1'){throw 'Native fixture guard failed'}
            if(Test-Path -LiteralPath (Join-Path $nativeRoot 'synthetic-db')){throw 'Rejected native invocation created a DB'}
            Write-Host "native $($case.name) rejected without creating a DB"
        }finally{
            if(-not $child.HasExited){$child.Kill($true);[void]$child.WaitForExit(5000)}
            $child.Dispose()
        }
    }
}finally{
    if(Test-Path -LiteralPath $marker){Remove-Item -LiteralPath $marker}
    Remove-Item -LiteralPath $nativeRoot
}
Write-Host 'Recovery AST, five harness and five native fixture policy checks passed.'
