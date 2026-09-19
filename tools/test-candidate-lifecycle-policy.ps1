#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$StageDirectory)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
. (Join-Path $repo 'installer/windows/staging-policy.ps1')
. (Join-Path $repo 'installer/windows/test-fixture.ps1')
$stage = Assert-MoPlainPath $StageDirectory
$null = Assert-MoPreparedStage $stage
$fixture = Join-Path $repo ('build/mo-candidate-policy-' + [Guid]::NewGuid().ToString('N'))
$null = Assert-MoNewBuildOutput $fixture $repo
New-Item -ItemType Directory -Path $fixture | Out-Null
$candidateEntry = Join-Path $PSScriptRoot 'candidate-lifecycle-probe.ps1'
$passed = 0
function Expect-Rejection([string]$Label, [hashtable]$Arguments, [string]$Expected) {
    try { & $candidateEntry @Arguments; throw 'Unexpected acceptance.' }
    catch {
        if ($_.Exception.Message -notmatch $Expected) { throw "Unexpected rejection for ${Label}: $($_.Exception.Message)" }
    }
    $script:passed++
    Write-Host "PASS $Label"
}
try {
    $validOutput = Join-Path $fixture 'must-not-be-created'
    Expect-Rejection 'relative stage' @{StageDirectory='relative-stage'; OutputDirectory=$validOutput} 'absolute'
    Expect-Rejection 'missing stage' @{StageDirectory=(Join-Path $fixture 'missing'); OutputDirectory=$validOutput} 'does not exist'
    Expect-Rejection 'relative output' @{StageDirectory=$stage; OutputDirectory='relative-output'} 'absolute'
    Expect-Rejection 'repository output' @{StageDirectory=$stage; OutputDirectory=$repo} 'new child'
    Expect-Rejection 'build root output' @{StageDirectory=$stage; OutputDirectory=(Join-Path $repo 'build')} 'new child'
    Expect-Rejection 'existing output' @{StageDirectory=$stage; OutputDirectory=$fixture} 'overwrite'
    Expect-Rejection 'invalid repetition' @{StageDirectory=$stage; OutputDirectory=$validOutput; Repetitions=0} '0.*less than|range'
    Expect-Rejection 'invalid toolchain parameter' @{StageDirectory=$stage; OutputDirectory=$validOutput; RustToolchain='bad/toolchain'} 'pattern'
    $previousAuto = [Environment]::GetEnvironmentVariable('RUSTUP_AUTO_INSTALL', 'Process')
    $missingToolchain = 'mo-candidate-missing-' + [Guid]::NewGuid().ToString('N')
    Expect-Rejection 'missing compiler without download' @{StageDirectory=$stage; OutputDirectory=$validOutput; RustToolchain=$missingToolchain} 'installed pinned Rust'
    if ([Environment]::GetEnvironmentVariable('RUSTUP_AUTO_INSTALL', 'Process') -cne $previousAuto) { throw 'Rustup process preference was not restored.' }
    ++$passed
    Write-Host 'PASS rustup preference restored'
    try {
        foreach ($autoPreference in @('0', '1', '')) {
            [Environment]::SetEnvironmentVariable('RUSTUP_AUTO_INSTALL', $autoPreference, 'Process')
            Expect-Rejection "missing compiler with preference length=$($autoPreference.Length)" @{StageDirectory=$stage; OutputDirectory=$validOutput; RustToolchain=$missingToolchain} 'installed pinned Rust'
            if ([Environment]::GetEnvironmentVariable('RUSTUP_AUTO_INSTALL', 'Process') -cne $autoPreference) { throw 'Explicit rustup preference was not restored.' }
            ++$passed
            Write-Host 'PASS explicit rustup preference restored'
        }
    } finally {
        if ($null -eq $previousAuto) { Remove-Item Env:RUSTUP_AUTO_INSTALL -ErrorAction SilentlyContinue }
        else { [Environment]::SetEnvironmentVariable('RUSTUP_AUTO_INSTALL', $previousAuto, 'Process') }
    }
    if (Test-Path -LiteralPath $validOutput) { throw 'Invalid invocation created build output.' }
    ++$passed
    Write-Host 'PASS invalid invocations created no build output'
    $null = Assert-MoPreparedStage $stage
    ++$passed
    Write-Host "Candidate lifecycle policy tests passed: $passed. No compilation, registration, installation or network access."
    # The missing-toolchain negative cases intentionally leave rustup's native
    # exit code nonzero. Clear only that process-status channel after every policy
    # assertion has passed so callers do not mistake a successful script for a
    # native-tool failure.
    $global:LASTEXITCODE = 0
} finally {
    $resolved = Assert-MoPlainPath $fixture
    if (-not $resolved.StartsWith((Join-Path $repo 'build') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe policy fixture cleanup target.' }
    Assert-MoOwnedFixtureTree $resolved
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
