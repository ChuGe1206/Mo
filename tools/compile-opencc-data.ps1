[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$SharedDataDir,
    [Parameter(Mandatory = $true)][string]$CompilerPath,
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'opencc-data.ps1')
$source = Join-Path (Resolve-Path -LiteralPath $SharedDataDir).Path 'opencc'
$compiler = (Resolve-Path -LiteralPath $CompilerPath).Path
$output = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $output) { throw 'Refusing to overwrite an existing resource pack.' }
foreach ($entry in (Get-MoOpenccSourceHashes).GetEnumerator()) {
    if ((Get-FileHash -LiteralPath (Join-Path $source $entry.Key) -Algorithm SHA256).Hash -ne $entry.Value) {
        throw 'OpenCC input does not match the locked rime-ice sources.'
    }
}
New-Item -ItemType Directory -Path (Join-Path $output 'source') | Out-Null
foreach ($name in @('emoji.json', 'emoji.txt', 'others.txt')) {
    Copy-Item -LiteralPath (Join-Path $source $name) -Destination (Join-Path $output "source/$name")
}
foreach ($name in @('emoji', 'others')) {
    & $compiler (Join-Path $source "$name.txt") (Join-Path $output "$name.ocd2")
    if ($LASTEXITCODE -ne 0) { throw 'OpenCC compile/complete-entry round-trip verification failed; pack is incomplete.' }
}
$config = Get-Content -LiteralPath (Join-Path $source 'emoji.json') -Raw | ConvertFrom-Json
$config.segmentation.dict.type = 'ocd2'
$config.segmentation.dict.file = 'emoji.ocd2'
$config.conversion_chain[0].dict.dicts[0].type = 'ocd2'
$config.conversion_chain[0].dict.dicts[0].file = 'emoji.ocd2'
$config.conversion_chain[0].dict.dicts[1].type = 'ocd2'
$config.conversion_chain[0].dict.dicts[1].file = 'others.ocd2'
$config | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $output 'emoji.json') -Encoding utf8NoBOM
$outputs = [ordered]@{}
foreach ($name in @('emoji.json', 'emoji.ocd2', 'others.ocd2')) {
    $outputs[$name] = (Get-FileHash -LiteralPath (Join-Path $output $name) -Algorithm SHA256).Hash
}
# The manifest is written last; failures do not leave a valid/publishable pack.
[ordered]@{
    format = 1
    rime_ice_commit = '6810e8916d160498620a16fef2135956fecbd485'
    compiler_sha256 = (Get-FileHash -LiteralPath $compiler -Algorithm SHA256).Hash
    outputs = $outputs
} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $output 'manifest.json') -Encoding utf8NoBOM
[void](Assert-MoCompiledOpenccData $output)
Write-Host 'Compiled OpenCC resources verified. Original sources retained; this is not a distribution/license approval.'
