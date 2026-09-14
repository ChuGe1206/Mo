[CmdletBinding()]
param(
    [string]$BinaryRoot = (Join-Path $PSScriptRoot 'out\msbuild')
)

$ErrorActionPreference = 'Stop'

function Resolve-Artifact([string]$Path, [string]$Label) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Label was not found: $Path. Run build-probe.ps1 first."
    }
    return (Resolve-Path -LiteralPath $Path).Path
}

$registrar = Resolve-Artifact (Join-Path $BinaryRoot 'x64\Release\mo_tip_registrar.exe') 'x64 registrar'
$tipX64 = Resolve-Artifact (Join-Path $BinaryRoot 'x64\Release\mo_tip.dll') 'x64 TIP'
$tipX86 = Resolve-Artifact (Join-Path $BinaryRoot 'Win32\Release\mo_tip.dll') 'x86 TIP'

for ($attempt = 1; $attempt -le 2; $attempt++) {
    & $registrar self-test-registry $tipX64 $tipX86
    if ($LASTEXITCODE -ne 0) {
        throw "Registrar isolated registry self-test attempt $attempt failed with exit code $LASTEXITCODE."
    }
}

Write-Host 'Registrar isolated x64/x86 registry self-test passed twice and cleaned its test CLSID.'
Write-Host 'This test did not register or enable the Mo input profile.'
