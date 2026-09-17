# Build-time resource checks only; dot-sourcing performs no filesystem mutation.
function Get-MoOpenccSourceHashes {
    return [ordered]@{
        'emoji.json' = '68F054CD3AFA752E74C8D2DEF3313A805401E9C28FDECF7B4C9237687EE4AC20'
        'emoji.txt' = '10C73123042C719DBF2236AF5E651D3FCC934C295A3F9E24B16816A2A751D5BA'
        'others.txt' = 'A468EC9ACCFEC55EA4A466FDD4A9B27531A1F385D4ECC75746029045BBB60BB7'
    }
}
function Assert-MoCompiledOpenccData([string]$Directory) {
    $root = (Resolve-Path -LiteralPath $Directory).Path
    $manifest = Get-Content -LiteralPath (Join-Path $root 'manifest.json') -Raw | ConvertFrom-Json
    if ($manifest.format -ne 1 -or $manifest.rime_ice_commit -ne '6810e8916d160498620a16fef2135956fecbd485') {
        throw 'Unsupported compiled OpenCC manifest.'
    }
    foreach ($entry in (Get-MoOpenccSourceHashes).GetEnumerator()) {
        if ((Get-FileHash -LiteralPath (Join-Path $root "source/$($entry.Key)") -Algorithm SHA256).Hash -ne $entry.Value) {
            throw 'Compiled OpenCC pack does not retain the pinned source.'
        }
    }
    # Fixed allow-list, not paths provided by the manifest.
    $names = @('emoji.json', 'emoji.ocd2', 'others.ocd2')
    if (@($manifest.outputs.PSObject.Properties).Count -ne $names.Count) { throw 'Invalid output list.' }
    foreach ($name in $names) {
        $hash = $manifest.outputs.$name
        if ($hash -notmatch '^[A-Fa-f0-9]{64}$' -or
            (Get-FileHash -LiteralPath (Join-Path $root $name) -Algorithm SHA256).Hash -ne $hash) {
            throw "Compiled OpenCC output hash mismatch: $name"
        }
    }
    $config = Get-Content -LiteralPath (Join-Path $root 'emoji.json') -Raw | ConvertFrom-Json
    $expectedConfig = Get-Content -LiteralPath (Join-Path $root 'source/emoji.json') -Raw | ConvertFrom-Json
    $expectedConfig.segmentation.dict.type = 'ocd2'
    $expectedConfig.segmentation.dict.file = 'emoji.ocd2'
    $expectedConfig.conversion_chain[0].dict.dicts[0].type = 'ocd2'
    $expectedConfig.conversion_chain[0].dict.dicts[0].file = 'emoji.ocd2'
    $expectedConfig.conversion_chain[0].dict.dicts[1].type = 'ocd2'
    $expectedConfig.conversion_chain[0].dict.dicts[1].file = 'others.ocd2'
    if (($config | ConvertTo-Json -Depth 10 -Compress) -cne
        ($expectedConfig | ConvertTo-Json -Depth 10 -Compress)) {
        throw 'Compiled OpenCC configuration changed fields beyond dictionary type/file.'
    }
    if ($config.segmentation.type -ne 'mmseg' -or $config.segmentation.dict.type -ne 'ocd2' -or
        $config.segmentation.dict.file -ne 'emoji.ocd2' -or $config.conversion_chain.Count -ne 1 -or
        $config.conversion_chain[0].dict.type -ne 'group' -or $config.conversion_chain[0].dict.dicts.Count -ne 2 -or
        $config.conversion_chain[0].dict.dicts[0].type -ne 'ocd2' -or $config.conversion_chain[0].dict.dicts[0].file -ne 'emoji.ocd2' -or
        $config.conversion_chain[0].dict.dicts[1].type -ne 'ocd2' -or $config.conversion_chain[0].dict.dicts[1].file -ne 'others.ocd2') {
        throw 'Compiled OpenCC configuration changed dictionary order or conversion semantics.'
    }
    return $root
}
function Copy-MoCompiledOpenccData([string]$Directory, [string]$DisposableUser) {
    $root = Assert-MoCompiledOpenccData $Directory
    $destination = Join-Path $DisposableUser 'opencc'
    if (Test-Path -LiteralPath $destination) { throw 'Refusing to overwrite fixture OpenCC data.' }
    New-Item -ItemType Directory -Path $destination | Out-Null
    foreach ($name in @('emoji.json', 'emoji.ocd2', 'others.ocd2')) {
        Copy-Item -LiteralPath (Join-Path $root $name) -Destination (Join-Path $destination $name)
    }
}
