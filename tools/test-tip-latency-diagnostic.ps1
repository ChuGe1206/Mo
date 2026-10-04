[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
$collector=Join-Path $PSScriptRoot 'tip-latency-diagnostic.ps1'
$pwsh=(Get-Process -Id $PID).Path
$fixture=Join-Path $repo ('build/mo-tip-latency-policy-'+[Guid]::NewGuid().ToString('N'))
$dist=Join-Path $fixture 'dist'
$shared=Join-Path $fixture 'shared'
$native=Join-Path $fixture 'native'
New-Item -ItemType Directory -Path "$dist/lib","$shared/build","$native/x64/Release","$native/Win32/Release" | Out-Null
foreach($path in @("$dist/lib/rime.dll","$fixture/mo-broker.exe","$shared/default.yaml","$shared/rime_ice.schema.yaml","$shared/build/default.yaml","$shared/build/rime_ice.schema.yaml","$native/x64/Release/mo_tip.dll","$native/x64/Release/mo_tip_abi_probe.exe","$native/Win32/Release/mo_tip.dll","$native/Win32/Release/mo_tip_abi_probe.exe")){
    [IO.File]::WriteAllText($path,'synthetic preflight only',[Text.UTF8Encoding]::new($false))
}
$count=0
function Reject-Case([string]$Name,[hashtable]$Overrides){
    $evidence='mo-tip-latency-reject-'+[Guid]::NewGuid().ToString('N')
    $argsMap=@{LibrimeDistDir=$dist;SharedDataDir=$shared;BrokerPath="$fixture/mo-broker.exe";NativeOutputDirectory=$native;EvidenceName=$evidence}
    foreach($key in $Overrides.Keys){$argsMap[$key]=$Overrides[$key]}
    $argsList=@('-NoProfile','-File',$collector)
    foreach($key in $argsMap.Keys){$argsList+=@('-'+$key,[string]$argsMap[$key])}
    $actualOutput=Join-Path $repo ('build/'+$argsMap.EvidenceName)
    $alreadyExists=Test-Path -LiteralPath $actualOutput
    & $pwsh @argsList *> (Join-Path $fixture ($Name+'.log'))
    if($LASTEXITCODE -eq 0){throw "Diagnostic preflight unexpectedly accepted $Name"}
    if(Test-Path -LiteralPath (Join-Path $repo ('build/'+$evidence))){throw 'Rejected diagnostic created evidence output'}
    if(-not $alreadyExists -and (Test-Path -LiteralPath $actualOutput)){throw 'Rejected diagnostic created its requested output'}
    $script:count++
}
Reject-Case 'relative-runtime' @{LibrimeDistDir='relative-dist'}
Reject-Case 'missing-runtime' @{LibrimeDistDir=(Join-Path $fixture 'missing')}
Reject-Case 'relative-shared' @{SharedDataDir='relative-shared'}
Reject-Case 'relative-broker' @{BrokerPath='mo-broker.exe'}
Reject-Case 'traversal-name' @{EvidenceName='../outside'}
Remove-Item -LiteralPath "$shared/build/default.yaml"
Reject-Case 'incomplete-prebuilt' @{}
[IO.File]::WriteAllText("$shared/build/default.yaml",'synthetic preflight only')
Remove-Item -LiteralPath "$native/Win32/Release/mo_tip_abi_probe.exe"
Reject-Case 'missing-win32-probe' @{}
$existing='mo-tip-latency-existing-'+[Guid]::NewGuid().ToString('N')
$existingRoot=Join-Path $repo ('build/'+$existing)
New-Item -ItemType Directory -Path $existingRoot | Out-Null
$marker=Join-Path $existingRoot 'keep.txt'
[IO.File]::WriteAllText($marker,'preserve existing evidence')
$before=(Get-FileHash $marker).Hash
Reject-Case 'existing-output' @{EvidenceName=$existing}
if((Get-FileHash $marker).Hash -ne $before -or @(Get-ChildItem $existingRoot).Count -ne 1){throw 'Existing evidence modified'}
$tokens=$null;$errors=$null
[void][Management.Automation.Language.Parser]::ParseFile($collector,[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'Collector AST failed'}
Write-Host "$count diagnostic refusal cases and collector AST passed. Synthetic fixtures retained."
$global:LASTEXITCODE=0