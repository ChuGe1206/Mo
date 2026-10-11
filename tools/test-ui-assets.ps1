#Requires -Version 7.4
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$root = Join-Path $repo 'assets/ui'
$sizes = @(20,25,30,40,50,60,80)
foreach ($name in @('previous','next','expand','collapse','close','smile','shield','add')) {
    [xml]$svg = Get-Content -Raw -LiteralPath (Join-Path $root "$name.svg")
    if ($svg.DocumentElement.LocalName -cne 'svg') { throw "Not SVG: $name" }
    $bytes = [IO.File]::ReadAllBytes((Join-Path $root "$name.mask"))
    if ($bytes[0] -ne $sizes.Count) { throw "Mask DPI count mismatch: $name" }
    $offset = 1
    foreach ($size in $sizes) {
        if ($offset + 6 -gt $bytes.Length -or [BitConverter]::ToUInt16($bytes,$offset) -ne $size -or
            [BitConverter]::ToUInt32($bytes,$offset+2) -ne $size*$size) { throw "Mask size mismatch: $name" }
        $offset += 6
        if ($offset + $size*$size -gt $bytes.Length) { throw "Truncated mask: $name" }
        $alpha = $bytes[$offset..($offset+$size*$size-1)]
        if (-not ($alpha | Where-Object { $_ -gt 0 })) { throw "Blank mask: $name" }
        $offset += $size*$size
    }
    if ($offset -ne $bytes.Length) { throw "Trailing mask data: $name" }
}
$rust = [IO.File]::ReadAllText((Join-Path $repo 'crates/mo-domain/src/symbols.rs'))
$native = [IO.File]::ReadAllText((Join-Path $repo 'native/windows-tip/include/mo_symbols.h'))
$entries = [regex]::Matches($rust, 'text: "([^"]+)",\s*label: "([^"]+)",\s*category: SymbolCategory::(\w+)')
$nativeEntries = [regex]::Matches($native, '\{L"([^"]+)", L"([^"]+)", ([123])\}')
if ($entries.Count -ne 28 -or $nativeEntries.Count -ne 28) { throw 'Catalog v1 size mismatch' }
$categories = @{ Smile = '1'; Gesture = '2'; Kaomoji = '3' }
for ($i=0; $i -lt 28; $i++) {
    if ($entries[$i].Groups[1].Value -cne $nativeEntries[$i].Groups[1].Value -or
        $entries[$i].Groups[2].Value -cne $nativeEntries[$i].Groups[2].Value -or
        $categories[$entries[$i].Groups[3].Value] -cne $nativeEntries[$i].Groups[3].Value) {
        throw "Catalog wire ordinal mismatch: $i"
    }
}
. (Join-Path $repo 'installer/windows/staging-policy.ps1')
. (Join-Path $repo 'installer/windows/release-materials.ps1')
. (Join-Path $repo 'installer/windows/release-compliance.ps1')
$policy = Read-MoReleaseMaterialsPolicy (Join-Path $repo 'installer/windows/release-materials-policy.json')
if ($policy['documents'].Count -ne 17) { throw 'License/source material count mismatch' }
foreach ($id in @('figma-sds-mit','figma-sds-smile')) {
    $matches = @($policy['documents'] | Where-Object { $_['id'] -ceq $id })
    if ($matches.Count -ne 1) { throw "Missing UI material: $id" }
    $entry = $matches[0]
    $source = Join-Path $repo $entry['origin'].Substring(5)
    if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -cne $entry['sha256']) { throw "UI material hash mismatch: $id" }
}
$compliance = Read-MoReleaseCompliancePolicy (Join-Path $repo 'installer/windows/release-compliance-policy.json')
$nativePackage = @($compliance['packages'] | Where-Object { $_['id'] -ceq 'mo-native' })[0]
if ('figma-sds' -cnotin $nativePackage['depends_on']) { throw 'Embedded icon missing from SPDX dependencies' }
$sds = @($compliance['packages'] | Where-Object { $_['id'] -ceq 'figma-sds' })[0]
$notices = (Get-MoThirdPartyNotices @{ packages = @{ 'figma-sds' = $sds } }) -join [Environment]::NewLine
$license = [IO.File]::ReadAllText((Join-Path $repo 'third_party/figma-sds-MIT.txt')).Trim()
if (-not $notices.Contains($license)) { throw 'Full MIT notice missing from distribution notices' }
Write-Host 'UI asset masks, catalog ordinals, license pins and distribution notice tests passed.'
exit 0
