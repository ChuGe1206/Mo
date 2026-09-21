# Deterministic source-material and license-document packaging helpers.
# A technically complete inventory is not a legal conclusion or release approval.
Set-StrictMode -Version Latest

function Read-MoReleaseMaterialsPolicy([string]$Path) {
    $policy = Read-MoStageJson (Assert-MoPlainPath $Path)
    $required = @('format', 'status', 'archives', 'documents')
    if ($policy.Count -ne $required.Count -or
        @($required | Where-Object { -not $policy.Contains($_) }).Count -or
        $policy['format'] -ne 1 -or $policy['status'] -cne 'phase-0-technical-materials' -or
        $policy['archives'] -isnot [Collections.IList] -or $policy['archives'].Count -ne 9 -or
        $policy['documents'] -isnot [Collections.IList] -or $policy['documents'].Count -ne 15) {
        throw 'Invalid release materials policy header.'
    }
    foreach ($group in @('archives', 'documents')) {
        $ids = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($entry in $policy[$group]) {
            $entryRequired = if ($group -ceq 'archives') { @('id', 'file_name', 'origin', 'sha256') } else {
                @('id', 'component', 'file_name', 'origin', 'sha256')
            }
            if ($entry -isnot [Collections.IDictionary]) {
                throw "Invalid or duplicate release materials $group entry."
            }
            $expectedCount = if ($group -ceq 'archives' -and $entry.Contains('gpl_corresponding_source')) { 5 } else { $entryRequired.Count }
            if ($entry.Count -ne $expectedCount -or
                @($entryRequired | Where-Object { -not $entry.Contains($_) }).Count -or
                $entry['id'] -cnotmatch '^[a-z0-9-]+$' -or
                $entry['file_name'] -cnotmatch '^[A-Za-z0-9][A-Za-z0-9._-]+$' -or
                $entry['origin'] -isnot [string] -or $entry['sha256'] -cnotmatch '^[A-F0-9]{64}$' -or
                -not $ids.Add($entry['id']) -or -not $names.Add($entry['file_name'])) {
                throw "Invalid or duplicate release materials $group entry."
            }
            if ($entry.Contains('gpl_corresponding_source') -and
                $entry['gpl_corresponding_source'] -isnot [bool]) {
                throw 'Invalid GPL corresponding-source marker.'
            }
        }
    }
    $gpl = @($policy['archives'] | Where-Object { $_.Contains('gpl_corresponding_source') -and $_['gpl_corresponding_source'] })
    if ($gpl.Count -ne 1 -or $gpl[0]['id'] -cne 'rime-ice') {
        throw 'Release materials policy must identify the exact rime-ice corresponding source.'
    }
    return $policy
}

function Resolve-MoReleaseMaterialOrigin(
    [string]$Origin,
    [string]$Repository,
    [string]$Stage,
    [string]$RuntimeBuild,
    [string]$CargoRegistrySource,
    [string]$BoostArchive,
    [string]$LuaArchive
) {
    $path = switch -Regex -CaseSensitive ($Origin) {
        '^repo:(.+)$' { Assert-MoRelativeName $Matches[1]; Join-Path $Repository $Matches[1]; break }
        '^stage:(.+)$' { Assert-MoRelativeName $Matches[1]; Join-Path $Stage $Matches[1]; break }
        '^runtime:(.+)$' { Assert-MoRelativeName $Matches[1]; Join-Path $RuntimeBuild $Matches[1]; break }
        '^cargo:(.+)$' { Assert-MoRelativeName $Matches[1]; Join-Path $CargoRegistrySource $Matches[1]; break }
        '^boost-archive$' { $BoostArchive; break }
        '^lua-archive$' { $LuaArchive; break }
        default { throw "Unknown release material origin: $Origin" }
    }
    $resolved = Assert-MoPlainPath $path
    if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
        throw "Release material input is missing: $Origin"
    }
    return $resolved
}

