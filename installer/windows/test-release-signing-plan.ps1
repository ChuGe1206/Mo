#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$StageDirectory,
    [Parameter(Mandatory)][string]$LinkedDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'test-fixture.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $repo ('build/mo-release-signing-test-' + [Guid]::NewGuid().ToString('N'))
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
try {
    $stage = Assert-MoPlainPath $StageDirectory
    $linked = Assert-MoPlainPath $LinkedDirectory
    $evidence = Join-Path $linked 'verification/linked-installer-evidence.json'
    $msi = Join-Path $linked 'mo-production-shape-unsigned.msi'
    $bundle = Join-Path $linked 'mo-setup-production-shape-unsigned.exe'
    $plan = Join-Path $fixture 'plan'
    Pass 'prepare exact unsigned signing plan' {
        & (Join-Path $PSScriptRoot 'prepare-release-signing-plan.ps1') -StageDirectory $stage `
            -LinkedEvidencePath $evidence -MsiPath $msi -BundlePath $bundle -OutputDirectory $plan
    }
    Pass 'independent signing plan verification' {
        & (Join-Path $PSScriptRoot 'verify-release-signing-plan.ps1') -StageDirectory $stage `
            -LinkedEvidencePath $evidence -MsiPath $msi -BundlePath $bundle -SigningPlanDirectory $plan
    }
    Pass 'five inner payloads and six ordered steps' {
        $document = Read-MoStageJson (Join-Path $plan 'release-signing-plan.json')
        if ($document['unsigned_inner_payloads'].Count -ne 5 -or
            $document['unsigned_linked_baselines'].Count -ne 2 -or
            $document['required_sequence'].Count -ne 6 -or
            $document['required_sequence'][5]['action'] -cne 'sign-timestamp-and-verify-bundle' -or
            $document['signing_executed'] -ne $false -or $document['release_authorized'] -ne $false) {
            throw 'Unexpected signing plan shape.'
        }
    }
    $badEvidence = Join-Path $fixture 'development-evidence.json'
    $doc = Read-MoStageJson $evidence
    $doc['build_flavor'] = 'DevelopmentTest'
    $doc['fault_injection_included'] = $true
    $doc | ConvertTo-Json -Depth 5 | Set-Content $badEvidence -Encoding utf8NoBOM
    Reject 'development-test linked evidence' {
        & (Join-Path $PSScriptRoot 'prepare-release-signing-plan.ps1') -StageDirectory $stage `
            -LinkedEvidencePath $badEvidence -MsiPath $msi -BundlePath $bundle -OutputDirectory (Join-Path $fixture 'bad')
    } 'ProductionShape'
    $badHash = Join-Path $fixture 'bad-hash-evidence.json'
    $doc = Read-MoStageJson $evidence
    $doc['msi_sha256'] = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
    $doc | ConvertTo-Json -Depth 5 | Set-Content $badHash -Encoding utf8NoBOM
    Reject 'detached installer hash' {
        & (Join-Path $PSScriptRoot 'prepare-release-signing-plan.ps1') -StageDirectory $stage `
            -LinkedEvidencePath $badHash -MsiPath $msi -BundlePath $bundle -OutputDirectory (Join-Path $fixture 'bad-hash')
    } 'installer hash'
    $planPath = Join-Path $plan 'release-signing-plan.json'
    $original = [IO.File]::ReadAllBytes($planPath)
    Add-Content $planPath ' ' -Encoding utf8NoBOM
    Reject 'tampered signing plan' {
        & (Join-Path $PSScriptRoot 'verify-release-signing-plan.ps1') -StageDirectory $stage `
            -LinkedEvidencePath $evidence -MsiPath $msi -BundlePath $bundle -SigningPlanDirectory $plan
    } 'plan mismatch'
    [IO.File]::WriteAllBytes($planPath, $original)
    Set-Content (Join-Path $plan 'extra.txt') 'unexpected'
    Reject 'extra signing plan output' {
        & (Join-Path $PSScriptRoot 'verify-release-signing-plan.ps1') -StageDirectory $stage `
            -LinkedEvidencePath $evidence -MsiPath $msi -BundlePath $bundle -SigningPlanDirectory $plan
    } 'inventory mismatch'
    Write-Host "Release signing plan tests passed: $script:testCount. No installation, registration, signing or network access."
} finally {
    $resolved = Assert-MoPlainPath $fixture
    Assert-MoOwnedFixtureTree $resolved
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
