# Build-time consistency checks, not a signature/trust or hostile-filesystem boundary.
# Dot-sourcing this file performs no filesystem mutation.
Set-StrictMode -Version Latest

function Assert-MoRelativeName([string]$Name) {
    if ($Name.Length -gt 240 -or $Name -cnotmatch '^[A-Za-z0-9_-][A-Za-z0-9_.-]*(/[A-Za-z0-9_-][A-Za-z0-9_.-]*)*$') {
        throw 'Invalid staging relative path.'
    }
    foreach ($part in $Name.Split('/')) {
        if ($part.EndsWith('.') -or $part -match '^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(\.|$)') {
            throw 'Reserved staging relative path.'
        }
    }
}

function Assert-MoPlainPath([string]$Path, [switch]$MayNotExist) {
    if (-not [IO.Path]::IsPathFullyQualified($Path)) { throw 'Staging paths must be absolute.' }
    $full = [IO.Path]::GetFullPath($Path)
    if ($full -notmatch '^[A-Za-z]:\\' -or $full.Substring(2).Contains(':')) { throw 'Only local DOS paths are supported.' }
    $current = [IO.Path]::GetPathRoot($full)
    foreach ($part in $full.Substring($current.Length).Split('\', [StringSplitOptions]::RemoveEmptyEntries)) {
        $current = Join-Path $current $part
        if (Test-Path -LiteralPath $current) {
            $item = Get-Item -LiteralPath $current -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Staging paths must not traverse reparse points.' }
        } elseif (-not $MayNotExist) { throw 'Staging path does not exist.' }
    }
    return $full.TrimEnd('\')
}

function Assert-MoNewBuildOutput([string]$Path, [string]$Repository) {
    $full = Assert-MoPlainPath $Path -MayNotExist
    $build = Assert-MoPlainPath (Join-Path $Repository 'build') -MayNotExist
    if (-not $full.StartsWith($build + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Staging output must be a new child of repository build.'
    }
    if (Test-Path -LiteralPath $full) { throw 'Refusing to overwrite staging output.' }
    return $full
}

function Resolve-MoPinnedSourceInput(
    [string]$SourceDirectory,
    [string]$ArchivePath,
    [string]$ExpectedArchiveSha256
) {
    if ($ExpectedArchiveSha256 -cnotmatch '^[A-Fa-f0-9]{64}$') {
        throw 'Pinned source archive hash is invalid.'
    }
    $hasSource = -not [string]::IsNullOrWhiteSpace($SourceDirectory)
    $hasArchive = -not [string]::IsNullOrWhiteSpace($ArchivePath)
    if ($hasSource -eq $hasArchive) {
        throw 'Specify exactly one pinned source checkout or source archive.'
    }
    if ($hasSource) {
        $source = Assert-MoPlainPath $SourceDirectory
        if (-not (Test-Path -LiteralPath $source -PathType Container)) {
            throw 'Pinned source checkout is not a directory.'
        }
        return [pscustomobject]@{ Kind = 'Checkout'; Path = $source }
    }
    $archive = Assert-MoPlainPath $ArchivePath
    if (-not (Test-Path -LiteralPath $archive -PathType Leaf) -or
        (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ine $ExpectedArchiveSha256) {
        throw 'Pinned source archive mismatch.'
    }
    return [pscustomobject]@{ Kind = 'Archive'; Path = $archive }
}

function Get-MoStageFiles([string]$Directory) {
    $root = Assert-MoPlainPath $Directory
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { throw 'Expected a staging directory.' }
    $pending = [Collections.Generic.Stack[string]]::new()
    $pending.Push($root)
    $files = [Collections.Generic.List[string]]::new()
    while ($pending.Count) {
        foreach ($entry in Get-ChildItem -LiteralPath $pending.Pop() -Force) {
            if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse point in staging inventory.' }
            $relative = [IO.Path]::GetRelativePath($root, $entry.FullName).Replace('\', '/')
            Assert-MoRelativeName $relative
            if ($entry.PSIsContainer) {
                if (@(Get-ChildItem -LiteralPath $entry.FullName -Force).Count -eq 0) { throw 'Empty directory in staging inventory.' }
                $pending.Push($entry.FullName)
            } else { $files.Add($relative) }
        }
    }
    $result = $files.ToArray()
    [Array]::Sort($result, [StringComparer]::Ordinal)
    return $result
}

function Get-MoStageInventory([string]$Directory, [string[]]$Exclude = @()) {
    $inventory = [ordered]@{}
    foreach ($name in Get-MoStageFiles $Directory) {
        if ($name -cin $Exclude) { continue }
        $file = Get-Item -LiteralPath (Join-Path $Directory $name) -Force
        $inventory[$name] = [ordered]@{ size = $file.Length; sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash }
    }
    return $inventory
}

function Assert-MoInventory([string]$Directory, [Collections.IDictionary]$Expected, [string[]]$Exclude = @()) {
    if ($Expected.Count -eq 0 -or $Expected.Count -gt 4096) { throw 'Invalid staging inventory count.' }
    $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $exactNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($entry in $Expected.GetEnumerator()) {
        Assert-MoRelativeName $entry.Key
        if (-not $names.Add($entry.Key) -or $entry.Key -cin $Exclude) { throw 'Duplicate/reserved staging inventory path.' }
        [void]$exactNames.Add($entry.Key)
        $value = $entry.Value
        if ($value -isnot [Collections.IDictionary] -or $value.Count -ne 2 -or
            ($value['size'] -isnot [long] -and $value['size'] -isnot [int]) -or
            $value['size'] -lt 0 -or $value['size'] -gt 268435456 -or
            $value['sha256'] -isnot [string] -or $value['sha256'] -cnotmatch '^[A-Fa-f0-9]{64}$') { throw 'Invalid staging inventory entry.' }
    }
    $actual = Get-MoStageInventory $Directory $Exclude
    if ($actual.Count -ne $Expected.Count) { throw 'Staging file count mismatch.' }
    foreach ($entry in $actual.GetEnumerator()) {
        $expectedEntry = $Expected[$entry.Key]
        if (-not $exactNames.Contains($entry.Key) -or $null -eq $expectedEntry -or $entry.Value.size -ne $expectedEntry['size'] -or
            $entry.Value.sha256 -ine $expectedEntry['sha256']) { throw 'Staging file hash/size mismatch.' }
    }
}

function Assert-MoJsonElement([System.Text.Json.JsonElement]$Element) {
    switch ($Element.ValueKind) {
        'Object' {
            $keys = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
            foreach ($property in $Element.EnumerateObject()) {
                if ($property.Name.Contains([char]0) -or -not $keys.Add($property.Name)) { throw 'Duplicate/NUL JSON property.' }
                Assert-MoJsonElement $property.Value
            }
        }
        'Array' { foreach ($child in $Element.EnumerateArray()) { Assert-MoJsonElement $child } }
        'String' { if ($Element.GetString().Contains([char]0)) { throw 'NUL JSON string.' } }
    }
}

function Read-MoStageJson([string]$Path) {
    $full = Assert-MoPlainPath $Path
    $item = Get-Item -LiteralPath $full -Force
    if ($item.PSIsContainer -or $item.Length -gt 1048576) { throw 'Staging metadata exceeds 1 MiB.' }
    $text = [Text.UTF8Encoding]::new($false, $true).GetString([IO.File]::ReadAllBytes($full))
    $options = [System.Text.Json.JsonDocumentOptions]::new()
    $options.MaxDepth = 32
    $document = [System.Text.Json.JsonDocument]::Parse($text, $options)
    try {
        if ($document.RootElement.ValueKind -ne 'Object') { throw 'Staging metadata must be a JSON object.' }
        Assert-MoJsonElement $document.RootElement
    } finally { $document.Dispose() }
    return ($text | ConvertFrom-Json -AsHashtable -Depth 32)
}

function Assert-MoDevelopmentMetadata([Collections.IDictionary]$Metadata, [int]$Format) {
    if (($Metadata['format'] -isnot [int] -and $Metadata['format'] -isnot [long]) -or
        $Metadata['format'] -ne $Format -or $Metadata['development_only'] -isnot [bool] -or
        $Metadata['development_only'] -ne $true -or $Metadata['redistributable'] -isnot [bool] -or
        $Metadata['redistributable'] -ne $false) { throw 'Only non-redistributable development metadata is supported.' }
}

function Assert-MoPeArchitecture([string]$Path, [ValidateSet('x64', 'x86')][string]$Architecture, [bool]$Dll) {
    $full = Assert-MoPlainPath $Path
    $stream = [IO.File]::Open($full, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $reader = [IO.BinaryReader]::new($stream)
    try {
        if ($stream.Length -lt 256 -or $reader.ReadUInt16() -ne 0x5a4d) { throw 'Not a PE image.' }
        $stream.Position = 0x3c
        $offset = $reader.ReadUInt32()
        if ($offset -lt 64 -or $offset -gt $stream.Length - 24) { throw 'Invalid PE header offset.' }
        $stream.Position = $offset
        if ($reader.ReadUInt32() -ne 0x4550) { throw 'Invalid PE signature.' }
        $machine = $reader.ReadUInt16()
        $stream.Position = $offset + 20
        $optionalSize = $reader.ReadUInt16()
        $characteristics = $reader.ReadUInt16()
        $magic = $reader.ReadUInt16()
        $expectedMachine = if ($Architecture -eq 'x64') { 0x8664 } else { 0x14c }
        $expectedMagic = if ($Architecture -eq 'x64') { 0x20b } else { 0x10b }
        if ($machine -ne $expectedMachine -or $magic -ne $expectedMagic -or $optionalSize -lt 96 -or
            $offset + 24 + $optionalSize -gt $stream.Length -or
            -not ($characteristics -band 2) -or [bool]($characteristics -band 0x2000) -ne $Dll) { throw 'PE architecture/kind mismatch.' }
    } finally { $reader.Dispose() }
}

function Get-MoPeRvaOffset([byte[]]$Bytes, [int]$SectionOffset, [int]$SectionCount,
        [uint32]$Rva, [int]$Length) {
    for ($index = 0; $index -lt $SectionCount; $index++) {
        $section = $SectionOffset + 40 * $index
        $virtualSize = [BitConverter]::ToUInt32($Bytes, $section + 8)
        $virtualAddress = [BitConverter]::ToUInt32($Bytes, $section + 12)
        $rawSize = [BitConverter]::ToUInt32($Bytes, $section + 16)
        $rawOffset = [BitConverter]::ToUInt32($Bytes, $section + 20)
        $span = [Math]::Max($virtualSize, $rawSize)
        if ([uint64]$Rva -ge $virtualAddress -and [uint64]$Rva -lt [uint64]$virtualAddress + $span) {
            $within = [uint64]$Rva - $virtualAddress
            if ($within + $Length -gt $rawSize -or [uint64]$rawOffset + $within + $Length -gt $Bytes.Length) {
                throw 'PE import points outside section bytes.'
            }
            return [int]($rawOffset + $within)
        }
    }
    throw 'PE import RVA does not map to a section.'
}

function Get-MoPeImportedDlls([string]$Path) {
    $bytes = [IO.File]::ReadAllBytes((Assert-MoPlainPath $Path))
    if ($bytes.Length -lt 256 -or [BitConverter]::ToUInt16($bytes, 0) -ne 0x5a4d) { throw 'Not a PE image.' }
    $pe = [BitConverter]::ToUInt32($bytes, 0x3c)
    if ($pe -lt 64 -or [uint64]$pe + 24 -gt $bytes.Length -or
        [BitConverter]::ToUInt32($bytes, [int]$pe) -ne 0x4550) { throw 'Invalid PE header.' }
    $sections = [BitConverter]::ToUInt16($bytes, [int]$pe + 6)
    $optionalSize = [BitConverter]::ToUInt16($bytes, [int]$pe + 20)
    $optional = [int]$pe + 24
    $sectionOffset = $optional + $optionalSize
    if ($sections -lt 1 -or $sections -gt 96 -or $sectionOffset + 40 * $sections -gt $bytes.Length) {
        throw 'Invalid PE sections.'
    }
    $magic = [BitConverter]::ToUInt16($bytes, $optional)
    $directoryOffset = switch ($magic) { 0x10b { 96 } 0x20b { 112 } default { throw 'Invalid PE optional header.' } }
    if ($optionalSize -lt $directoryOffset + 16 * 2) { throw 'Missing PE import directory.' }
    $directoryCount = [BitConverter]::ToUInt32($bytes, $optional + $directoryOffset - 4)
    if ($directoryCount -lt 2) { throw 'Missing PE import directory.' }
    $names = [Collections.Generic.List[string]]::new()
    foreach ($kind in @(@(1, 20, 12), @(13, 32, 4))) {
        $index, $recordSize, $nameField = $kind
        if ($directoryCount -le $index) { continue }
        if ($optionalSize -lt $directoryOffset + 8 * ($index + 1)) { throw 'Truncated PE data directories.' }
        $directory = $optional + $directoryOffset + 8 * $index
        $rva = [BitConverter]::ToUInt32($bytes, $directory)
        $size = [BitConverter]::ToUInt32($bytes, $directory + 4)
        if (($rva -eq 0) -ne ($size -eq 0)) { throw 'Incomplete PE import directory.' }
        if ($rva -eq 0) { continue }
        if ($size -lt $recordSize -or $size -gt 1MB) { throw 'Invalid PE import directory size.' }
        $terminated = $false
        for ($position = 0; $position + $recordSize -le $size; $position += $recordSize) {
            $entry = Get-MoPeRvaOffset $bytes $sectionOffset $sections ([uint32]($rva + $position)) $recordSize
            $empty = $true
            for ($byte = 0; $byte -lt $recordSize; $byte++) {
                if ($bytes[$entry + $byte] -ne 0) { $empty = $false; break }
            }
            if ($empty) { $terminated = $true; break }
            if ($index -eq 13 -and ([BitConverter]::ToUInt32($bytes, $entry) -band 1) -eq 0) {
                throw 'Unsupported VA-based PE delay import.'
            }
            $nameRva = [BitConverter]::ToUInt32($bytes, $entry + $nameField)
            $nameOffset = Get-MoPeRvaOffset $bytes $sectionOffset $sections $nameRva 1
            $end = $nameOffset
            while ($end -lt $bytes.Length -and $end -lt $nameOffset + 256 -and $bytes[$end] -ne 0) { $end++ }
            if ($end -eq $nameOffset -or $end -eq $bytes.Length -or $end -eq $nameOffset + 256) {
                throw 'Invalid PE import name.'
            }
            $null = Get-MoPeRvaOffset $bytes $sectionOffset $sections $nameRva ($end - $nameOffset + 1)
            $name = [Text.Encoding]::ASCII.GetString($bytes, $nameOffset, $end - $nameOffset)
            if ($name -cnotmatch '^[A-Za-z0-9_.-]+\.dll$') { throw 'Invalid PE import DLL name.' }
            $names.Add($name)
        }
        if (-not $terminated) { throw 'Unterminated PE import directory.' }
    }
    return $names.ToArray()
}

function Assert-MoNoDynamicVCRuntime([string]$Path) {
    $imports = @(Get-MoPeImportedDlls $Path)
    foreach ($name in $imports) {
        if ($name -match '^(?i:(?:(?:vcruntime|msvcp|msvcr|concrt|vcomp)\d+(?:_\d+)?d?|mfc\d+\w*))\.dll$') {
            throw "Staged PE depends on a Visual C++ redistributable DLL: $name"
        }
    }
}

function Get-MoRuntimePins {
    return [ordered]@{
        '' = @('33e78140250125871856cdc5b42ddc6a5fcd3cd4', 'B22594E1FCF55DF5BBF60E76DC49200671410E98540BE265F68465D03678C722')
        'deps/leveldb' = @('99b3c03b3284f5886f9ef9a4ef703d57373e61be', 'FE47E88AE4D2B1162209BF65C74D203753D2D5F105262192B9DAC41841E6C0B8')
        'deps/marisa-trie' = @('3e87d53b78e15f2f43783d5e376561a8c9722051', '8F557F1171ABF1D205B81F9C40B3E09F90BBE2E664CBA3172ECF9D603E406CD8')
        'deps/opencc' = @('556ed22496d650bd0b13b6c163be9814637970ae', 'C46A6130BC85F09B64A144E6BB316374E1B919D07055B7C5FAD870983495A60E')
        'deps/yaml-cpp' = @('2f86d13775d119edbb69af52e5f566fd65c6953b', '460796C79164719FFDBE7E57D0AB8E0C0A68990F8EB2814A56734C8415F8C59C')
        'plugins/lua' = @('ec52e48ea18f11af37717a01c337f853215cf70b', 'F13438CFA7AE8E64D722D89F9A76A05206CB18AEC5B34DB6E6A0546BBEE1A640')
    }
}

function Get-MoRuntimeResourceNames {
    return @('emoji.json', 'emoji.ocd2', 'hk2s.json', 'hk2t.json', 'HKVariants.ocd2', 'HKVariantsRev.ocd2',
        'HKVariantsRevPhrases.ocd2', 'jp2t.json', 'JPShinjitaiCharacters.ocd2', 'JPShinjitaiPhrases.ocd2',
        'JPVariants.ocd2', 'JPVariantsRev.ocd2', 'others.ocd2', 's2hk.json', 's2t.json', 's2tw.json', 's2twp.json',
        'STCharacters.ocd2', 'STPhrases.ocd2', 't2hk.json', 't2jp.json', 't2s.json', 't2tw.json',
        'TSCharacters.ocd2', 'TSPhrases.ocd2', 'tw2s.json', 'tw2sp.json', 'tw2t.json', 'TWPhrases.ocd2',
        'TWPhrasesRev.ocd2', 'TWVariants.ocd2', 'TWVariantsRev.ocd2', 'TWVariantsRevPhrases.ocd2')
}

function Get-MoRuntimeOwnSourceNames {
    return @('tools/runtime-build/build.ps1', 'tools/runtime-build/source-policy.ps1', 'tools/opencc-data.ps1',
        'native/librime/preparation/resources-v2.patch', 'native/librime/preparation/opencc-directory.patch',
        'native/librime/preparation/lua-signed-stack.patch', 'native/librime/preparation/lua-machine-data-only.patch',
        'native/librime/preparation/mo-learning-option.patch', 'native/librime/preparation/mo-lua-learning-option.patch',
        'native/librime/preparation/userdb-preserve.patch',
        'native/librime/preparation/mo_preparation.cc', 'native/librime/preparation/mo_project.cmake',
        'native/librime/preparation/mo_resource_directory.cpp', 'native/librime/preparation/mo_resource_file.h',
        'native/librime/preparation/mo_resource_file.cpp')
}

function Assert-MoStageRuntime([string]$BuildDirectory, [string]$Repository) {
    $root = Assert-MoPlainPath $BuildDirectory
    $dist = Join-Path $root 'dist'
    $metadata = Read-MoStageJson (Join-Path $dist 'mo-build-provenance.json')
    Assert-MoDevelopmentMetadata $metadata 2
    if ($metadata['preparation_abi'] -ne 3 -or $metadata['resource_directory'] -cne 'lib/opencc' -or
        $metadata['lua_data_policy'] -cne 'machine-shared-only-v1' -or
        $metadata['userdb_policy'] -cne 'strict-open-no-auto-recovery-v1' -or
        $metadata['learning_policy'] -cne 'session-option-v1' -or
        @($metadata['plugins']).Count -ne 1 -or $metadata['plugins'][0] -cne 'lua') { throw 'Runtime ABI/plugin policy mismatch.' }
    $pins = Get-MoRuntimePins
    if ($metadata['inputs'].Count -ne $pins.Count) { throw 'Runtime source pin count mismatch.' }
    foreach ($pin in $pins.GetEnumerator()) {
        $actual = $metadata['inputs'][$pin.Key]
        $archive = Join-Path $root ('inputs/' + ($pin.Key -replace '/', '-') + 'source.tar')
        if ($null -eq $actual -or $actual['commit'] -cne $pin.Value[0] -or
            $actual['archive_sha256'] -ine $pin.Value[1] -or
            (Get-FileHash -LiteralPath (Assert-MoPlainPath $archive)).Hash -ine $pin.Value[1]) { throw 'Runtime source archive/pin mismatch.' }
    }
    $ownFiles = Get-MoRuntimeOwnSourceNames
    if ($metadata['mo_inputs'].Count -ne $ownFiles.Count) { throw 'Runtime own-source inventory mismatch.' }
    foreach ($name in $ownFiles) {
        $hash = $metadata['mo_inputs'][$name]
        foreach ($path in @((Join-Path $Repository $name), (Join-Path $root "inputs/mo-runtime/$name"))) {
            if ($hash -isnot [string] -or (Get-FileHash -LiteralPath (Assert-MoPlainPath $path)).Hash -ine $hash) { throw 'Runtime own-source snapshot mismatch.' }
        }
    }
    if ($metadata['lua_data_policy_patch_sha256'] -ine $metadata['mo_inputs']['native/librime/preparation/lua-machine-data-only.patch']) {
        throw 'Runtime Lua data-policy patch binding mismatch.'
    }
    foreach ($binding in @(
        @('userdb_policy_patch_sha256', 'native/librime/preparation/userdb-preserve.patch'),
        @('learning_policy_patch_sha256', 'native/librime/preparation/mo-learning-option.patch'),
        @('lua_learning_policy_patch_sha256', 'native/librime/preparation/mo-lua-learning-option.patch'))) {
        if ($metadata[$binding[0]] -ine $metadata['mo_inputs'][$binding[1]]) {
            throw 'Runtime policy patch binding mismatch.'
        }
    }
    $names = Get-MoRuntimeResourceNames
    $resources = Join-Path $dist 'lib/opencc'
    $files = @(Get-MoStageFiles $resources)
    if ($files.Count -ne $names.Count -or $metadata['resources'].Count -ne $names.Count) { throw 'Runtime OpenCC inventory mismatch.' }
    foreach ($name in $names) {
        if ($name -cnotin $files -or (Get-FileHash -LiteralPath (Join-Path $resources $name)).Hash -ine $metadata['resources'][$name]) { throw 'Runtime OpenCC hash mismatch.' }
    }
    $dll = Join-Path $dist 'lib/rime.dll'
    if ((Get-FileHash -LiteralPath (Assert-MoPlainPath $dll)).Hash -ine $metadata['dll_sha256']) { throw 'Runtime DLL hash mismatch.' }
    Assert-MoPeArchitecture $dll x64 $true
    $header = Join-Path $dist 'include/rime_api.h'
    if ((Get-FileHash -LiteralPath (Assert-MoPlainPath $header)).Hash -ine '85CAF744B4E5405A9A1DE9C7AEF3AFFC4AE315F4AE5D7EBDD08E191A2C16DAD4') { throw 'Runtime public ABI header mismatch.' }
    return $metadata
}

function Get-MoExpectedPrebuiltNames {
    $names = [Collections.Generic.List[string]]::new()
    $names.Add('default.yaml')
    foreach ($schema in @('double_pinyin', 'double_pinyin_abc', 'double_pinyin_flypy', 'double_pinyin_jiajia',
        'double_pinyin_mspy', 'double_pinyin_sogou', 'double_pinyin_ziguang', 't9')) {
        $names.Add("$schema.schema.yaml"); $names.Add("$schema.prism.bin")
    }
    foreach ($schema in @('melt_eng', 'radical_pinyin', 'rime_ice')) {
        foreach ($suffix in @('schema.yaml', 'prism.bin', 'reverse.bin', 'table.bin')) { $names.Add("$schema.$suffix") }
    }
    return $names.ToArray()
}

function Assert-MoPrebuiltData([string]$Directory) {
    $actual = @(Get-MoStageFiles $Directory)
    $expected = Get-MoExpectedPrebuiltNames
    if ($actual.Count -ne $expected.Count) { throw 'Prebuilt data inventory count mismatch.' }
    foreach ($name in $expected) {
        if ($name -cnotin $actual -or (Get-Item -LiteralPath (Join-Path $Directory $name)).Length -eq 0) { throw 'Missing/empty prebuilt dictionary or config.' }
    }
}

function Assert-MoStagePayloadNames([string[]]$Names) {
    foreach ($required in @('bin/mo-broker.exe', 'bin/mo-settings.exe', 'bin/mo-tip-registrar.exe', 'tip/x64/mo-tip.dll',
        'tip/x86/mo-tip.dll', 'runtime/librime/rime.dll', 'data/rime-ice/default.yaml', 'data/rime-ice/rime_ice.schema.yaml')) {
        if ($required -cnotin $Names) { throw 'Required staging payload file is missing.' }
    }
    foreach ($name in $Names) {
        if ($name -cin @('bin/mo-broker.exe', 'bin/mo-settings.exe', 'bin/mo-tip-registrar.exe', 'tip/x64/mo-tip.dll', 'tip/x86/mo-tip.dll', 'runtime/librime/rime.dll')) { continue }
        if ($name.StartsWith('runtime/librime/opencc/', [StringComparison]::Ordinal) -and
            $name.Substring(23) -cin (Get-MoRuntimeResourceNames)) { continue }
        if ($name -cmatch '^data/rime-ice/(build/[A-Za-z0-9_.-]+|[A-Za-z0-9_.-]+\.yaml|custom_phrase\.txt|cn_dicts/[A-Za-z0-9_.-]+\.yaml|en_dicts/[A-Za-z0-9_.-]+\.(yaml|txt)|lua/[A-Za-z0-9_./-]+\.(lua|db))$') { continue }
        throw 'Unexpected staging payload file.'
    }
}

function Assert-MoPreparedStage([string]$Directory, [ValidateSet('mo-stage.json', 'mo-stage.pending.json')][string]$ManifestName = 'mo-stage.json') {
    $root = Assert-MoPlainPath $Directory
    $metadata = Read-MoStageJson (Join-Path $root $ManifestName)
    Assert-MoDevelopmentMetadata $metadata 1
    if ($metadata['kind'] -cne 'mo-windows-development-stage' -or $metadata['installable'] -isnot [bool] -or
        $metadata['installable'] -ne $false -or $metadata['payload_root'] -cne 'payload/Mo' -or
        $metadata['rime_ice_commit'] -cne '6810e8916d160498620a16fef2135956fecbd485' -or
        $metadata['rime_ice_archive_sha256'] -ine 'CD1895FBC961131A62F23277F636C27A6FB941DAC66DAF43C4A10D4E9E6ADAD3') { throw 'Unsupported staging metadata.' }
    Assert-MoInventory $root $metadata['files'] @($ManifestName)
    $evidenceNames = @('runtime-provenance.json', 'rime-data.json', 'rime-ice-source.tar', 'build-receipt.json',
        'rime-ice-LICENSE', 'rime-ice-Credits.md')
    foreach ($name in $metadata['files'].Keys) {
        if ($name.StartsWith('payload/Mo/', [StringComparison]::Ordinal)) { continue }
        if (-not $name.StartsWith('evidence/', [StringComparison]::Ordinal) -or $name.Substring(9) -cnotin $evidenceNames) {
            throw 'Unexpected staging evidence file.'
        }
    }
    foreach ($name in $evidenceNames) {
        if (-not $metadata['files'].Contains("evidence/$name")) { throw 'Missing staging evidence.' }
    }
    $payload = Join-Path $root 'payload/Mo'
    $names = @(Get-MoStageFiles $payload)
    Assert-MoStagePayloadNames $names
    Assert-MoPrebuiltData (Join-Path $payload 'data/rime-ice/build')
    foreach ($image in @(@('bin/mo-broker.exe', 'x64', $false), @('bin/mo-settings.exe', 'x64', $false),
        @('bin/mo-tip-registrar.exe', 'x64', $false),
        @('tip/x64/mo-tip.dll', 'x64', $true), @('tip/x86/mo-tip.dll', 'x86', $true), @('runtime/librime/rime.dll', 'x64', $true))) {
        Assert-MoPeArchitecture (Join-Path $payload $image[0]) $image[1] $image[2]
        Assert-MoNoDynamicVCRuntime (Join-Path $payload $image[0])
    }
    $archive = Join-Path $root 'evidence/rime-ice-source.tar'
    if ((Get-FileHash -LiteralPath $archive).Hash -ine $metadata['rime_ice_archive_sha256']) { throw 'Staged rime-ice source archive mismatch.' }
    $runtime = Read-MoStageJson (Join-Path $root 'evidence/runtime-provenance.json')
    Assert-MoDevelopmentMetadata $runtime 2
    if ($runtime['preparation_abi'] -ne 3 -or $runtime['resource_directory'] -cne 'lib/opencc' -or
        $runtime['lua_data_policy'] -cne 'machine-shared-only-v1' -or
        $runtime['userdb_policy'] -cne 'strict-open-no-auto-recovery-v1' -or
        $runtime['learning_policy'] -cne 'session-option-v1' -or
        @($runtime['plugins']).Count -ne 1 -or $runtime['plugins'][0] -cne 'lua' -or
        (Get-FileHash -LiteralPath (Join-Path $payload 'runtime/librime/rime.dll')).Hash -ine $runtime['dll_sha256']) { throw 'Staged runtime contract mismatch.' }
    $pins = Get-MoRuntimePins
    if ($runtime['inputs'].Count -ne $pins.Count) { throw 'Staged runtime source pin count mismatch.' }
    foreach ($pin in $pins.GetEnumerator()) {
        $input = $runtime['inputs'][$pin.Key]
        if ($null -eq $input -or $input['commit'] -cne $pin.Value[0] -or $input['archive_sha256'] -ine $pin.Value[1]) {
            throw 'Staged runtime source pin mismatch.'
        }
    }
    $runtimeOwnFiles = Get-MoRuntimeOwnSourceNames
    if ($runtime['mo_inputs'].Count -ne $runtimeOwnFiles.Count) { throw 'Staged runtime own-source inventory mismatch.' }
    foreach ($name in $runtimeOwnFiles) {
        if ($runtime['mo_inputs'][$name] -isnot [string] -or $runtime['mo_inputs'][$name] -cnotmatch '^[A-Fa-f0-9]{64}$') {
            throw 'Staged runtime own-source hash mismatch.'
        }
    }
    if ($runtime['lua_data_policy_patch_sha256'] -ine $runtime['mo_inputs']['native/librime/preparation/lua-machine-data-only.patch']) {
        throw 'Staged runtime Lua data-policy patch binding mismatch.'
    }
    foreach ($binding in @(
        @('userdb_policy_patch_sha256', 'native/librime/preparation/userdb-preserve.patch'),
        @('learning_policy_patch_sha256', 'native/librime/preparation/mo-learning-option.patch'),
        @('lua_learning_policy_patch_sha256', 'native/librime/preparation/mo-lua-learning-option.patch'))) {
        if ($runtime[$binding[0]] -ine $runtime['mo_inputs'][$binding[1]]) {
            throw 'Staged runtime policy patch binding mismatch.'
        }
    }
    $resources = Get-MoRuntimeResourceNames
    if ($runtime['resources'].Count -ne $resources.Count) { throw 'Staged runtime resource count mismatch.' }
    foreach ($name in $resources) {
        if ((Get-FileHash -LiteralPath (Join-Path $payload "runtime/librime/opencc/$name")).Hash -ine $runtime['resources'][$name]) {
            throw 'Staged runtime resource hash mismatch.'
        }
    }
    $data = Read-MoStageJson (Join-Path $root 'evidence/rime-data.json')
    Assert-MoDevelopmentMetadata $data 1
    if ($data['rime_ice_commit'] -cne $metadata['rime_ice_commit'] -or
        $data['source_archive_sha256'] -ine $metadata['rime_ice_archive_sha256'] -or
        $data['runtime_dll_sha256'] -ine $runtime['dll_sha256']) { throw 'Staged data build dependency mismatch.' }
    Assert-MoInventory (Join-Path $payload 'data/rime-ice') $data['outputs']
    $receipt = Read-MoStageJson (Join-Path $root 'evidence/build-receipt.json')
    Assert-MoDevelopmentMetadata $receipt 1
    if ($receipt['rust_target'] -cne 'x86_64-pc-windows-msvc' -or $receipt['rust_profile'] -cne 'release' -or
        $receipt['debug_assertions'] -isnot [bool] -or $receipt['debug_assertions'] -ne $false -or
        $receipt['latency_trace'] -isnot [bool] -or $receipt['latency_trace'] -ne $false -or
        $receipt['native_platforms'].Count -ne 2 -or $receipt['native_platforms'][0] -cne 'x64' -or
        $receipt['native_platforms'][1] -cne 'Win32' -or $receipt['images'].Count -ne 5 -or
        $receipt['abi_probe_images'].Count -ne 2) { throw 'Staged Mo build contract mismatch.' }
    foreach ($platform in @('x64', 'Win32')) {
        if ($receipt['abi_probe_images'][$platform] -isnot [string] -or
            $receipt['abi_probe_images'][$platform] -cnotmatch '^[A-Fa-f0-9]{64}$') { throw 'Staged ABI probe receipt mismatch.' }
    }
    foreach ($name in @('bin/mo-broker.exe', 'bin/mo-settings.exe', 'bin/mo-tip-registrar.exe',
        'tip/x64/mo-tip.dll', 'tip/x86/mo-tip.dll')) {
        if ((Get-FileHash -LiteralPath (Join-Path $payload $name)).Hash -ine $receipt['images'][$name]) { throw 'Staged Mo image receipt mismatch.' }
    }
    return $metadata
}
