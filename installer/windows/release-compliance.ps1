# Build-time SPDX draft generation helpers. These prove inventory coverage and
# provenance pins; they are not legal approval or release authorization.
Set-StrictMode -Version Latest

function Read-MoReleaseCompliancePolicy([string]$Path) {
    $policy = Read-MoStageJson (Assert-MoPlainPath $Path)
    $required = @(
        'format', 'status', 'document_created', 'expected_payload_files',
        'cargo_lock_sha256', 'rime_ice', 'runtime', 'packages', 'file_rules',
        'release_blockers'
    )
    if ($policy.Count -ne $required.Count -or
        @($required | Where-Object { -not $policy.Contains($_) }).Count -or
        $policy['format'] -ne 1 -or $policy['status'] -cne 'phase-0-draft' -or
        $policy['document_created'] -cnotmatch '^\d{4}-\d{2}-\d{2}$' -or
        $policy['expected_payload_files'] -isnot [long] -or
        $policy['expected_payload_files'] -ne 131 -or
        $policy['cargo_lock_sha256'] -cnotmatch '^[A-F0-9]{64}$' -or
        $policy['packages'] -isnot [Collections.IList] -or $policy['packages'].Count -lt 10 -or
        $policy['file_rules'] -isnot [Collections.IList] -or $policy['file_rules'].Count -lt 5 -or
        $policy['release_blockers'] -isnot [Collections.IList] -or $policy['release_blockers'].Count -eq 0) {
        throw 'Invalid release compliance policy header.'
    }
    return $policy
}

function Assert-MoComplianceProvenance(
    [Collections.IDictionary]$Policy,
    [Collections.IDictionary]$BuildReceipt,
    [Collections.IDictionary]$Runtime,
    [Collections.IDictionary]$RimeData
) {
    $cargo = $BuildReceipt['source_files']['Cargo.lock']
    if ($null -eq $cargo -or $cargo['sha256'] -cne $Policy['cargo_lock_sha256']) {
        throw 'Compliance policy Cargo.lock pin does not match the staged build.'
    }
    $ice = $Policy['rime_ice']
    if ($ice.Count -ne 2 -or $ice['commit'] -cne $RimeData['rime_ice_commit'] -or
        $ice['archive_sha256'] -cne $RimeData['source_archive_sha256']) {
        throw 'Compliance policy rime-ice provenance mismatch.'
    }
    $expectedRuntime = $Policy['runtime']
    if ($Runtime['plugins'] -isnot [Collections.IList] -or
        $Runtime['plugins'].Count -ne $expectedRuntime['plugins'].Count -or
        @($expectedRuntime['plugins'] | Where-Object { $_ -cnotin $Runtime['plugins'] }).Count) {
        throw 'Compliance policy runtime plugin allowlist mismatch.'
    }
    if ('octagram' -cin $Runtime['plugins']) { throw 'Forbidden octagram plugin entered the staged runtime.' }
    if ($Runtime['inputs'].Count -ne $expectedRuntime['inputs'].Count) {
        throw 'Compliance policy runtime source count mismatch.'
    }
    foreach ($name in $expectedRuntime['inputs'].Keys) {
        $expected = $expectedRuntime['inputs'][$name]
        $actual = $Runtime['inputs'][$name]
        if ($null -eq $actual -or $actual.Count -ne 2 -or
            $actual['commit'] -cne $expected['commit'] -or
            $actual['archive_sha256'] -cne $expected['archive_sha256']) {
            throw "Compliance policy runtime source mismatch: $name"
        }
    }
    foreach ($name in @('boost_archive_sha256', 'lua_archive_sha256')) {
        if ($Runtime[$name] -cne $expectedRuntime[$name]) {
            throw "Compliance policy runtime archive mismatch: $name"
        }
    }
    $packageArchivePins = [ordered]@{
        'rime-ice' = $RimeData['source_archive_sha256']
        'rime-ice-opencc' = $RimeData['source_archive_sha256']
        'librime' = $Runtime['inputs']['']['archive_sha256']
        'librime-lua' = $Runtime['inputs']['plugins/lua']['archive_sha256']
        'leveldb' = $Runtime['inputs']['deps/leveldb']['archive_sha256']
        'marisa-trie' = $Runtime['inputs']['deps/marisa-trie']['archive_sha256']
        'opencc' = $Runtime['inputs']['deps/opencc']['archive_sha256']
        'opencc-data' = $Runtime['inputs']['deps/opencc']['archive_sha256']
        'yaml-cpp' = $Runtime['inputs']['deps/yaml-cpp']['archive_sha256']
        'boost' = $Runtime['boost_archive_sha256']
        'lua' = $Runtime['lua_archive_sha256']
    }
    foreach ($id in $packageArchivePins.Keys) {
        $matches = @($Policy['packages'] | Where-Object { $_['id'] -ceq $id })
        if ($matches.Count -ne 1 -or $matches[0]['archive_sha256'] -cne $packageArchivePins[$id]) {
            throw "Compliance package source archive does not match staged provenance: $id"
        }
    }
}

