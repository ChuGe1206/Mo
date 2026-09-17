[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$OpenccDataDir,
    [Parameter(Mandatory = $true)][string]$CompilerPath
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'opencc-data.ps1')
$pack = Assert-MoCompiledOpenccData $OpenccDataDir
$compiler = (Resolve-Path -LiteralPath $CompilerPath).Path
$repoRoot = Split-Path -Parent $PSScriptRoot
$testRoot = Join-Path $repoRoot ('build/mo-opencc-test-墨-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null
$script:checks = 0
function Require([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
    $script:checks++
}
function Require-Rejected([scriptblock]$Action) {
    $rejected = $false
    try { & $Action | Out-Null } catch { $rejected = $true }
    Require $rejected 'Invalid OpenCC input was accepted.'
}
function Copy-TestPack([string]$Name) {
    $directory = Join-Path $testRoot $Name
    Copy-Item -LiteralPath $pack -Destination $directory -Recurse
    return $directory
}
try {
    Require ((Assert-MoCompiledOpenccData $pack) -eq $pack) 'Valid resource pack rejected.'
    $user = Join-Path $testRoot 'user'
    New-Item -ItemType Directory -Path $user | Out-Null
    Copy-MoCompiledOpenccData $pack $user
    Require ((Get-ChildItem -LiteralPath (Join-Path $user 'opencc') -File).Count -eq 3) 'Fixture did not get exactly the fixed output set.'
    Require-Rejected { Copy-MoCompiledOpenccData $pack $user }

    $corrupt = Copy-TestPack 'corrupt'
    Add-Content -LiteralPath (Join-Path $corrupt 'emoji.ocd2') -Value 'corruption'
    Require-Rejected { Assert-MoCompiledOpenccData $corrupt }
    $sourceCorrupt = Copy-TestPack 'source-corrupt'
    Add-Content -LiteralPath (Join-Path $sourceCorrupt 'source/emoji.txt') -Value 'corruption'
    Require-Rejected { Assert-MoCompiledOpenccData $sourceCorrupt }
    $missing = Copy-TestPack 'missing'
    Remove-Item -LiteralPath (Join-Path $missing 'manifest.json')
    Require-Rejected { Assert-MoCompiledOpenccData $missing }

    $order = Copy-TestPack 'order'
    $configPath = Join-Path $order 'emoji.json'
    $config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
    $first = $config.conversion_chain[0].dict.dicts[0]
    $config.conversion_chain[0].dict.dicts[0] = $config.conversion_chain[0].dict.dicts[1]
    $config.conversion_chain[0].dict.dicts[1] = $first
    $config | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $configPath -Encoding utf8NoBOM
    $manifestPath = Join-Path $order 'manifest.json'
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $manifest.outputs.'emoji.json' = (Get-FileHash -LiteralPath $configPath -Algorithm SHA256).Hash
    $manifest | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $manifestPath -Encoding utf8NoBOM
    Require-Rejected { Assert-MoCompiledOpenccData $order }

    $semantics = Copy-TestPack 'semantics'
    $configPath = Join-Path $semantics 'emoji.json'
    $config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
    $config | Add-Member -NotePropertyName 'unexpected_setting' -NotePropertyValue 'changed'
    $config | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $configPath -Encoding utf8NoBOM
    $manifestPath = Join-Path $semantics 'manifest.json'
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $manifest.outputs.'emoji.json' = (Get-FileHash -LiteralPath $configPath -Algorithm SHA256).Hash
    $manifest | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $manifestPath -Encoding utf8NoBOM
    Require-Rejected { Assert-MoCompiledOpenccData $semantics }

    $extra = Copy-TestPack 'extra'
    $manifestPath = Join-Path $extra 'manifest.json'
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $manifest.outputs | Add-Member -NotePropertyName '../escape.ocd2' -NotePropertyValue ('0' * 64)
    $manifest | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $manifestPath -Encoding utf8NoBOM
    Require-Rejected { Assert-MoCompiledOpenccData $extra }

    $dictionaryInput = Join-Path $testRoot '词典.txt'
    $binary = Join-Path $testRoot '词典.ocd2'
    @("a`tone two", "中国`t中國 🐉", "墨`t🖋️ 墨") | Set-Content -LiteralPath $dictionaryInput -Encoding utf8NoBOM
    & $compiler $dictionaryInput $binary
    Require ($LASTEXITCODE -eq 0) 'Unicode path/ordered multi-value round-trip failed.'
    $before = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash
    & $compiler $dictionaryInput $binary
    Require ($LASTEXITCODE -ne 0) 'Compiler overwrote a pre-existing dictionary.'
    Require ((Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash -eq $before) 'Existing dictionary changed after rejection.'
    & $compiler 'relative.txt' $binary
    Require ($LASTEXITCODE -ne 0) 'Relative dictionary path accepted.'
    & $compiler
    Require ($LASTEXITCODE -ne 0) 'Missing arguments accepted.'
    $duplicates = Join-Path $testRoot 'duplicates.txt'
    @("a`tone", "a`ttwo") | Set-Content -LiteralPath $duplicates -Encoding utf8NoBOM
    $invalidOutput = Join-Path $testRoot 'invalid.ocd2'
    & $compiler $duplicates $invalidOutput
    Require ($LASTEXITCODE -ne 0) 'Duplicate keys accepted.'
    Require (-not (Test-Path -LiteralPath $invalidOutput)) 'Rejected source created a binary.'
    & $compiler (Join-Path $testRoot 'absent.txt') $invalidOutput
    Require ($LASTEXITCODE -ne 0) 'Missing input accepted.'
    Require (-not (Test-Path -LiteralPath $invalidOutput)) 'Missing input created a binary.'
    Require-Rejected { & (Join-Path $PSScriptRoot 'compile-opencc-data.ps1') -SharedDataDir $testRoot -CompilerPath $compiler -OutputDirectory $pack }
    Require-Rejected { & (Join-Path $PSScriptRoot 'opencc-build/build.ps1') -SourceArchivePath $dictionaryInput }
    Write-Host "$script:checks OpenCC resource/compiler checks passed. No Windows input state changed."
} finally {
    $resolved = (Resolve-Path -LiteralPath $testRoot).Path
    $expectedPrefix = [IO.Path]::GetFullPath((Join-Path $repoRoot 'build')).TrimEnd('\') + '\'
    if (-not $resolved.StartsWith($expectedPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Refusing to clean a fixture outside the build root.'
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
