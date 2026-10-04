[CmdletBinding()]
param([Parameter(Mandatory)][string]$TestExe)
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
$hash=(Get-FileHash -LiteralPath $TestExe).Hash
$count=0
foreach($case in @('relative','missing','hash','escape','existing','variant')) {
    $name='watchdog-policy-'+[Guid]::NewGuid().ToString('N')
    $out=Join-Path $repo "build/$name"
    $invocation=@{TestExe=$TestExe;ExpectedSha256=$hash;Variant='current-terminate';EvidenceName=$name}
    switch($case) {
        relative {$invocation.TestExe='fixture.exe'}
        missing {$invocation.TestExe=Join-Path $repo 'build/missing-watchdog-fixture.exe'}
        hash {$invocation.ExpectedSha256='0'*64}
        escape {$invocation.EvidenceName='../escape'}
        variant {$invocation.Variant='unknown'}
        existing {New-Item -ItemType Directory -Path $out | Out-Null;[IO.File]::WriteAllText((Join-Path $out 'sentinel'),'synthetic sentinel')}
    }
    $rejected=$false
    try{& (Join-Path $PSScriptRoot 'test-watchdog-exit.ps1') @invocation}catch{$rejected=$true}
    if(-not $rejected){throw 'Watchdog harness guard accepted invalid input'}
    if($case -eq 'existing') {
        if(@(Get-ChildItem -LiteralPath $out).Count -ne 1 -or [IO.File]::ReadAllText((Join-Path $out 'sentinel')) -cne 'synthetic sentinel'){throw 'Existing evidence mutated'}
        Remove-Item -LiteralPath (Join-Path $out 'sentinel');Remove-Item -LiteralPath $out
    } elseif(Test-Path -LiteralPath $out){throw 'Preflight created output'}
    $count++
}
foreach($name in @('test-watchdog-exit.ps1','test-watchdog-exit-policy.ps1')) {
    $tokens=$null;$errors=$null
    [void][Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $name),[ref]$tokens,[ref]$errors)
    if($errors.Count){throw 'Invalid script AST'}
}
Write-Host "Watchdog harness $count preflight rejection and two AST checks passed."
