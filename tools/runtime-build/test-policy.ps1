[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SourceDir,
    [Parameter(Mandatory)][string]$CmakeArchivePath,
    [Parameter(Mandatory)][string]$BoostArchivePath,
    [Parameter(Mandatory)][string]$LuaArchivePath,
    [Parameter(Mandatory)][string]$PythonPath,
    [Parameter(Mandatory)][string]$OpenccDataDir
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $PSScriptRoot 'source-policy.ps1')
$fixture = Join-Path $repoRoot ('build/mo-runtime-policy-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$arguments = @{
    SourceDir = $SourceDir; CmakeArchivePath = $CmakeArchivePath
    BoostArchivePath = $BoostArchivePath; LuaArchivePath = $LuaArchivePath
    PythonPath = $PythonPath; OutputDirectory = (Join-Path $fixture 'new')
    OpenccDataDir = $OpenccDataDir
}
function Reject([hashtable]$Changes, [string]$Expected) {
    $call = $arguments.Clone()
    foreach ($key in $Changes.Keys) { $call[$key] = $Changes[$key] }
    $message = $null
    try { & (Join-Path $PSScriptRoot 'build.ps1') @call }
    catch { $message = $_.Exception.Message }
    if ($null -eq $message -or $message -notlike "*$Expected*") {
        throw "Expected builder rejection '$Expected', got '$message'."
    }
    if (Test-Path -LiteralPath $arguments.OutputDirectory) {
        throw 'Rejected builder created its output directory.'
    }
}
try {
    $sentinel = Join-Path $fixture 'sentinel'
    New-Item -ItemType File -Path $sentinel | Out-Null
    Reject @{ OutputDirectory = $repoRoot } 'new child'
    Reject @{ OutputDirectory = $fixture } 'refusing to overwrite'
    if (-not (Test-Path -LiteralPath $sentinel -PathType Leaf)) { throw 'Existing output changed.' }
    $badArchive = Join-Path $fixture 'bad.zip'
    New-Item -ItemType File -Path $badArchive | Out-Null
    Reject @{ CmakeArchivePath = $badArchive } 'archive hash mismatch'
    Reject @{ SourceDir = $repoRoot } 'source commit mismatch'
    $badPack = Join-Path $fixture 'bad-pack'
    New-Item -ItemType Directory -Path $badPack | Out-Null
    '{"format":0}' | Set-Content -LiteralPath (Join-Path $badPack 'manifest.json') -Encoding utf8NoBOM
    Reject @{ OpenccDataDir = $badPack } 'Unsupported compiled OpenCC manifest'
    Write-Host 'Five runtime builder fail-closed cases passed.'
    $cache = Join-Path $fixture 'CMakeCache.txt'
    $expected = @{ MoLibrary = (Join-Path $fixture 'pinned.lib') }
    "MoLibrary:FILEPATH=$($expected.MoLibrary)" | Set-Content -LiteralPath $cache
    Assert-MoRuntimeSourcePaths $cache $expected
    foreach ($record in @('MoLibrary:FILEPATH=C:/foreign/other.lib', 'OtherLibrary:FILEPATH=C:/foreign/other.lib', "MoLibrary:FILEPATH=$($expected.MoLibrary)`nMoLibrary:FILEPATH=$($expected.MoLibrary)")) {
        $record | Set-Content -LiteralPath $cache
        $rejected = $false
        try { Assert-MoRuntimeSourcePaths $cache $expected } catch { $rejected = $true }
        if (-not $rejected) { throw 'Invalid source cache accepted.' }
    }
    Write-Host 'Four CMake source-path policy cases passed.'
    $project = Join-Path $fixture 'native.vcxproj'
    $source = Join-Path $fixture 'wrapper.cpp'
    $entry = '<ClCompile Include="' + $source + '"><WarningLevel>Level4</WarningLevel><TreatWarningAsError>true</TreatWarningAsError></ClCompile>'
    ('<Project>' + $entry + '</Project>') | Set-Content -LiteralPath $project
    Assert-MoRuntimeStrictSources $project @($source)
    foreach ($invalid in @('<Project />', ('<Project>' + $entry.Replace('Level4', 'Level3') + '</Project>'),
        ('<Project>' + $entry.Replace('>true<', '>false<') + '</Project>'),
        ('<Project>' + $entry + $entry + '</Project>'),
        ('<Project>' + $entry.Replace('<WarningLevel>', '<WarningLevel Condition="wrong">') + '</Project>'),
        ('<Project>' + $entry.Replace('<TreatWarningAsError>true</TreatWarningAsError>', '') + '</Project>'))) {
        $invalid | Set-Content -LiteralPath $project
        $rejected = $false
        try { Assert-MoRuntimeStrictSources $project @($source) } catch { $rejected = $true }
        if (-not $rejected) { throw 'Invalid generated strict source properties accepted.' }
    }
    Write-Host 'Seven generated native strict-source property cases passed.'
} finally {
    $resolved = (Resolve-Path -LiteralPath $fixture).Path
    if (-not $resolved.StartsWith((Join-Path $repoRoot 'build').TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Refusing to remove a policy fixture outside repository build.'
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
