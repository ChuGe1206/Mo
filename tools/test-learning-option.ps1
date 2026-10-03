#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$RuntimeBuildDirectory,
    [Parameter(Mandatory)][string]$SharedDataDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidatePattern('^[A-Za-z0-9._-]+$')][string]$RustToolchain = '1.97.1'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
. (Join-Path $repo 'installer/windows/staging-policy.ps1')
$runtimeRoot = Assert-MoPlainPath $RuntimeBuildDirectory
$null = Assert-MoStageRuntime $runtimeRoot $repo
$shared = Assert-MoPlainPath $SharedDataDirectory
$output = Assert-MoNewBuildOutput $OutputDirectory $repo
$dll = Join-Path $runtimeRoot 'dist/lib/rime.dll'
$manager = Join-Path $runtimeRoot 'dist/bin/rime_dict_manager.exe'
foreach ($path in @($dll, $manager, (Join-Path $shared 'build/rime_ice.schema.yaml'))) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing learning probe input: $path" }
}
New-Item -ItemType Directory -Path $output | Out-Null
$user = Join-Path $output 'user'
New-Item -ItemType Directory -Path $user | Out-Null
Push-Location $repo
try {
    & cargo "+$RustToolchain" build --locked --offline -p mo-rime --example learning_smoke
    if ($LASTEXITCODE -ne 0) { throw 'Learning probe compilation failed.' }
} finally { Pop-Location }
$probe = Join-Path $repo 'target/debug/examples/learning_smoke.exe'
$originalPath = $env:Path
function Invoke-LearningProbe([bool]$Disable, [int]$ExpectedEntries) {
    $mode = if ($Disable) { 'true' } else { 'false' }
    & $probe $dll $shared $user $mode
    if ($LASTEXITCODE -ne 0) { throw "Learning probe failed with mo_disable_learning=$mode." }
    $entries = @()
    if (Test-Path -LiteralPath (Join-Path $user 'rime_ice.userdb') -PathType Container) {
        $export = Join-Path $output ("rime_ice-$mode-$ExpectedEntries.userdb.txt")
        Push-Location $user
        try {
            $env:Path = (Join-Path $runtimeRoot 'dist/lib') + ';' + $originalPath
            & $manager -e rime_ice $export
            if ($LASTEXITCODE -ne 0) { throw 'User dictionary export failed.' }
        } finally {
            $env:Path = $originalPath
            Pop-Location
        }
        $entries = @(Get-Content -LiteralPath $export | Where-Object { $_ -and -not $_.StartsWith('#') })
    }
    if ($entries.Count -ne $ExpectedEntries) {
        throw "Expected $ExpectedEntries learned entries with mo_disable_learning=$mode; got $($entries.Count)."
    }
    if ($ExpectedEntries -eq 1 -and $entries[0] -cne "你好`tni hao`t1") {
        throw 'Learned entry or frequency changed unexpectedly.'
    }
    Write-Host "PASS mo_disable_learning=$mode, learned entries=$ExpectedEntries"
}

Invoke-LearningProbe $true 0
Invoke-LearningProbe $false 1
Invoke-LearningProbe $true 1
Write-Host 'Real librime session learning gate passed: private -> normal -> private.'
