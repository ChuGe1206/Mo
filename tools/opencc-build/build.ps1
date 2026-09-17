[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$SourceArchivePath)
$ErrorActionPreference = 'Stop'
$expected = 'F8AAC3EDA054EDAF0313AA2103BAA965F63F80F4A496B35AC7B632DFD1A33953'
if ((Get-FileHash -LiteralPath $SourceArchivePath -Algorithm SHA256).Hash -ne $expected) {
    throw 'OpenCC 1.1.9 source archive hash mismatch.'
}
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$sourceRoot = Join-Path $repoRoot 'build/opencc-dict-tool/source'
if (-not (Test-Path -LiteralPath $sourceRoot)) {
    Expand-Archive -LiteralPath $SourceArchivePath -DestinationPath $sourceRoot
}
# Rebuilds are allowed only when EVERY extracted file still matches the pinned
# archive; never replace or silently compile a modified vendor tree.
$archive = [IO.Compression.ZipFile]::OpenRead((Resolve-Path -LiteralPath $SourceArchivePath).Path)
try {
    $fileCount = 0
    foreach ($entry in $archive.Entries) {
        if ($entry.FullName.EndsWith('/')) { continue }
        $fileCount++
        $entryPath = [IO.Path]::GetFullPath((Join-Path $sourceRoot $entry.FullName))
        if (-not $entryPath.StartsWith([IO.Path]::GetFullPath($sourceRoot).TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Archive path escapes the source root.'
        }
        $stream = $entry.Open()
        try { $archiveHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)) }
        finally { $stream.Dispose() }
        if ((Get-FileHash -LiteralPath $entryPath -Algorithm SHA256).Hash -ne $archiveHash) {
            throw 'Refusing to build a modified extracted OpenCC source tree.'
        }
    }
    if (@(Get-ChildItem -LiteralPath $sourceRoot -File -Recurse).Count -ne $fileCount) {
        throw 'Unexpected files in extracted OpenCC sources.'
    }
} finally { $archive.Dispose() }
$openccRoot = Join-Path $sourceRoot 'OpenCC-ver.1.1.9'
$vswhere = Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio/Installer/vswhere.exe'
$msbuild = & $vswhere -latest -products * -find 'MSBuild/**/Bin/MSBuild.exe' | Select-Object -First 1
if (-not $msbuild) { throw 'Visual Studio MSBuild is required.' }
$processPath = [Environment]::GetEnvironmentVariable('PATH', 'Process')
Remove-Item Env:Path -ErrorAction SilentlyContinue
$env:Path = $processPath
& $msbuild (Join-Path $PSScriptRoot 'MoOpenccDict.vcxproj') /m /nologo /p:Configuration=Release /p:Platform=x64 "/p:OpenccRoot=$openccRoot"
if ($LASTEXITCODE -ne 0) { throw "OpenCC dictionary tool build failed: $LASTEXITCODE" }
