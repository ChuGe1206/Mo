[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$LibrimeDistDir,
    [Parameter(Mandatory = $true)][string]$SharedDataDir,
    [Parameter(Mandatory = $true)][string]$UserDataDir,
    [switch]$OmitEmoji,
    [string]$OpenccDataDir,
    [switch]$KeepResources,
    [switch]$PrepareResources
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'opencc-data.ps1')
if ($OmitEmoji -and $OpenccDataDir) { throw 'Ablation and compiled dictionaries must be tested separately.' }
if ($PrepareResources -and $OmitEmoji) { throw 'Preparation requires the intact locked schema.' }
if ($OpenccDataDir) { $OpenccDataDir = Assert-MoCompiledOpenccData $OpenccDataDir }
$dist = (Resolve-Path -LiteralPath $LibrimeDistDir).Path
$shared = (Resolve-Path -LiteralPath $SharedDataDir).Path
$user = (Resolve-Path -LiteralPath $UserDataDir).Path
$fixture = Join-Path $user ('mo-latency-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
try {
    Copy-Item -LiteralPath (Join-Path $user 'build') -Destination (Join-Path $fixture 'build') -Recurse
    if ($OpenccDataDir) { Copy-MoCompiledOpenccData $OpenccDataDir $fixture }
    if ($OmitEmoji) {
        # A controlled mechanical ablation ONLY in this fresh disposable copy.
        # Keep the original schema/shared inputs untouched. Not a product default.
        $schemaPath = Join-Path $fixture 'build/rime_ice.schema.yaml'
        $schema = [IO.File]::ReadAllText($schemaPath)
        $pattern = '(?m)^\s*- "simplifier@emoji"\r?\n'
        if ([regex]::Matches($schema, $pattern).Count -ne 1) { throw 'Expected one deployed emoji filter.' }
        [IO.File]::WriteAllText($schemaPath, [regex]::Replace($schema, $pattern, ''), [Text.UTF8Encoding]::new($false))
    }
    # Empty, test-owned marker; no user settings or dictionaries are edited.
    New-Item -ItemType File -Path (Join-Path $fixture 'mo-latency-fixture') | Out-Null
    Push-Location $repoRoot
    try {
        [string[]]$probeOptions = if ($PrepareResources) { @('--prepare-resources') } elseif ($KeepResources) { @('--keep-resources') } else { @() }
        & cargo +stable run --quiet -p mo-rime --example latency_probe -- (Join-Path $dist 'lib/rime.dll') $shared $fixture @probeOptions
        if ($LASTEXITCODE -ne 0) { throw "Direct latency probe failed: $LASTEXITCODE" }
    } finally { Pop-Location }
} finally {
    $resolved = (Resolve-Path -LiteralPath $fixture).Path
    if (-not $resolved.StartsWith($user.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Refusing to clean a latency fixture outside the supplied root.'
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
