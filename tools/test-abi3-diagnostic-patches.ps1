[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$RuntimeBuildDirectory)
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'runtime-build/source-policy.ps1')
$base=(Resolve-Path -LiteralPath (Join-Path $RuntimeBuildDirectory 'inputs/librime')).Path
Assert-MoUserDbStartupPolicy $base
$out=Join-Path $repo ('build/mo-abi3-diagnostic-replay-'+[Guid]::NewGuid().ToString('N'))
$source=Join-Path $out 'source'
$patchDir=Join-Path $repo 'native/librime/diagnostics'
$patches=@('components-v3.patch','queries-v3.patch')
$names=@('src/rime/dict/level_db.cc')
foreach($patch in $patches){
    $text=Get-Content -LiteralPath (Join-Path $patchDir $patch) -Raw
    if($text -match 'Prefault|PREFAULT|reuse_logs|RepairDB|userdb_recovery_task'){throw 'Timing patch changes preparation or DB policy'}
    $names+=@([regex]::Matches($text,'(?m)^\+\+\+ b/(.+)$')|ForEach-Object{$_.Groups[1].Value.Trim()})
}
foreach($name in @($names|Sort-Object -Unique)){
    $target=Join-Path $source $name
    New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $base $name) -Destination $target
}
$relative='build/'+(Split-Path -Leaf $out)+'/source'
Push-Location $repo
try{
    foreach($patch in $patches){
        & git apply --check "--directory=$relative" (Join-Path $patchDir $patch)
        if($LASTEXITCODE -ne 0){throw 'Diagnostic patch check failed'}
        & git apply "--directory=$relative" (Join-Path $patchDir $patch)
        if($LASTEXITCODE -ne 0){throw 'Diagnostic patch application failed'}
        Assert-MoUserDbStartupPolicy $source
    }
}finally{Pop-Location}
Copy-Item -LiteralPath (Join-Path $patchDir 'mo_diagnostic.h') -Destination (Join-Path $source 'src/rime/mo_diagnostic.h')
Write-Host 'Both ABI3 timing patches applied; startup userdb protection preserved after each patch.'
Write-Host "Synthetic source replay retained at $out"
$global:LASTEXITCODE=0