function Get-MoReleaseMaterialsModel(
    [string]$StageDirectory,
    [string]$RuntimeBuildDirectory,
    [string]$CargoRegistrySourceDirectory,
    [string]$BoostArchivePath,
    [string]$LuaArchivePath,
    [string]$ComplianceDirectory,
    [string]$PolicyPath,
    [string]$CompliancePolicyPath
) {
    $repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    $stage = Assert-MoPlainPath $StageDirectory
    $runtime = Assert-MoPlainPath $RuntimeBuildDirectory
    $cargo = Assert-MoPlainPath $CargoRegistrySourceDirectory
    $boost = Assert-MoPlainPath $BoostArchivePath
    $lua = Assert-MoPlainPath $LuaArchivePath
    $compliance = Assert-MoPlainPath $ComplianceDirectory
    $null = Assert-MoPreparedStage $stage
    & (Join-Path $PSScriptRoot 'verify-release-compliance.ps1') -StageDirectory $stage `
        -ComplianceDirectory $compliance -PolicyPath $CompliancePolicyPath | Out-Null
    $policy = Read-MoReleaseMaterialsPolicy $PolicyPath
    $compliancePolicy = Read-MoReleaseCompliancePolicy $CompliancePolicyPath
    $runtimeProvenance = Read-MoStageJson (Join-Path $stage 'evidence/runtime-provenance.json')
    $rimeData = Read-MoStageJson (Join-Path $stage 'evidence/rime-data.json')
    Assert-MoComplianceProvenance $compliancePolicy `
        (Read-MoStageJson (Join-Path $stage 'evidence/build-receipt.json')) $runtimeProvenance $rimeData

    $records = [ordered]@{ archives = [Collections.Generic.List[object]]::new(); documents = [Collections.Generic.List[object]]::new() }
    foreach ($group in @('archives', 'documents')) {
        foreach ($entry in $policy[$group]) {
            $source = Resolve-MoReleaseMaterialOrigin $entry['origin'] $repo $stage $runtime $cargo $boost $lua
            $hash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
            if ($hash -cne $entry['sha256']) { throw "Release material hash mismatch: $($entry['id'])" }
            $record = [ordered]@{
                id = $entry['id']; file_name = $entry['file_name']; source = $source
                size = (Get-Item -LiteralPath $source).Length; sha256 = $hash
            }
            if ($group -ceq 'documents') { $record['component'] = $entry['component'] }
            if ($entry.Contains('gpl_corresponding_source')) {
                $record['gpl_corresponding_source'] = $entry['gpl_corresponding_source']
            }
            $records[$group].Add($record)
        }
    }
    return [ordered]@{
        repository = $repo; stage = $stage; runtime = $runtime; compliance = $compliance
        policy = $policy; records = $records
        stage_manifest_sha256 = (Get-FileHash (Join-Path $stage 'mo-stage.json') -Algorithm SHA256).Hash
        compliance_evidence_sha256 = (Get-FileHash (Join-Path $compliance 'release-compliance-evidence.json') -Algorithm SHA256).Hash
        policy_sha256 = (Get-FileHash (Assert-MoPlainPath $PolicyPath) -Algorithm SHA256).Hash
    }
}

function ConvertTo-MoReleaseMaterialsManifest([Collections.IDictionary]$Model) {
    $archives = @($Model['records']['archives'] | ForEach-Object {
        $entry = [ordered]@{ id = $_['id']; path = 'sources/' + $_['file_name']; size = $_['size']; sha256 = $_['sha256'] }
        if ($_.Contains('gpl_corresponding_source')) { $entry['gpl_corresponding_source'] = $_['gpl_corresponding_source'] }
        $entry
    })
    $documents = @($Model['records']['documents'] | ForEach-Object {
        [ordered]@{ id = $_['id']; component = $_['component']; path = 'licenses/' + $_['file_name']; size = $_['size']; sha256 = $_['sha256'] }
    })
    return [ordered]@{
        format = 1; kind = 'mo-release-source-and-license-materials'
        development_only = $true; technical_materials_complete = $true
        legal_review_complete = $false; release_authorized = $false
        stage_manifest_sha256 = $Model['stage_manifest_sha256']
        compliance_evidence_sha256 = $Model['compliance_evidence_sha256']
        policy_sha256 = $Model['policy_sha256']
        archives = $archives; documents = $documents
    }
}
