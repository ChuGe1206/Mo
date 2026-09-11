[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $LibrimeDistDir,

    [Parameter(Mandatory = $true)]
    [string] $SharedDataDir,

    [Parameter(Mandatory = $true)]
    [string] $UserDataDir,

    [switch] $Deploy
)

$ErrorActionPreference = 'Stop'

function Resolve-Directory([string] $Path, [string] $Label) {
    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        throw "$Label does not exist: $Path"
    }
    return (Resolve-Path -LiteralPath $Path).Path
}

function Invoke-Checked([string] $Program, [string[]] $Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $Program"
    }
}

$dist = Resolve-Directory $LibrimeDistDir 'librime dist directory'
$shared = Resolve-Directory $SharedDataDir 'Rime shared data directory'
$user = Resolve-Directory $UserDataDir 'Rime user data directory'
$libraryDirectory = Resolve-Directory (Join-Path $dist 'lib') 'librime library directory'
$deployer = Join-Path $dist 'bin\rime_deployer.exe'
$importLibrary = Join-Path $libraryDirectory 'rime.lib'
$dynamicLibrary = Join-Path $libraryDirectory 'rime.dll'
foreach ($artifact in @($deployer, $importLibrary, $dynamicLibrary)) {
    if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) {
        throw "Required librime artifact does not exist: $artifact"
    }
}

if ($Deploy) {
    Push-Location $libraryDirectory
    try {
        Invoke-Checked $deployer @('--build', $user, $shared, (Join-Path $user 'build'))
    } finally {
        Pop-Location
    }
}

$manifest = (Resolve-Path -LiteralPath "$PSScriptRoot\..\..\crates\mo-rime\Cargo.toml").Path
$previousLibraryDirectory = $env:MO_LIBRIME_LIB_DIR
try {
    $env:MO_LIBRIME_LIB_DIR = $libraryDirectory
    Push-Location $libraryDirectory
    try {
        Invoke-Checked 'cargo' @(
            '+stable', 'run', '--quiet', '--manifest-path', $manifest,
            '--features', 'link-dynamic', '--example', 'real_smoke', '--',
            $shared, $user
        )
    } finally {
        Pop-Location
    }
} finally {
    $env:MO_LIBRIME_LIB_DIR = $previousLibraryDirectory
}
