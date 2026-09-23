#Requires -Version 7.4
[CmdletBinding()]
param([string]$StageDirectory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'test-fixture.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $repo ('build/mo-stage-policy-墨-' + [Guid]::NewGuid().ToString('N'))
$null = Assert-MoNewBuildOutput $fixture $repo
New-Item -ItemType Directory -Path $fixture | Out-Null
$script:stageTests = 0
function Pass([string]$Label, [scriptblock]$Action) {
    & $Action
    $script:stageTests++
    Write-Host "PASS $Label"
}
function Reject([string]$Label, [scriptblock]$Action, [string]$ErrorPattern) {
    $rejected = $false
    try { & $Action | Out-Null } catch {
        if ($_.Exception.Message -notmatch $ErrorPattern) { throw "Unexpected failure for ${Label}: $($_.Exception.Message)" }
        $rejected = $true
    }
    if (-not $rejected) { throw "Expected rejection: $Label" }
    $script:stageTests++
    Write-Host "PASS $Label rejected"
}
function Write-FixtureJson([string]$Path, $Value) {
    $Value | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $Path -Encoding utf8NoBOM
}
$junction = Join-Path $fixture 'junction'
try {
    Pass 'safe relative path' { Assert-MoRelativeName 'data/rime-ice/build/rime_ice.table.bin' }
    foreach ($name in @('../escape', 'a/../escape', '/root', 'a\b', 'a:b', 'a//b', 'a.', 'CON', 'nul.txt', 'COM1.db', 'LPT9', 'a/file ', ('a' * 241))) {
        Reject "relative path $name" { Assert-MoRelativeName $name } 'relative path'
    }
    $pinnedArchive = Join-Path $fixture 'pinned-source.tar'
    [IO.File]::WriteAllBytes($pinnedArchive, [byte[]](1, 2, 3, 4))
    $pinnedHash = (Get-FileHash -LiteralPath $pinnedArchive -Algorithm SHA256).Hash
    Pass 'pinned source archive input' {
        $input = Resolve-MoPinnedSourceInput '' $pinnedArchive $pinnedHash
        if ($input.Kind -cne 'Archive' -or $input.Path -cne $pinnedArchive) { throw 'Wrong archive input result.' }
    }
    Pass 'pinned source checkout input' {
        $input = Resolve-MoPinnedSourceInput $fixture '' $pinnedHash
        if ($input.Kind -cne 'Checkout' -or $input.Path -cne $fixture) { throw 'Wrong checkout input result.' }
    }
    Reject 'missing pinned source input' { Resolve-MoPinnedSourceInput '' '' $pinnedHash } 'exactly one'
    Reject 'ambiguous pinned source input' { Resolve-MoPinnedSourceInput $fixture $pinnedArchive $pinnedHash } 'exactly one'
    Reject 'mismatched pinned source archive' { Resolve-MoPinnedSourceInput '' $pinnedArchive ('0' * 64) } 'archive mismatch'
    Reject 'invalid pinned source hash' { Resolve-MoPinnedSourceInput '' $pinnedArchive 'bad' } 'hash is invalid'
    Reject 'build root output' { Assert-MoNewBuildOutput (Join-Path $repo 'build') $repo } 'new child'
    Reject 'workspace output' { Assert-MoNewBuildOutput $repo $repo } 'new child'
    Reject 'prefix sibling output' { Assert-MoNewBuildOutput (Join-Path $repo 'build-escape/new') $repo } 'new child'
    Reject 'existing output' { Assert-MoNewBuildOutput $fixture $repo } 'overwrite'
    foreach ($leaf in @('native-%VARIABLE%', 'native-!VARIABLE!', 'native-"quote')) {
        Reject 'cmd expansion output path' {
            & (Join-Path $PSScriptRoot 'prepare-stage.ps1') -RuntimeBuildDirectory 'missing' -RimeIceSourceDir 'missing' -OutputDirectory (Join-Path $fixture $leaf)
        } 'shell expansion characters'
    }
    Reject 'relative input path' { Assert-MoPlainPath 'relative' } 'absolute'
    Reject 'UNC input' { Assert-MoPlainPath '\\server\share\x' } 'local DOS'
    Reject 'ADS input' { Assert-MoPlainPath ($fixture + ':stream') } 'local DOS'
    New-Item -ItemType Junction -Path $junction -Target $fixture | Out-Null
    Reject 'junction ancestor' { Assert-MoPlainPath (Join-Path $junction 'future') -MayNotExist } 'reparse'
    # Remove the junction itself, never recursively traverse it.
    Remove-Item -LiteralPath $junction -Force

    $files = Join-Path $fixture 'inventory'
    New-Item -ItemType Directory -Path (Join-Path $files 'nested') | Out-Null
    'base' | Set-Content -LiteralPath (Join-Path $files 'base.txt') -Encoding utf8NoBOM
    'child' | Set-Content -LiteralPath (Join-Path $files 'nested/child.txt') -Encoding utf8NoBOM
    $inventory = Get-MoStageInventory $files
    Pass 'exact inventory' { Assert-MoInventory $files $inventory }
    $serialized = $inventory | ConvertTo-Json -Depth 5
    $roundTrip = $serialized | ConvertFrom-Json -AsHashtable
    Pass 'JSON inventory round trip' { Assert-MoInventory $files $roundTrip }
    'extra' | Set-Content -LiteralPath (Join-Path $files 'extra.txt') -Encoding utf8NoBOM
    Reject 'unlisted file' { Assert-MoInventory $files $inventory } 'file count'
    Remove-Item -LiteralPath (Join-Path $files 'extra.txt')
    $fileBytes = [IO.File]::ReadAllBytes((Join-Path $files 'base.txt'))
    'tampered' | Set-Content -LiteralPath (Join-Path $files 'base.txt') -Encoding utf8NoBOM
    Reject 'tampered bytes' { Assert-MoInventory $files $inventory } 'hash/size'
    [IO.File]::WriteAllBytes((Join-Path $files 'base.txt'), $fileBytes)
    foreach ($badSize in @(-1, 268435457, '5', 5.5, $true)) {
        $bad = $serialized | ConvertFrom-Json -AsHashtable
        $bad['base.txt']['size'] = $badSize
        Reject 'invalid inventory size' { Assert-MoInventory $files $bad } 'inventory entry'
    }
    $bad = $serialized | ConvertFrom-Json -AsHashtable
    $bad['base.txt']['sha256'] = 'not-a-hash'
    Reject 'invalid inventory hash' { Assert-MoInventory $files $bad } 'inventory entry'
    $bad = [Collections.Specialized.OrderedDictionary]::new([StringComparer]::Ordinal)
    $bad.Add('base.txt', $inventory['base.txt']); $bad.Add('BASE.txt', $inventory['base.txt'])
    Reject 'case alias inventory' { Assert-MoInventory $files $bad } 'Duplicate'
    $bad = [ordered]@{ '../base.txt' = $inventory['base.txt'] }
    Reject 'manifest traversal path' { Assert-MoInventory $files $bad } 'relative path'
    $bad = [ordered]@{ 'BASE.txt' = $inventory['base.txt']; 'nested/child.txt' = $inventory['nested/child.txt'] }
    Reject 'inventory path case mismatch' { Assert-MoInventory $files $bad } 'hash/size'
    New-Item -ItemType Directory -Path (Join-Path $files 'empty') | Out-Null
    Reject 'unlisted empty directory' { Assert-MoInventory $files $inventory } 'Empty directory'
    Remove-Item -LiteralPath (Join-Path $files 'empty')
    New-Item -ItemType Junction -Path $junction -Target $files | Out-Null
    Reject 'inventory root junction' { Get-MoStageInventory $junction } 'reparse'
    Remove-Item -LiteralPath $junction -Force
    $junction = Join-Path $files 'linked'
    New-Item -ItemType Junction -Path $junction -Target $files | Out-Null
    Reject 'inventory leaf junction' { Get-MoStageInventory $files } 'Reparse'
    Remove-Item -LiteralPath $junction -Force
    $emptyFixture = Join-Path $fixture 'empty-cleanup'
    New-Item -ItemType Directory -Path (Join-Path $emptyFixture 'nested-empty') | Out-Null
    Pass 'incomplete fixture cleanup traversal' { Assert-MoOwnedFixtureTree $emptyFixture }
    $junction = Join-Path $emptyFixture 'linked'
    New-Item -ItemType Junction -Path $junction -Target $files | Out-Null
    Reject 'cleanup rejects unowned reparse traversal' { Assert-MoOwnedFixtureTree $emptyFixture } 'fixture reparse'
    Remove-Item -LiteralPath $junction -Force

    $jsonPath = Join-Path $fixture 'metadata.json'
    foreach ($case in @(
        @('{"x":1,"x":2}', 'Duplicate'), @('{"x":1,"X":2}', 'Duplicate'),
        @('{"x":"\u0000"}', 'NUL'), @('{"\u0000":1}', 'NUL'),
        @('[]', 'JSON object'), @('{"x":}', 'Exception calling'),
        @('{"x":1,}', 'Exception calling'), @(('{"x":' + ('[' * 33) + '0' + (']' * 33) + '}'), 'Exception calling')
    )) {
        $case[0] | Set-Content -LiteralPath $jsonPath -Encoding utf8NoBOM
        Reject 'invalid JSON metadata' { Read-MoStageJson $jsonPath } $case[1]
    }
    [IO.File]::WriteAllBytes($jsonPath, [byte[]]@(123, 34, 120, 34, 58, 34, 255, 34, 125))
    Reject 'invalid UTF-8 metadata' { Read-MoStageJson $jsonPath } 'Exception calling'
    [IO.File]::WriteAllBytes($jsonPath, [byte[]]::new(1048577))
    Reject 'oversized metadata' { Read-MoStageJson $jsonPath } '1 MiB'
    Write-FixtureJson $jsonPath ([ordered]@{ format = 1; development_only = $true; redistributable = $false })
    $valid = Read-MoStageJson $jsonPath
    Pass 'development metadata' { Assert-MoDevelopmentMetadata $valid 1 }
    foreach ($field in @('development_only', 'redistributable')) {
        $bad = $valid.Clone(); $bad[$field] = [string]$valid[$field]
        Reject 'string boolean metadata' { Assert-MoDevelopmentMetadata $bad 1 } 'development metadata'
    }
    $bad = $valid.Clone(); $bad['redistributable'] = $true
    Reject 'production authorization claim' { Assert-MoDevelopmentMetadata $bad 1 } 'development metadata'
    foreach ($format in @($true, '1', 1.0)) {
        $bad = $valid.Clone(); $bad['format'] = $format
        Reject 'non-integer format' { Assert-MoDevelopmentMetadata $bad 1 } 'development metadata'
    }
    Reject 'missing prebuilt data' { Assert-MoPrebuiltData $files } 'inventory count'
    Reject 'missing payload image' { Assert-MoStagePayloadNames @('data/rime-ice/default.yaml') } 'Required'
    Reject 'non-PE bytes' { Assert-MoPeArchitecture (Join-Path $files 'base.txt') x64 $false } 'Not a PE'
    Reject 'development installer authorization gate' {
        & (Join-Path $PSScriptRoot 'build.ps1') `
            -StageDirectory (Join-Path $fixture 'does-not-exist') `
            -WixToolchainDirectory (Join-Path $fixture 'missing-toolchain') `
            -OutputDirectory (Join-Path $fixture 'installer-output')
    } 'Refusing to build a non-deployable installer'

    if ($StageDirectory) {
        $source = Assert-MoPlainPath $StageDirectory
        Pass 'real prepared stage' { $null = Assert-MoPreparedStage $source }
        $copy = Join-Path $fixture 'stage-copy'
        Copy-Item -LiteralPath $source -Destination $copy -Recurse
        Pass 'relocated prepared stage' { $null = Assert-MoPreparedStage $copy }
        $manifest = Join-Path $copy 'mo-stage.json'
        $originalManifest = [IO.File]::ReadAllBytes($manifest)
        'extra' | Set-Content -LiteralPath (Join-Path $copy 'unlisted.txt') -Encoding utf8NoBOM
        Reject 'real stage unlisted file' { Assert-MoPreparedStage $copy } 'file count'
        Remove-Item -LiteralPath (Join-Path $copy 'unlisted.txt')
        $dll = Join-Path $copy 'payload/Mo/tip/x64/mo-tip.dll'
        $originalDll = [IO.File]::ReadAllBytes($dll)
        $changed = [byte[]]$originalDll.Clone(); $changed[100] = $changed[100] -bxor 1
        [IO.File]::WriteAllBytes($dll, $changed)
        Reject 'real stage image mutation' { Assert-MoPreparedStage $copy } 'hash/size'
        [IO.File]::WriteAllBytes($dll, $originalDll)
        foreach ($kind in @('installable', 'redistributable')) {
            $bad = Read-MoStageJson $manifest; $bad[$kind] = $true
            Write-FixtureJson $manifest $bad
            Reject 'real stage release claim' { Assert-MoPreparedStage $copy } 'development metadata|Unsupported staging'
            [IO.File]::WriteAllBytes($manifest, $originalManifest)
        }
        $broker = Join-Path $copy 'payload/Mo/bin/mo-broker.exe'
        Pass 'real x64 EXE architecture' { Assert-MoPeArchitecture $broker x64 $false }
        Reject 'x64 as x86 image' { Assert-MoPeArchitecture $broker x86 $false } 'architecture/kind'
        Reject 'EXE as DLL' { Assert-MoPeArchitecture $broker x64 $true } 'architecture/kind'
        Reject 'x86 as x64 image' { Assert-MoPeArchitecture (Join-Path $copy 'payload/Mo/tip/x86/mo-tip.dll') x64 $true } 'architecture/kind'
        # Recompute ONLY the outer inventory to demonstrate intrinsic contract
        # checks are independent of that inventory. This is not authentication.
        $brokerBytes = [IO.File]::ReadAllBytes($broker)
        [IO.File]::WriteAllBytes($broker, [IO.File]::ReadAllBytes((Join-Path $copy 'payload/Mo/tip/x86/mo-tip.dll')))
        $changedMeta = Read-MoStageJson $manifest
        $changedMeta['files'] = Get-MoStageInventory $copy @('mo-stage.json')
        Write-FixtureJson $manifest $changedMeta
        Reject 'resealed wrong-architecture image' { Assert-MoPreparedStage $copy } 'architecture/kind'
        [IO.File]::WriteAllBytes($broker, $brokerBytes)
        [IO.File]::WriteAllBytes($manifest, $originalManifest)
        $unexpected = Join-Path $copy 'payload/Mo/bin/debug-probe.exe'
        [IO.File]::WriteAllBytes($unexpected, $brokerBytes)
        $changedMeta = Read-MoStageJson $manifest
        $changedMeta['files'] = Get-MoStageInventory $copy @('mo-stage.json')
        Write-FixtureJson $manifest $changedMeta
        Reject 'resealed diagnostic payload' { Assert-MoPreparedStage $copy } 'Unexpected staging payload'
        Remove-Item -LiteralPath $unexpected
        [IO.File]::WriteAllBytes($manifest, $originalManifest)
        $receiptPath = Join-Path $copy 'evidence/build-receipt.json'
        $originalReceipt = [IO.File]::ReadAllBytes($receiptPath)
        $badReceipt = Read-MoStageJson $receiptPath; $badReceipt['latency_trace'] = $true
        Write-FixtureJson $receiptPath $badReceipt
        $changedMeta = Read-MoStageJson $manifest
        $changedMeta['files'] = Get-MoStageInventory $copy @('mo-stage.json')
        Write-FixtureJson $manifest $changedMeta
        Reject 'resealed diagnostic build receipt' { Assert-MoPreparedStage $copy } 'Mo build contract'
        [IO.File]::WriteAllBytes($receiptPath, $originalReceipt)
        [IO.File]::WriteAllBytes($manifest, $originalManifest)
        $runtimePath = Join-Path $copy 'evidence/runtime-provenance.json'
        $originalRuntime = [IO.File]::ReadAllBytes($runtimePath)
        foreach ($mutation in @('policy', 'patch-binding', 'own-source')) {
            $badRuntime = Read-MoStageJson $runtimePath
            switch ($mutation) {
                'policy' { $badRuntime['lua_data_policy'] = 'user-first' }
                'patch-binding' { $badRuntime['lua_data_policy_patch_sha256'] = '0' * 64 }
                'own-source' { [void]$badRuntime['mo_inputs'].Remove('native/librime/preparation/lua-machine-data-only.patch') }
            }
            Write-FixtureJson $runtimePath $badRuntime
            $changedMeta = Read-MoStageJson $manifest
            $changedMeta['files'] = Get-MoStageInventory $copy @('mo-stage.json')
            Write-FixtureJson $manifest $changedMeta
            Reject "resealed runtime Lua $mutation" { Assert-MoPreparedStage $copy } 'runtime.*(contract|own-source|patch)'
            [IO.File]::WriteAllBytes($runtimePath, $originalRuntime)
            [IO.File]::WriteAllBytes($manifest, $originalManifest)
        }
        Move-Item -LiteralPath $manifest -Destination (Join-Path $fixture 'completion-marker.json')
        Reject 'missing completion marker' { Assert-MoPreparedStage $copy } 'does not exist'
        Move-Item -LiteralPath (Join-Path $fixture 'completion-marker.json') -Destination $manifest
        Pass 'restored actual stage' { $null = Assert-MoPreparedStage $copy }
    }
    Write-Host "Staging policy tests passed: $script:stageTests. No registration, installation, signing or network access."
} finally {
    if (Test-Path -LiteralPath $junction) {
        $link = Get-Item -LiteralPath $junction -Force
        if (-not ($link.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Fixture junction changed type.' }
        Remove-Item -LiteralPath $junction -Force
    }
    $resolved = Assert-MoPlainPath $fixture
    if (-not $resolved.StartsWith((Join-Path $repo 'build') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe fixture cleanup target.' }
    # Traversal check before recursive cleanup; all remaining entries are owned fixtures.
    Assert-MoOwnedFixtureTree $resolved
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
