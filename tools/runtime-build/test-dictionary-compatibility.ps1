[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$RuntimeBuildDirectory,
    [Parameter(Mandatory)][string]$OpenccDataDir
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $repoRoot 'tools/opencc-data.ps1')
$pack = Assert-MoCompiledOpenccData $OpenccDataDir
$runtime = (Resolve-Path -LiteralPath $RuntimeBuildDirectory).Path
$converter = (Resolve-Path -LiteralPath (Join-Path $runtime 'prefix/bin/opencc_dict.exe')).Path
$fixture = Join-Path $repoRoot ('build/mo-runtime-dict-墨-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
function Read-Entries([string]$Path) {
    $entries = [Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
    foreach ($line in [IO.File]::ReadAllLines($Path, [Text.UTF8Encoding]::new($false, $true))) {
        if ($line.Length -eq 0) { continue }
        $separator = $line.IndexOf("`t", [StringComparison]::Ordinal)
        if ($separator -le 0) { throw 'Invalid runtime dictionary record.' }
        $entries.Add($line.Substring(0, $separator), $line.Substring($separator + 1))
    }
    return ,$entries
}
try {
    foreach ($name in @('emoji', 'others')) {
        $sourceText = Join-Path $fixture "$name-source.txt"
        $binaryText = Join-Path $fixture "$name-binary.txt"
        # Canonicalize with the runtime's own OpenCC/Marisa, preserving values
        # within each key. Test-generated outputs only; never change the pack.
        & $converter -i (Join-Path $pack "source/$name.txt") -o $sourceText -f text -t text
        if ($LASTEXITCODE -ne 0) { throw 'Runtime dictionary source decoding failed.' }
        & $converter -i (Join-Path $pack "$name.ocd2") -o $binaryText -f ocd2 -t text
        if ($LASTEXITCODE -ne 0) { throw 'Runtime binary dictionary decoding failed.' }
        $sourceEntries = Read-Entries $sourceText
        $binaryEntries = Read-Entries $binaryText
        # TextDict and MarisaDict can enumerate keys in different orders. Only
        # key-set equality and the EXACT value sequence per key are semantic.
        if ($sourceEntries.Count -eq 0 -or $sourceEntries.Count -ne $binaryEntries.Count) {
            throw 'Runtime dictionary key count changed across Marisa versions.'
        }
        foreach ($key in $sourceEntries.Keys) {
            if (-not $binaryEntries.ContainsKey($key) -or -not [StringComparer]::Ordinal.Equals($sourceEntries[$key], $binaryEntries[$key])) {
                throw 'Runtime dictionary key or ordered values changed across Marisa versions.'
            }
        }
        Write-Host "Runtime $name dictionary: all $($sourceEntries.Count) keys and ordered values match source."
    }
} finally {
    $resolved = (Resolve-Path -LiteralPath $fixture).Path
    if (-not $resolved.StartsWith((Join-Path $repoRoot 'build').TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Refusing to remove a dictionary fixture outside repository build.'
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
