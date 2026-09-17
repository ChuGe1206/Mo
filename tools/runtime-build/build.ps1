[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SourceDir,
    [Parameter(Mandatory)][string]$CmakeArchivePath,
    [Parameter(Mandatory)][string]$BoostArchivePath,
    [Parameter(Mandatory)][string]$LuaArchivePath,
    [Parameter(Mandatory)][string]$PythonPath,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
# Codex's Windows host may supply both PATH and Path. MSBuild's .NET Framework
# child-environment dictionary rejects that duplication; normalize process only.
$processPath = [Environment]::GetEnvironmentVariable('PATH', 'Process')
Remove-Item Env:Path -ErrorAction SilentlyContinue
$env:Path = $processPath
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $PSScriptRoot 'source-policy.ps1')
$source = (Resolve-Path -LiteralPath $SourceDir).Path
$python = (Resolve-Path -LiteralPath $PythonPath).Path
$output = [IO.Path]::GetFullPath($OutputDirectory)
if (-not $output.StartsWith((Join-Path $repoRoot 'build').TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Runtime output must be a new child of the repository build directory.'
}
if (Test-Path -LiteralPath $output) { throw 'Runtime output must be new; refusing to overwrite.' }

function Checked([string]$Program, [string[]]$Arguments) {
    $script:runtimeLogCount++
    $log = Join-Path $script:runtimeLogRoot ('{0:D2}.log' -f $script:runtimeLogCount)
    & $Program @Arguments 2>&1 | Tee-Object -FilePath $log | ForEach-Object {
        if ($_ -match 'error|fatal|^-- |mo_preparation|rime\.dll|MSBuild') { Write-Host $_ }
    }
    if ($LASTEXITCODE -ne 0) { throw "Runtime build command failed: $Program ($LASTEXITCODE); full log: $log" }
}
function Pinned-Archive([string]$ArchivePath, [string]$Expected) {
    $resolved = (Resolve-Path -LiteralPath $ArchivePath).Path
    if ((Get-FileHash -LiteralPath $resolved -Algorithm SHA256).Hash -ne $Expected) {
        throw 'Runtime input archive hash mismatch.'
    }
    return $resolved
}
$cmakeArchive = Pinned-Archive $CmakeArchivePath '13D1A463D7130DF5339BAEDD63D8AE990AAF385062B2F42F372796143AE94086'
$boostArchive = Pinned-Archive $BoostArchivePath 'CC77EB8ED25DA4D596B25E77E4DBB6C5AFAAC9CDDD00DC9CA947B6B268CC76A4'
$luaArchive = Pinned-Archive $LuaArchivePath '2335B6C582A52654F94612BF10D2F4672805D05329AA6568B1D8CD9E5C6FB8E6'

# Consume ONLY archived Git objects at exact commits, never mutable checkouts,
# ignored headers/libs, source-tree CMake caches or auto-discovered plugins.
$pins = @(
    @('', '33e78140250125871856cdc5b42ddc6a5fcd3cd4', 'B22594E1FCF55DF5BBF60E76DC49200671410E98540BE265F68465D03678C722'),
    @('deps/leveldb', '99b3c03b3284f5886f9ef9a4ef703d57373e61be', 'FE47E88AE4D2B1162209BF65C74D203753D2D5F105262192B9DAC41841E6C0B8'),
    @('deps/marisa-trie', '3e87d53b78e15f2f43783d5e376561a8c9722051', '8F557F1171ABF1D205B81F9C40B3E09F90BBE2E664CBA3172ECF9D603E406CD8'),
    @('deps/opencc', '556ed22496d650bd0b13b6c163be9814637970ae', 'C46A6130BC85F09B64A144E6BB316374E1B919D07055B7C5FAD870983495A60E'),
    @('deps/yaml-cpp', '2f86d13775d119edbb69af52e5f566fd65c6953b', '460796C79164719FFDBE7E57D0AB8E0C0A68990F8EB2814A56734C8415F8C59C'),
    @('plugins/lua', 'ec52e48ea18f11af37717a01c337f853215cf70b', 'F13438CFA7AE8E64D722D89F9A76A05206CB18AEC5B34DB6E6A0546BBEE1A640')
)
foreach ($pin in $pins) {
    $head = & git -C (Join-Path $source $pin[0]) rev-parse HEAD
    if ($LASTEXITCODE -ne 0 -or $head -ne $pin[1]) { throw 'Runtime source commit mismatch.' }
}
New-Item -ItemType Directory -Path $output | Out-Null
$script:runtimeLogCount = 0
$script:runtimeLogRoot = Join-Path $output 'commands'
New-Item -ItemType Directory -Path $script:runtimeLogRoot | Out-Null
$inputs = Join-Path $output 'inputs'
$prepared = Join-Path $inputs 'librime'
New-Item -ItemType Directory -Path $prepared | Out-Null
$inputHashes = [ordered]@{}
foreach ($pin in $pins) {
    $archivePath = Join-Path $inputs (($pin[0] -replace '/', '-') + 'source.tar')
    Checked 'git' @('-C', (Join-Path $source $pin[0]), 'archive', '--format=tar', 'HEAD', '-o', $archivePath)
    $hash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash
    if ($hash -ne $pin[2]) { throw 'Runtime source archive mismatch.' }
    $inputHashes[$pin[0]] = @{ commit = $pin[1]; archive_sha256 = $hash }
    $destination = Join-Path $prepared $pin[0]
    if (-not (Test-Path -LiteralPath $destination)) { New-Item -ItemType Directory -Path $destination | Out-Null }
    Checked 'tar' @('-xf', $archivePath, '-C', $destination)
}

# Fresh extraction is confined to this new build output; no vendor tree edits.
[IO.Compression.ZipFile]::ExtractToDirectory($cmakeArchive, (Join-Path $inputs 'cmake'))
[IO.Compression.ZipFile]::ExtractToDirectory($boostArchive, (Join-Path $inputs 'boost'))
Checked 'tar' @('-xf', $luaArchive, '-C', $inputs)
$luaSource = Join-Path $prepared 'plugins/lua/thirdparty/lua5.4'
New-Item -ItemType Directory -Path (Split-Path -Parent $luaSource) | Out-Null
Copy-Item -LiteralPath (Join-Path $inputs 'lua-5.4.9/src') -Destination $luaSource -Recurse
$patch = Join-Path $repoRoot 'native/librime/preparation/resources-v1.patch'
Checked 'git' @('-C', $prepared, "--git-dir=$source/.git", "--work-tree=$prepared", 'apply', '--check', $patch)
Checked 'git' @('-C', $prepared, "--git-dir=$source/.git", "--work-tree=$prepared", 'apply', $patch)
$luaPatch = Join-Path $repoRoot 'native/librime/preparation/lua-signed-stack.patch'
Checked 'git' @('-C', (Join-Path $prepared 'plugins/lua'), "--git-dir=$source/plugins/lua/.git",
    "--work-tree=$prepared/plugins/lua", 'apply', '--check', $luaPatch)
Checked 'git' @('-C', (Join-Path $prepared 'plugins/lua'), "--git-dir=$source/plugins/lua/.git",
    "--work-tree=$prepared/plugins/lua", 'apply', $luaPatch)
$cmake = Join-Path $inputs 'cmake/cmake-3.31.10-windows-x86_64/bin/cmake.exe'
$boost = Join-Path $inputs 'boost/boost_1_84_0'
$prefix = Join-Path $output 'prefix'
$dist = Join-Path $output 'dist'
$common = @('-G', 'Visual Studio 17 2022', '-A', 'x64', '-DCMAKE_CONFIGURATION_TYPES=Release',
    '-DCMAKE_POLICY_DEFAULT_CMP0091=NEW',
    '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded', '-DBUILD_SHARED_LIBS=OFF',
    "-DCMAKE_INSTALL_PREFIX=$prefix", '-DBUILD_TESTING=OFF', '-DCMAKE_CXX_FLAGS=/DWIN32 /D_WINDOWS /W3 /GR /EHsc /MP4')

foreach ($dep in @('leveldb', 'yaml-cpp', 'marisa-trie', 'opencc')) {
    $options = switch ($dep) {
        'leveldb' { @('-DLEVELDB_BUILD_BENCHMARKS=OFF', '-DLEVELDB_BUILD_TESTS=OFF') }
        'yaml-cpp' { @('-DYAML_CPP_BUILD_CONTRIB=OFF', '-DYAML_CPP_BUILD_TESTS=OFF', '-DYAML_CPP_BUILD_TOOLS=OFF', '-DYAML_MSVC_SHARED_RT=OFF') }
        'marisa-trie' { @('-DENABLE_TOOLS=OFF', '-DENABLE_NATIVE_CODE=OFF') }
        'opencc' { @('-DENABLE_GTEST=OFF', '-DENABLE_BENCHMARK=OFF', "-DPYTHON_EXECUTABLE=$python",
            '-DUSE_SYSTEM_MARISA=ON', "-DCMAKE_INCLUDE_PATH=$prefix/include",
            "-DCMAKE_LIBRARY_PATH=$prefix/lib", "-DCMAKE_CXX_FLAGS=/DWIN32 /D_WINDOWS /W3 /GR /EHsc /MP4 /std:c++17 /I`"$prefix/include`"",
            "-DCMAKE_EXE_LINKER_FLAGS=/LIBPATH:`"$prefix/lib`"") }
    }
    $depBuild = Join-Path $output "compile/$dep"
    Checked $cmake (@('-S', (Join-Path $prepared "deps/$dep"), '-B', $depBuild) + $common + $options)
    if ($dep -eq 'opencc') {
        Assert-MoRuntimeSourcePaths (Join-Path $depBuild 'CMakeCache.txt') @{ LIBMARISA = (Join-Path $prefix 'lib/marisa.lib') }
    }
    Checked $cmake @('--build', $depBuild, '--config', 'Release', '--target', 'install', '--parallel', '4')
}
# OpenCC must not install its bundled older Marisa over the pinned core library.
$marisaBuilt = Join-Path $output 'compile/marisa-trie/Release/marisa.lib'
if ((Get-FileHash -LiteralPath $marisaBuilt).Hash -ne (Get-FileHash -LiteralPath (Join-Path $prefix 'lib/marisa.lib')).Hash) {
    throw 'Runtime Marisa library was replaced after its pinned build.'
}
$previousPlugins = [Environment]::GetEnvironmentVariable('RIME_PLUGINS', 'Process')
try {
    $env:RIME_PLUGINS = 'lua'
    $rimeBuild = Join-Path $output 'compile/rime'
    Checked $cmake @('-S', $prepared, '-B', $rimeBuild, '-G', 'Visual Studio 17 2022', '-A', 'x64',
        '-DCMAKE_CONFIGURATION_TYPES=Release', '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded',
        '-DCMAKE_POLICY_DEFAULT_CMP0091=NEW',
        '-DBUILD_STATIC=ON', '-DBUILD_SHARED_LIBS=ON', '-DBUILD_MERGED_PLUGINS=ON',
        '-DBUILD_SEPARATE_LIBS=OFF', '-DENABLE_EXTERNAL_PLUGINS=OFF', '-DENABLE_LOGGING=OFF',
        '-DBUILD_TEST=OFF', '-DBUILD_DATA=OFF', '-DENABLE_TIMESTAMP=OFF',
        "-DCMAKE_INCLUDE_PATH=$prefix/include", "-DCMAKE_LIBRARY_PATH=$prefix/lib", "-DBOOST_ROOT=$boost",
        "-DCMAKE_INSTALL_PREFIX=$dist", "-DCMAKE_PROJECT_rime_INCLUDE=$repoRoot/native/librime/preparation/mo_project.cmake")
    Assert-MoRuntimeSourcePaths (Join-Path $rimeBuild 'CMakeCache.txt') @{
        LevelDb_LIBRARY = (Join-Path $prefix 'lib/leveldb.lib'); LevelDb_INCLUDE_PATH = "$prefix/include"
        Marisa_LIBRARY = (Join-Path $prefix 'lib/marisa.lib'); Marisa_INCLUDE_PATH = "$prefix/include"
        Opencc_LIBRARY = (Join-Path $prefix 'lib/opencc.lib'); Opencc_INCLUDE_PATH = "$prefix/include"
        YamlCpp_LIBRARY = (Join-Path $prefix 'lib/yaml-cpp.lib'); YamlCpp_INCLUDE_PATH = "$prefix/include"
        YamlCpp_NEW_API = "$prefix/include"; Boost_INCLUDE_DIR = $boost
    }
    Checked $cmake @('--build', $rimeBuild, '--config', 'Release', '--target', 'install', '--parallel', '4')
} finally { [Environment]::SetEnvironmentVariable('RIME_PLUGINS', $previousPlugins, 'Process') }
Copy-Item -LiteralPath (Join-Path $prefix 'share/opencc') -Destination (Join-Path $dist 'share/opencc') -Recurse

# Build provenance, NOT a signed release manifest or permission to distribute.
$provenance = [ordered]@{ format = 1; development_only = $true; redistributable = $false;
    plugins = @('lua'); preparation_abi = 1; inputs = $inputHashes;
    cmake_archive_sha256 = (Get-FileHash -LiteralPath $cmakeArchive).Hash;
    boost_archive_sha256 = (Get-FileHash -LiteralPath $boostArchive).Hash;
    lua_archive_sha256 = (Get-FileHash -LiteralPath $luaArchive).Hash;
    patch_sha256 = (Get-FileHash -LiteralPath $patch).Hash;
    lua_patch_sha256 = (Get-FileHash -LiteralPath $luaPatch).Hash;
    preparation_source_sha256 = (Get-FileHash -LiteralPath (Join-Path $repoRoot 'native/librime/preparation/mo_preparation.cc')).Hash;
    project_hook_sha256 = (Get-FileHash -LiteralPath (Join-Path $repoRoot 'native/librime/preparation/mo_project.cmake')).Hash;
    dll_sha256 = (Get-FileHash -LiteralPath (Join-Path $dist 'lib/rime.dll')).Hash }
$provenance | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $dist 'mo-build-provenance.json') -Encoding utf8NoBOM
Write-Host "Mo core+Lua development runtime built: $dist"