function Get-MoReleaseComplianceModel([string]$StageDirectory, [string]$PolicyPath) {
    $stage = Assert-MoPlainPath $StageDirectory
    $null = Assert-MoPreparedStage $stage
    $policy = Read-MoReleaseCompliancePolicy $PolicyPath
    $receipt = Read-MoStageJson (Join-Path $stage 'evidence/build-receipt.json')
    $runtime = Read-MoStageJson (Join-Path $stage 'evidence/runtime-provenance.json')
    $rimeData = Read-MoStageJson (Join-Path $stage 'evidence/rime-data.json')
    Assert-MoComplianceProvenance $policy $receipt $runtime $rimeData

    $packages = [ordered]@{}
    foreach ($package in $policy['packages']) {
        $required = @('id', 'name', 'version', 'license_declared', 'license_concluded',
            'download_location', 'third_party', 'depends_on')
        if ($package -isnot [Collections.IDictionary] -or
            @($required | Where-Object { -not $package.Contains($_) }).Count -or
            $package['id'] -cnotmatch '^[a-z0-9-]+$' -or $packages.Contains($package['id']) -or
            $package['name'] -isnot [string] -or $package['version'] -isnot [string] -or
            $package['license_declared'] -isnot [string] -or
            $package['license_concluded'] -isnot [string] -or
            $package['download_location'] -isnot [string] -or
            $package['third_party'] -isnot [bool] -or
            $package['depends_on'] -isnot [Collections.IList]) {
            throw 'Invalid or duplicate compliance package.'
        }
        if ($package.Contains('archive_sha256') -and
            ($package['archive_sha256'] -isnot [string] -or
             $package['archive_sha256'] -cnotmatch '^[A-F0-9]{64}$')) {
            throw 'Invalid compliance package source archive hash.'
        }
        $packages[$package['id']] = $package
    }
    foreach ($package in $packages.Values) {
        foreach ($dependency in $package['depends_on']) {
            if (-not $packages.Contains($dependency) -or $dependency -ceq $package['id']) {
                throw "Invalid compliance dependency: $($package['id']) -> $dependency"
            }
        }
    }
    if ($packages.Contains('librime-octagram') -or
        -not $packages.Contains('librime-runtime') -or
        $packages['librime-runtime']['license_concluded'] -cne 'NOASSERTION') {
        throw 'Compliance policy closed or expanded the pending librime review without authorization.'
    }

    $rules = @($policy['file_rules'])
    foreach ($rule in $rules) {
        if ($rule -isnot [Collections.IDictionary] -or $rule.Count -ne 2 -or
            -not $rule.Contains('component') -or -not $rule.Contains('pattern') -or
            -not $packages.Contains($rule['component'])) {
            throw 'Invalid compliance file rule.'
        }
        try { $null = [regex]::new($rule['pattern'], [Text.RegularExpressions.RegexOptions]::CultureInvariant) }
        catch { throw 'Invalid compliance file-rule pattern.' }
    }

    $payload = Join-Path $stage 'payload/Mo'
    $names = @(Get-MoStageFiles $payload)
    if ($names.Count -ne $policy['expected_payload_files']) {
        throw 'Compliance policy payload file count mismatch.'
    }
    $files = [Collections.Generic.List[object]]::new()
    $componentCounts = [ordered]@{}
    foreach ($id in $packages.Keys) { $componentCounts[$id] = 0 }
    foreach ($name in $names) {
        $matches = @($rules | Where-Object { $name -cmatch $_['pattern'] })
        if ($matches.Count -ne 1) {
            throw "Payload file must map to exactly one compliance component: $name"
        }
        $component = $matches[0]['component']
        $componentCounts[$component]++
        $item = Get-Item -LiteralPath (Join-Path $payload $name)
        $files.Add([ordered]@{
            path = $name
            size = $item.Length
            sha256 = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash
            sha1 = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA1).Hash.ToLowerInvariant()
            component = $component
        })
    }
    foreach ($rule in $rules) {
        if ($componentCounts[$rule['component']] -eq 0) {
            throw "Compliance file rule matched no payload: $($rule['component'])"
        }
    }
    return [ordered]@{
        stage = $stage
        policy = $policy
        packages = $packages
        files = $files.ToArray()
        component_counts = $componentCounts
    }
}

function Get-MoSpdxVerificationCode([object[]]$Files) {
    $hashes = @($Files | ForEach-Object { $_['sha1'] } | Sort-Object -CaseSensitive)
    if ($hashes.Count -eq 0 -or @($hashes | Where-Object { $_ -cnotmatch '^[a-f0-9]{40}$' }).Count) {
        throw 'Cannot calculate SPDX package verification code without SHA-1 file hashes.'
    }
    $bytes = [Text.Encoding]::ASCII.GetBytes($hashes -join '')
    return [Convert]::ToHexString([Security.Cryptography.SHA1]::HashData($bytes)).ToLowerInvariant()
}

