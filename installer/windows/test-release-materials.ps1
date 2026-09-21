#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$StageDirectory,
    [Parameter(Mandatory)][string]$RuntimeBuildDirectory,
    [Parameter(Mandatory)][string]$CargoRegistrySourceDirectory,
    [Parameter(Mandatory)][string]$BoostArchivePath,
    [Parameter(Mandatory)][string]$LuaArchivePath,
    [Parameter(Mandatory)][string]$ComplianceDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'test-fixture.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $repo ('build/mo-release-materials-test-' + [Guid]::NewGuid().ToString('N'))
$null = Assert-MoNewBuildOutput $fixture $repo
New-Item -ItemType Directory $fixture | Out-Null
$script:testCount = 0
function Pass([string]$Label, [scriptblock]$Action) { & $Action; $script:testCount++; Write-Host "PASS $Label" }
function Reject([string]$Label, [scriptblock]$Action, [string]$Pattern) {
    $rejected = $false
    try { & $Action | Out-Null } catch { if ($_.Exception.Message -notmatch $Pattern) { throw }; $rejected = $true }
    if (-not $rejected) { throw "Expected rejection: $Label" }
    $script:testCount++; Write-Host "PASS $Label rejected"
}
$common = @{
    StageDirectory = (Assert-MoPlainPath $StageDirectory)
    RuntimeBuildDirectory = (Assert-MoPlainPath $RuntimeBuildDirectory)
    CargoRegistrySourceDirectory = (Assert-MoPlainPath $CargoRegistrySourceDirectory)
    BoostArchivePath = (Assert-MoPlainPath $BoostArchivePath)
    LuaArchivePath = (Assert-MoPlainPath $LuaArchivePath)
    ComplianceDirectory = (Assert-MoPlainPath $ComplianceDirectory)
}
try {
    $materials = Join-Path $fixture 'materials'
    Pass 'prepare hash-bound release materials' {
        & (Join-Path $PSScriptRoot 'prepare-release-materials.ps1') @common -OutputDirectory $materials
    }
    Pass 'independent release materials verification' {
        & (Join-Path $PSScriptRoot 'verify-release-materials.ps1') @common -MaterialsDirectory $materials
    }
    Pass 'nine archives and fifteen documents' {
        $manifest = Read-MoStageJson (Join-Path $materials 'materials-manifest.json')
        if ($manifest['archives'].Count -ne 9 -or $manifest['documents'].Count -ne 15 -or
            @($manifest['archives'] | Where-Object { $_['gpl_corresponding_source'] -eq $true }).Count -ne 1 -or
            $manifest['legal_review_complete'] -ne $false -or $manifest['release_authorized'] -ne $false) {
            throw 'Unexpected materials manifest shape.'
        }
    }
    $policyPath = Join-Path $PSScriptRoot 'release-materials-policy.json'
    $badPolicy = Join-Path $fixture 'bad-policy.json'
    $policy = Read-MoStageJson $policyPath
    $policy['archives'][6]['sha256'] = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
    $policy | ConvertTo-Json -Depth 10 | Set-Content $badPolicy -Encoding utf8NoBOM
    Reject 'stale Boost archive pin' {
        & (Join-Path $PSScriptRoot 'prepare-release-materials.ps1') @common `
            -PolicyPath $badPolicy -OutputDirectory (Join-Path $fixture 'bad')
    } 'hash mismatch'
    $policy = Read-MoStageJson $policyPath
    $policy['archives'][8].Remove('gpl_corresponding_source')
    $policy | ConvertTo-Json -Depth 10 | Set-Content $badPolicy -Encoding utf8NoBOM
    Reject 'missing GPL source marker' {
        & (Join-Path $PSScriptRoot 'prepare-release-materials.ps1') @common `
            -PolicyPath $badPolicy -OutputDirectory (Join-Path $fixture 'bad-gpl')
    } 'corresponding source'
    $license = Join-Path $materials 'licenses/mo-APACHE-2.0.txt'
    $originalLicense = [IO.File]::ReadAllBytes($license)
    Add-Content $license 'unexpected' -Encoding utf8NoBOM
    Reject 'tampered license text' {
        & (Join-Path $PSScriptRoot 'verify-release-materials.ps1') @common -MaterialsDirectory $materials
    } 'output hash mismatch'
    [IO.File]::WriteAllBytes($license, $originalLicense)
    $evidencePath = Join-Path $materials 'release-materials-evidence.json'
    $originalEvidence = [IO.File]::ReadAllBytes($evidencePath)
    $evidence = Read-MoStageJson $evidencePath
    $evidence['release_authorized'] = $true
    $evidence | ConvertTo-Json -Depth 5 | Set-Content $evidencePath -Encoding utf8NoBOM
    Reject 'forged release authorization' {
        & (Join-Path $PSScriptRoot 'verify-release-materials.ps1') @common -MaterialsDirectory $materials
    } 'evidence mismatch'
    [IO.File]::WriteAllBytes($evidencePath, $originalEvidence)
    Set-Content (Join-Path $materials 'extra.txt') 'unexpected'
    Reject 'extra materials output' {
        & (Join-Path $PSScriptRoot 'verify-release-materials.ps1') @common -MaterialsDirectory $materials
    } 'inventory mismatch'
    Write-Host "Release materials tests passed: $script:testCount. No installation, registration, signing or network access."
} finally {
    $resolved = Assert-MoPlainPath $fixture
    Assert-MoOwnedFixtureTree $resolved
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
