[CmdletBinding()]
param([Parameter(Mandatory)][string]$RuntimePath,[Parameter(Mandatory)][string]$ActorPath,[Parameter(Mandatory)][string]$BrokerPath,[Parameter(Mandatory)][string]$FixtureProbePath,[Parameter(Mandatory)][string]$SharedDataDir,[Parameter(Mandatory)][string]$CompiledDataDir)
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
$paths=@{runtime=$RuntimePath;actor=$ActorPath;broker=$BrokerPath;fixture=$FixtureProbePath}
$hashes=@{}; foreach($key in $paths.Keys){$hashes[$key]=(Get-FileHash -LiteralPath $paths[$key]).Hash}
$common=@{RuntimePath=$RuntimePath;ActorPath=$ActorPath;BrokerPath=$BrokerPath;FixtureProbePath=$FixtureProbePath;SharedDataDir=$SharedDataDir;CompiledDataDir=$CompiledDataDir;ExpectedHashes=$hashes}
$count=0
foreach($case in @('relative','missing','hash','name','existing')) {
    $args=$common.Clone(); $name='userdb-error-policy-'+[Guid]::NewGuid().ToString('N'); $args.EvidenceName=$name
    $out=Join-Path $repo "build/$name"
    switch($case) {
        relative {$args.ActorPath='actor.exe'}
        missing {$args.ActorPath=Join-Path $repo 'build/missing-userdb-probe.exe'}
        hash {$args.ExpectedHashes=$hashes.Clone(); $args.ExpectedHashes.actor='0'*64}
        name {$args.EvidenceName='../escape'}
        existing {New-Item -ItemType Directory -Path $out | Out-Null; [IO.File]::WriteAllText((Join-Path $out 'sentinel'),'synthetic sentinel')}
    }
    $rejected=$false
    try { & (Join-Path $PSScriptRoot 'test-userdb-errors.ps1') @args } catch { $rejected=$true }
    if (-not $rejected) {throw 'Harness guard accepted invalid input'}
    if($case -eq 'existing') {
        if([IO.File]::ReadAllText((Join-Path $out 'sentinel')) -cne 'synthetic sentinel' -or @(Get-ChildItem -LiteralPath $out).Count -ne 1){throw 'Existing evidence changed'}
        Remove-Item -LiteralPath (Join-Path $out 'sentinel'); Remove-Item -LiteralPath $out
    } elseif(Test-Path -LiteralPath $out){throw 'Preflight guard created output'}
    $count++
}
$fixture=Join-Path $repo ('build/userdb-native-policy-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
try {
    foreach($args in @(@('--seed',$fixture),@('--seed','relative'),@('--unknown',$fixture),@('--verify',$fixture,'extra'))) {
        & $FixtureProbePath @args *> $null
        if($LASTEXITCODE -eq 0 -or (Test-Path -LiteralPath (Join-Path $fixture 'rime_ice.userdb'))){throw 'Native guard failed'}
        $count++
    }
} finally { Remove-Item -LiteralPath $fixture }
foreach($script in @('test-userdb-errors.ps1','test-userdb-errors-policy.ps1')) {
    $tokens=$null;$errors=$null
    [void][Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $script),[ref]$tokens,[ref]$errors)
    if($errors.Count){throw 'Script AST invalid'}
}
Write-Host "User DB error policy $count guards and two AST checks passed."