function ConvertTo-MoSpdxDocument([Collections.IDictionary]$Model, [string]$StageManifestHash) {
    $policy = $Model['policy']
    $packages = [Collections.Generic.List[object]]::new()
    $relationships = [Collections.Generic.List[object]]::new()
    $files = [Collections.Generic.List[object]]::new()
    $mainId = 'SPDXRef-Package-Mo-Windows-Payload'
    $packages.Add([ordered]@{
        SPDXID = $mainId
        name = 'Mo Windows payload'
        versionInfo = '0.1.0-phase0'
        downloadLocation = 'NOASSERTION'
        filesAnalyzed = $false
        licenseConcluded = 'NOASSERTION'
        licenseDeclared = 'Apache-2.0 AND GPL-3.0-only AND NOASSERTION'
        copyrightText = 'NOASSERTION'
        comment = 'Development-only inventory; release authorization is false.'
    })
    foreach ($package in $Model['packages'].Values) {
        $id = 'SPDXRef-Package-' + $package['id']
        $ownedFiles = @($Model['files'] | Where-Object { $_['component'] -ceq $package['id'] })
        $analyzed = $ownedFiles.Count -gt 0
        $entry = [ordered]@{
            SPDXID = $id
            name = $package['name']
            versionInfo = $package['version']
            downloadLocation = $package['download_location']
            filesAnalyzed = $analyzed
            licenseConcluded = $package['license_concluded']
            licenseDeclared = $package['license_declared']
            copyrightText = 'NOASSERTION'
        }
        if ($analyzed) {
            $entry['packageVerificationCode'] = [ordered]@{
                packageVerificationCodeValue = Get-MoSpdxVerificationCode $ownedFiles
            }
            $entry['licenseInfoFromFiles'] = @($package['license_declared'])
        }
        if ($package.Contains('archive_sha256')) {
            $entry['sourceInfo'] = 'Pinned source archive SHA256: ' + $package['archive_sha256']
        }
        $packages.Add($entry)
        if ($analyzed) {
            $relationships.Add([ordered]@{
                spdxElementId = $mainId; relationshipType = 'CONTAINS'; relatedSpdxElement = $id
            })
        }
        foreach ($dependency in $package['depends_on']) {
            $relationships.Add([ordered]@{
                spdxElementId = $id
                relationshipType = 'DEPENDS_ON'
                relatedSpdxElement = 'SPDXRef-Package-' + $dependency
            })
        }
    }
    $index = 0
    foreach ($file in $Model['files']) {
        $index++
        $fileId = 'SPDXRef-File-{0:D3}' -f $index
        $package = $Model['packages'][$file['component']]
        $files.Add([ordered]@{
            SPDXID = $fileId
            fileName = './' + $file['path']
            checksums = @(
                [ordered]@{ algorithm = 'SHA1'; checksumValue = $file['sha1'] },
                [ordered]@{ algorithm = 'SHA256'; checksumValue = $file['sha256'] }
            )
            licenseConcluded = $package['license_concluded']
            licenseInfoInFiles = @($package['license_declared'])
            copyrightText = 'NOASSERTION'
            comment = 'component=' + $file['component'] + '; size=' + $file['size']
        })
        $relationships.Add([ordered]@{
            spdxElementId = 'SPDXRef-Package-' + $file['component']
            relationshipType = 'CONTAINS'
            relatedSpdxElement = $fileId
        })
    }
    return [ordered]@{
        spdxVersion = 'SPDX-2.3'
        dataLicense = 'CC0-1.0'
        SPDXID = 'SPDXRef-DOCUMENT'
        name = 'Mo Windows payload phase-0 SBOM draft'
        documentNamespace = 'https://github.com/mo-input/mo/spdx/windows/' + $StageManifestHash.ToLowerInvariant()
        creationInfo = [ordered]@{
            created = $policy['document_created'] + 'T00:00:00Z'
            creators = @('Organization: Mo Project', 'Tool: installer/windows/prepare-release-compliance.ps1')
            comment = 'Deterministic draft. Legal review and release authorization remain false.'
        }
        documentDescribes = @($mainId)
        packages = $packages.ToArray()
        files = $files.ToArray()
        relationships = $relationships.ToArray()
    }
}

function Get-MoThirdPartyNotices([Collections.IDictionary]$Model) {
    $notice = [Collections.Generic.List[string]]::new()
    $notice.Add('Mo Windows payload — THIRD_PARTY_NOTICES draft')
    $notice.Add('')
    $notice.Add('NOT FOR DISTRIBUTION. This inventory is hash-bound but still requires formal legal review, complete license texts and GPL corresponding-source packaging.')
    $notice.Add('')
    foreach ($package in $Model['packages'].Values | Where-Object { $_['third_party'] } | Sort-Object { $_['id'] }) {
        $notice.Add("[$($package['id'])] $($package['name']) $($package['version'])")
        $notice.Add("Declared: $($package['license_declared'])")
        $notice.Add("Concluded: $($package['license_concluded'])")
        $notice.Add("Source: $($package['download_location'])")
        if ($package.Contains('source_required') -and $package['source_required']) {
            $notice.Add('Corresponding source required: yes')
        }
        $notice.Add('')
    }
    return $notice.ToArray()
}
