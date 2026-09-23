#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$StageDirectory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'test-fixture.ps1')
. (Join-Path $PSScriptRoot 'release-compliance.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $repo ('build/mo-release-compliance-test-' + [Guid]::NewGuid().ToString('N'))
$null = Assert-MoNewBuildOutput $fixture $repo
New-Item -ItemType Directory -Path $fixture | Out-Null
$script:testCount = 0

function Pass([string]$Label, [scriptblock]$Action) {
    & $Action
    $script:testCount++
    Write-Host "PASS $Label"
}
function Reject([string]$Label, [scriptblock]$Action, [string]$Pattern) {
    $rejected = $false
    try { & $Action | Out-Null } catch {
        if ($_.Exception.Message -notmatch $Pattern) { throw }
        $rejected = $true
    }
    if (-not $rejected) { throw "Expected rejection: $Label" }
    $script:testCount++
    Write-Host "PASS $Label rejected"
}
try {
    $stage = Assert-MoPlainPath $StageDirectory
    $policyPath = Join-Path $PSScriptRoot 'release-compliance-policy.json'
    $first = Join-Path $fixture 'first'
    $second = Join-Path $fixture 'second'
    Pass 'first real-stage compliance draft' {
        & (Join-Path $PSScriptRoot 'prepare-release-compliance.ps1') `
            -StageDirectory $stage -OutputDirectory $first -PolicyPath $policyPath
    }
    Pass 'independent compliance verification' {
        & (Join-Path $PSScriptRoot 'verify-release-compliance.ps1') `
            -StageDirectory $stage -ComplianceDirectory $first -PolicyPath $policyPath
    }
    Pass 'deterministic compliance bytes' {
        & (Join-Path $PSScriptRoot 'prepare-release-compliance.ps1') `
            -StageDirectory $stage -OutputDirectory $second -PolicyPath $policyPath
        foreach ($name in Get-MoStageFiles $first) {
            if ((Get-FileHash -LiteralPath (Join-Path $first $name) -Algorithm SHA256).Hash -cne
                (Get-FileHash -LiteralPath (Join-Path $second $name) -Algorithm SHA256).Hash) {
                throw "Compliance output is not deterministic: $name"
            }
        }
    }
    Pass 'exact 132-file component coverage' {
        $model = Get-MoReleaseComplianceModel $stage $policyPath
        if ($model['files'].Count -ne 132 -or
            ($model['component_counts'].Values | Measure-Object -Sum).Sum -ne 132 -or
            $model['component_counts']['mo-settings-app'] -ne 1 -or
            $model['component_counts']['rime-ice'] -ne 93 -or
            $model['component_counts']['opencc-data'] -ne 30) {
            throw 'Unexpected release component coverage.'
        }
    }

    $policy = Read-MoStageJson $policyPath
    $missingPolicy = Join-Path $fixture 'missing-policy.json'
    $policy['file_rules'][0]['pattern'] = '^bin/not-mo-broker\\.exe$'
    $policy | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $missingPolicy -Encoding utf8NoBOM
    Reject 'unmapped payload' { Get-MoReleaseComplianceModel $stage $missingPolicy } 'exactly one'

    $policy = Read-MoStageJson $policyPath
    $overlapPolicy = Join-Path $fixture 'overlap-policy.json'
    $policy['file_rules'][5]['pattern'] = '^runtime/librime/opencc/.+'
    $policy | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $overlapPolicy -Encoding utf8NoBOM
    Reject 'overlapping ownership' { Get-MoReleaseComplianceModel $stage $overlapPolicy } 'exactly one'

    $policy = Read-MoStageJson $policyPath
    $pinPolicy = Join-Path $fixture 'pin-policy.json'
    $policy['cargo_lock_sha256'] = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
    $policy | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $pinPolicy -Encoding utf8NoBOM
    Reject 'stale Cargo lock provenance' { Get-MoReleaseComplianceModel $stage $pinPolicy } 'Cargo.lock pin'

    $policy = Read-MoStageJson $policyPath
    $archivePolicy = Join-Path $fixture 'archive-policy.json'
    $boostPackages = @($policy['packages'] | Where-Object { $_['id'] -ceq 'boost' })
    $boostPackages[0]['archive_sha256'] =
        'BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB'
    $policy | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $archivePolicy -Encoding utf8NoBOM
    Reject 'package archive detached from provenance' {
        Get-MoReleaseComplianceModel $stage $archivePolicy
    } 'source archive does not match'

    $tampered = Join-Path $fixture 'tampered'
    Copy-Item -LiteralPath $first -Destination $tampered -Recurse
    Add-Content -LiteralPath (Join-Path $tampered 'mo-windows-payload.spdx.json') -Value ' ' -Encoding utf8NoBOM
    Reject 'tampered SPDX draft' {
        & (Join-Path $PSScriptRoot 'verify-release-compliance.ps1') `
            -StageDirectory $stage -ComplianceDirectory $tampered -PolicyPath $policyPath
    } 'SPDX draft'

    $tamperedNotice = Join-Path $fixture 'tampered-notice'
    Copy-Item -LiteralPath $first -Destination $tamperedNotice -Recurse
    Add-Content -LiteralPath (Join-Path $tamperedNotice 'THIRD_PARTY_NOTICES.draft.txt') `
        -Value 'unexpected' -Encoding utf8NoBOM
    Reject 'tampered notices draft' {
        & (Join-Path $PSScriptRoot 'verify-release-compliance.ps1') `
            -StageDirectory $stage -ComplianceDirectory $tamperedNotice -PolicyPath $policyPath
    } 'notices draft'

    $unsafeEvidence = Join-Path $fixture 'unsafe-evidence'
    Copy-Item -LiteralPath $first -Destination $unsafeEvidence -Recurse
    $evidencePath = Join-Path $unsafeEvidence 'release-compliance-evidence.json'
    $evidence = Read-MoStageJson $evidencePath
    $evidence['release_authorized'] = $true
    $evidence | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $evidencePath -Encoding utf8NoBOM
    Reject 'forged release authorization' {
        & (Join-Path $PSScriptRoot 'verify-release-compliance.ps1') `
            -StageDirectory $stage -ComplianceDirectory $unsafeEvidence -PolicyPath $policyPath
    } 'evidence header'

    $extraOutput = Join-Path $fixture 'extra-output'
    Copy-Item -LiteralPath $first -Destination $extraOutput -Recurse
    Set-Content -LiteralPath (Join-Path $extraOutput 'extra.txt') -Value 'unexpected' -Encoding utf8NoBOM
    Reject 'extra compliance output' {
        & (Join-Path $PSScriptRoot 'verify-release-compliance.ps1') `
            -StageDirectory $stage -ComplianceDirectory $extraOutput -PolicyPath $policyPath
    } 'inventory mismatch'

    Write-Host "Release compliance tests passed: $script:testCount. No installation, registration, signing, legal approval or network access."
} finally {
    $resolved = Assert-MoPlainPath $fixture
    Assert-MoOwnedFixtureTree $resolved
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
