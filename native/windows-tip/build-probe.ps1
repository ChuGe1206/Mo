[CmdletBinding()]
param(
    [ValidateSet('All', 'x64', 'Win32')]
    [string]$Architecture = 'All',
    [ValidateSet('Auto', 'CMake', 'MSBuild')]
    [string]$Backend = 'Auto'
)

$ErrorActionPreference = 'Stop'
$sourceRoot = $PSScriptRoot

# Some orchestrated shells expose both `Path` and `PATH`. Legacy MSBuild uses a
# case-insensitive dictionary and refuses to launch CL when both are present.
# Normalize only this script process; the caller's environment is untouched.
$processPath = [Environment]::GetEnvironmentVariable('PATH', 'Process')
Remove-Item Env:Path -ErrorAction SilentlyContinue
$env:Path = $processPath

function Find-MSBuild {
    $command = Get-Command msbuild -ErrorAction SilentlyContinue
    if ($null -ne $command) { return $command.Source }

    $programFilesX86 = [Environment]::GetFolderPath('ProgramFilesX86')
    $vswhere = Join-Path $programFilesX86 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path -LiteralPath $vswhere) {
        $candidate = & $vswhere -latest -products * -find 'MSBuild\**\Bin\MSBuild.exe' | Select-Object -First 1
        if ($candidate) { return $candidate }
    }
    return $null
}

function Find-CMake {
    $command = Get-Command cmake -ErrorAction SilentlyContinue
    if ($null -ne $command) { return $command.Source }
    return $null
}

function Invoke-Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $Program $($Arguments -join ' ')"
    }
}

$architectures = if ($Architecture -eq 'All') { @('x64', 'Win32') } else { @($Architecture) }
$cmake = Find-CMake
$msbuild = Find-MSBuild
if ($Backend -eq 'Auto') {
    if ($cmake) { $Backend = 'CMake' }
    elseif ($msbuild) { $Backend = 'MSBuild' }
    else { throw 'Neither CMake nor MSBuild was found. Install Visual Studio C++ build tools and a Windows SDK.' }
}
if ($Backend -eq 'CMake' -and -not $cmake) { throw 'CMake was requested but was not found on PATH.' }
if ($Backend -eq 'MSBuild' -and -not $msbuild) { throw 'MSBuild was requested but Visual Studio Build Tools were not found.' }

foreach ($platform in $architectures) {
    if ($Backend -eq 'CMake') {
        $buildDirectory = Join-Path $sourceRoot "out\cmake\$platform"
        Invoke-Checked $cmake @('-S', $sourceRoot, '-B', $buildDirectory, '-G', 'Visual Studio 17 2022', '-A', $platform)
        Invoke-Checked $cmake @('--build', $buildDirectory, '--config', 'Release')
        $binaryDirectory = Join-Path $buildDirectory 'Release'
    } else {
        foreach ($project in @('MoTip.vcxproj', 'MoTipRegistrar.vcxproj', 'MoTipAbiProbe.vcxproj')) {
            Invoke-Checked $msbuild @((Join-Path $sourceRoot $project), '/m', '/nologo', '/t:Build', '/p:Configuration=Release', "/p:Platform=$platform")
        }
        $binaryDirectory = Join-Path $sourceRoot "out\msbuild\$platform\Release"
    }

    $probe = Join-Path $binaryDirectory 'mo_tip_abi_probe.exe'
    $tip = Join-Path $binaryDirectory 'mo_tip.dll'
    if (-not (Test-Path -LiteralPath $probe) -or -not (Test-Path -LiteralPath $tip)) {
        throw "Expected probe artifacts were not produced in $binaryDirectory"
    }
    Invoke-Checked $probe @($tip)
}

Write-Host 'Compile/load probes passed. This does not validate TSF registration, input, named pipes, ACLs, or AppContainer hosts.'
