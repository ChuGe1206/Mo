[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$LibrimeDistDir,
    [Parameter(Mandatory)][string]$SharedDataDir,
    [Parameter(Mandatory)][string]$UserDataDir,
    [ValidatePattern('^[A-Za-z0-9._-]+$')][string]$RustToolchain = 'stable'
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$dist = (Resolve-Path -LiteralPath $LibrimeDistDir).Path
$shared = (Resolve-Path -LiteralPath $SharedDataDir).Path
$user = (Resolve-Path -LiteralPath $UserDataDir).Path
$dllSource = Join-Path $dist 'lib/rime.dll'
$resources = Join-Path $dist 'lib/opencc'
$provenance = Get-Content -LiteralPath (Join-Path $dist 'mo-build-provenance.json') -Raw | ConvertFrom-Json -AsHashtable
if ($provenance.format -ne 2 -or $provenance.preparation_abi -ne 3 -or
    $provenance.userdb_policy -cne 'strict-open-no-auto-recovery-v1' -or
    $provenance.userdb_policy_patch_sha256 -ine $provenance.mo_inputs['native/librime/preparation/userdb-preserve.patch'] -or
    -not $provenance.development_only -or $provenance.redistributable -or
    (Get-FileHash -LiteralPath $dllSource).Hash -ne $provenance.dll_sha256) { throw 'Invalid v3 development provenance.' }
$files = @(Get-ChildItem -LiteralPath $resources -File)
if ($files.Count -ne $provenance.resources.Count) { throw 'Runtime resource inventory differs from provenance.' }
foreach ($file in $files) {
    if ((Get-FileHash -LiteralPath $file.FullName).Hash -ne $provenance.resources.($file.Name)) { throw 'Runtime resource hash differs from provenance.' }
}
Push-Location $repoRoot
try {
    & cargo "+$RustToolchain" build --quiet -p mo-rime --example preparation_probe
    if ($LASTEXITCODE -ne 0) { throw 'Preparation probe build failed.' }
} finally { Pop-Location }
$probe = Join-Path $repoRoot 'target/debug/examples/preparation_probe.exe'
$cases = @('success', 'missing-config', 'missing-standard', 'traversal', 'absolute', 'slash', 'backslash',
    'ads', 'escaped-nul', 'raw-nul', 'invalid-utf8', 'text', 'empty-chain', 'primitive-chain',
    'empty-group', 'large-group', 'large-chain', 'nested-group', 'duplicate-property', 'invalid-json',
    'corrupt-dictionary', 'large-config', 'root-junction', 'leaf-junction', 'hardlink',
    'unsupported-filter', 'duplicate-owner')
foreach ($case in $cases) {
    $fixture = Join-Path $repoRoot ('build/mo-relocation-搬迁-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $fixture | Out-Null
    $links = @()
    try {
        $testUser = Join-Path $fixture 'user-墨'
        $runtime = Join-Path $fixture 'runtime-墨'
        $cwd = Join-Path $fixture 'cwd-trap'
        New-Item -ItemType Directory -Path $testUser, $runtime, $cwd | Out-Null
        Copy-Item -LiteralPath (Join-Path $user 'build') -Destination (Join-Path $testUser 'build') -Recurse
        New-Item -ItemType File -Path (Join-Path $testUser 'mo-preparation-fixture') | Out-Null
        Copy-Item -LiteralPath $dllSource -Destination (Join-Path $runtime 'rime.dll')
        Copy-Item -LiteralPath $resources -Destination (Join-Path $runtime 'opencc') -Recurse
        # Valid traps ensure a missing installed resource cannot fall back.
        Copy-Item -LiteralPath $resources -Destination (Join-Path $testUser 'opencc') -Recurse
        foreach ($file in $files) { Copy-Item -LiteralPath $file.FullName -Destination $cwd }
        $root = Join-Path $runtime 'opencc'
        $json = Join-Path $root 'emoji.json'
        $config = Get-Content -LiteralPath $json -Raw | ConvertFrom-Json
        $outcome = 'failure'
        switch ($case) {
            'success' {
                $outcome = 'success'
                '{invalid trap' | Set-Content -LiteralPath (Join-Path $testUser 'opencc/emoji.json') -Encoding utf8NoBOM
                '{invalid trap' | Set-Content -LiteralPath (Join-Path $cwd 'emoji.json') -Encoding utf8NoBOM
            }
            'missing-config' { Remove-Item -LiteralPath $json }
            'missing-standard' { Remove-Item -LiteralPath (Join-Path $root 'STCharacters.ocd2') }
            'traversal' { $config.segmentation.dict.file = '../opencc/emoji.ocd2' }
            'absolute' { $config.segmentation.dict.file = Join-Path $resources 'emoji.ocd2' }
            'slash' { $config.segmentation.dict.file = 'sub/emoji.ocd2' }
            'backslash' { $config.segmentation.dict.file = 'sub\emoji.ocd2' }
            'ads' { $config.segmentation.dict.file = 'emoji.ocd2:stream' }
            'escaped-nul' { $config.segmentation.dict.file = "emoji.ocd2`0ignored" }
            'text' { $config.segmentation.dict.type = 'text' }
            'empty-chain' { $config.conversion_chain = @() }
            'primitive-chain' { $config.conversion_chain = @(1) }
            'empty-group' { $config.conversion_chain[0].dict.dicts = @() }
            'large-group' { $config.conversion_chain[0].dict.dicts = @(1..65 | ForEach-Object { @{ type = 'ocd2'; file = 'emoji.ocd2' } }) }
            'large-chain' { $config.conversion_chain = @(1..17 | ForEach-Object { @{ dict = @{ type = 'ocd2'; file = 'emoji.ocd2' } } }) }
            'nested-group' {
                $nested = @{ type = 'ocd2'; file = 'emoji.ocd2' }
                foreach ($index in 1..17) { $nested = @{ type = 'group'; dicts = @($nested) } }
                $config.conversion_chain[0].dict = $nested
            }
            'root-junction' {
                $moveSource = (Resolve-Path -LiteralPath $root).Path
                $moveTarget = [IO.Path]::GetFullPath((Join-Path $runtime 'original-opencc'))
                foreach ($path in @($moveSource, $moveTarget)) {
                    if (-not $path.StartsWith($fixture.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Refusing directory move outside its new relocation fixture.' }
                }
                Move-Item -LiteralPath $moveSource -Destination $moveTarget
                New-Item -ItemType Junction -Path $root -Target $moveTarget | Out-Null
                $links += $root
            }
            'leaf-junction' {
                Remove-Item -LiteralPath $json
                New-Item -ItemType Junction -Path $json -Target $cwd | Out-Null
                $links += $json
            }
            'hardlink' {
                Move-Item -LiteralPath (Join-Path $root 'emoji.ocd2') -Destination (Join-Path $root 'emoji-copy.ocd2')
                New-Item -ItemType HardLink -Path (Join-Path $root 'emoji.ocd2') -Target (Join-Path $root 'emoji-copy.ocd2') | Out-Null
            }
            'unsupported-filter' {
                $schema = Join-Path $testUser 'build/rime_ice.schema.yaml'
                (Get-Content -LiteralPath $schema -Raw).Replace('opencc_config: emoji.json', 'opencc_config: ../emoji.json') | Set-Content -LiteralPath $schema -Encoding utf8NoBOM
            }
            'duplicate-owner' {
                $schema = Join-Path $testUser 'build/rime_ice.schema.yaml'
                (Get-Content -LiteralPath $schema -Raw).Replace('opencc_config: s2t.json', 'opencc_config: emoji.json') | Set-Content -LiteralPath $schema -Encoding utf8NoBOM
            }
        }
        if ($case -in @('traversal', 'absolute', 'slash', 'backslash', 'ads', 'escaped-nul', 'text',
            'empty-chain', 'primitive-chain', 'empty-group', 'large-group', 'large-chain', 'nested-group')) {
            $config | ConvertTo-Json -Depth 64 | Set-Content -LiteralPath $json -Encoding utf8NoBOM
        }
        switch ($case) {
            'duplicate-property' { (Get-Content -LiteralPath $json -Raw).Replace('"emoji.ocd2"', '"emoji.ocd2", "file": "emoji.ocd2"') | Set-Content -LiteralPath $json -Encoding utf8NoBOM }
            'invalid-json' { '{invalid' | Set-Content -LiteralPath $json -Encoding utf8NoBOM }
            'raw-nul' { [IO.File]::WriteAllBytes($json, [Text.Encoding]::UTF8.GetBytes((Get-Content -LiteralPath $json -Raw) + "`0ignored")) }
            'invalid-utf8' {
                # Preserve otherwise valid JSON; invalidate only a string value.
                $content = Get-Content -LiteralPath $json -Raw
                $content = $content.Insert($content.IndexOf('{') + 1, '"mo": "x",')
                $bytes = [Text.Encoding]::UTF8.GetBytes($content)
                $bytes[[Array]::IndexOf($bytes, [byte][char]'x')] = 0xFF
                [IO.File]::WriteAllBytes($json, $bytes)
            }
            'large-config' { [IO.File]::WriteAllBytes($json, [Text.Encoding]::UTF8.GetBytes(' ' * 65537)) }
            'corrupt-dictionary' { [IO.File]::WriteAllBytes((Join-Path $root 'emoji.ocd2'), [byte[]]@(1, 2, 3, 4, 5)) }
        }
        Push-Location $cwd
        try {
            & $probe (Join-Path $runtime 'rime.dll') $shared $testUser $outcome
            if ($LASTEXITCODE -ne 0) { throw "Native relocation case failed: $case" }
        } finally { Pop-Location }
        Write-Host "Mo relocation boundary passed: $case"
    } finally {
        $resolved = (Resolve-Path -LiteralPath $fixture).Path
        if (-not $resolved.StartsWith((Join-Path $repoRoot 'build').TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Refusing relocation cleanup outside repository build.' }
        foreach ($link in $links) {
            if (-not ((Get-Item -LiteralPath $link -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Relocation fixture link changed type.' }
            Remove-Item -LiteralPath $link -Force
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
Write-Host "$($cases.Count) native relocation/preparation cases passed; original DLL, resources and Windows input state unchanged."
