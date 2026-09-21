[CmdletBinding()]
param(
    [ValidateSet('All', 'x64', 'Win32')]
    [string]$Architecture = 'All',
    [ValidateSet('Auto', 'CMake', 'MSBuild')]
    [string]$Backend = 'Auto',
    [switch]$LatencyTrace,
    [switch]$DevelopmentFaultInjection
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
        $traceOption = if ($LatencyTrace) { 'ON' } else { 'OFF' }
        $faultOption = if ($DevelopmentFaultInjection) { 'ON' } else { 'OFF' }
        Invoke-Checked $cmake @('-S', $sourceRoot, '-B', $buildDirectory, '-G', 'Visual Studio 17 2022', '-A', $platform, "-DMO_LATENCY_TRACE=$traceOption", "-DMO_DEVELOPMENT_FAULT_INJECTION=$faultOption")
        Invoke-Checked $cmake @('--build', $buildDirectory, '--config', 'Release')
        $binaryDirectory = Join-Path $buildDirectory 'Release'
    } else {
        foreach ($project in @('MoTip.vcxproj', 'MoTipRegistrar.vcxproj', 'MoTipAbiProbe.vcxproj', 'MoTipIpcProbe.vcxproj')) {
            $traceOption = if ($LatencyTrace) { 'true' } else { 'false' }
            $faultOption = if ($DevelopmentFaultInjection) { 'true' } else { 'false' }
            Invoke-Checked $msbuild @((Join-Path $sourceRoot $project), '/m', '/nologo', '/t:Build', '/p:Configuration=Release', "/p:Platform=$platform", "/p:MoLatencyTrace=$traceOption", "/p:MoDevelopmentFaultInjection=$faultOption")
        }
        $binaryDirectory = Join-Path $sourceRoot "out\msbuild\$platform\Release"
    }

    $probe = Join-Path $binaryDirectory 'mo_tip_abi_probe.exe'
    $tip = Join-Path $binaryDirectory 'mo_tip.dll'
    if (-not (Test-Path -LiteralPath $probe) -or -not (Test-Path -LiteralPath $tip)) {
        throw "Expected probe artifacts were not produced in $binaryDirectory"
    }
    Invoke-Checked $probe @($tip)
    $registrar = Join-Path $binaryDirectory 'mo_tip_registrar.exe'
    $transactionMarker = Join-Path $binaryDirectory 'machine-profile-transaction.marker'
    if (-not (Test-Path -LiteralPath $registrar -PathType Leaf) -or (Test-Path -LiteralPath $transactionMarker)) {
        throw "Registrar transaction self-test preflight failed in $binaryDirectory"
    }
    Invoke-Checked $registrar @('self-test-machine-transaction', $transactionMarker)
    if (Test-Path -LiteralPath $transactionMarker) { throw 'Registrar transaction self-test left a marker.' }
    Invoke-Checked $registrar @('self-test-user-finalizer-policy')
    $beforeFailureProbe = @(& $registrar status)
    if ($LASTEXITCODE -ne 0) { throw 'Registrar failure-injection preflight status failed.' }
    $failureProbe = @(& $registrar development-test-fail-fixed 2>&1)
    $failureText = $failureProbe -join "`n"
    if ($DevelopmentFaultInjection) {
        if ($LASTEXITCODE -eq 0 -or $failureText -cnotmatch '(?m)^Operation failed: 0x80004005$' -or
            $failureText -match '(?m)^Usage:$') {
            throw 'Registrar development failure injection did not return deterministic E_FAIL.'
        }
    } elseif ($LASTEXITCODE -eq 0 -or $failureText -cnotmatch '(?m)^Usage:$' -or
        $failureText -cnotmatch '(?m)^Operation failed: 0x80070057$' -or
        $failureText -match '0x80004005') {
        throw 'Production-shape registrar exposes the development failure command.'
    }
    $afterFailureProbe = @(& $registrar status)
    if ($LASTEXITCODE -ne 0 -or ($beforeFailureProbe -join "`n") -cne ($afterFailureProbe -join "`n")) {
        throw 'Registrar development failure injection mutated observable state.'
    }
}

$faultLabel = if ($DevelopmentFaultInjection) { 'included and deterministic' } else { 'physically absent' }
Write-Host "Compile/load plus non-mutating machine transaction and current-user finalizer policy probes passed; development fault injection is $faultLabel. This does not validate TSF registration, input, named pipes, ACLs, or AppContainer hosts."
