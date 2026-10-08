# Windows PowerShell 5.1 compatible. Consistency checks, not signature verification.
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'vm-test-policy.ps1')

function Assert-MoChangedUpgradePair([object]$Pair) {
    $names = @(
        'format', 'kind', 'development_only', 'install_executed', 'build_flavor',
        'fault_injection_included', 'base_version', 'upgrade_version',
        'msi_upgrade_code', 'bundle_upgrade_code', 'base_product_code', 'upgrade_product_code',
        'base_bundle_id', 'upgrade_bundle_id', 'base_bundle_sha256', 'upgrade_bundle_sha256',
        'base_stage_manifest_sha256', 'upgrade_stage_manifest_sha256',
        'base_linked_evidence_sha256', 'upgrade_linked_evidence_sha256'
    )
    $actual = @($Pair.PSObject.Properties.Name)
    if ($actual.Count -ne $names.Count -or @($names | Where-Object { $_ -cnotin $actual }).Count -or
        $Pair.format -ne 3 -or $Pair.kind -cne 'mo-linked-changed-payload-upgrade-pair' -or
        $Pair.development_only -isnot [bool] -or $Pair.development_only -ne $true -or
        $Pair.install_executed -isnot [bool] -or $Pair.install_executed -ne $false -or
        $Pair.build_flavor -cne 'DevelopmentTest' -or
        $Pair.fault_injection_included -isnot [bool] -or $Pair.fault_injection_included -ne $true) {
        throw 'Invalid changed-payload upgrade pair.'
    }
    Assert-MoMsiMajorUpgradeVersions $Pair.base_version $Pair.upgrade_version
    foreach ($name in @('msi_upgrade_code', 'bundle_upgrade_code', 'base_product_code',
        'upgrade_product_code', 'base_bundle_id', 'upgrade_bundle_id')) {
        if ($Pair.$name -cnotmatch '^\{[A-F0-9]{8}-[A-F0-9]{4}-[A-F0-9]{4}-[A-F0-9]{4}-[A-F0-9]{12}\}$') {
            throw "Invalid changed-payload GUID: $name"
        }
    }
    if ($Pair.msi_upgrade_code -cne '{8245E1F5-8BC4-4D41-BA9C-65F30CEBCB32}' -or
        $Pair.bundle_upgrade_code -cne '{E08C0321-3F73-4D39-BC28-E7DF02C5134E}') {
        throw 'Changed-payload pair has an unexpected Mo upgrade family.'
    }
    foreach ($name in @('base_bundle_sha256', 'upgrade_bundle_sha256',
        'base_stage_manifest_sha256', 'upgrade_stage_manifest_sha256',
        'base_linked_evidence_sha256', 'upgrade_linked_evidence_sha256')) {
        if ($Pair.$name -cnotmatch '^[A-F0-9]{64}$') { throw "Invalid changed-payload hash: $name" }
    }
    foreach ($suffix in @('product_code', 'bundle_id', 'bundle_sha256', 'stage_manifest_sha256')) {
        if ($Pair.("base_$suffix") -ceq $Pair.("upgrade_$suffix")) {
            throw "Changed-payload identities must differ: $suffix"
        }
    }
}

function Assert-MoVmPlainDosPath([string]$Path, [switch]$MayNotExist) {
    if (-not (Test-MoVmAbsoluteDosPath $Path) -or $Path.Substring(2).Contains(':')) {
        throw 'VM paths must be absolute local DOS paths.'
    }
    $full = [IO.Path]::GetFullPath($Path)
    $current = [IO.Path]::GetPathRoot($full)
    foreach ($part in $full.Substring($current.Length).Split('\', [StringSplitOptions]::RemoveEmptyEntries)) {
        $current = Join-Path $current $part
        if (Test-Path -LiteralPath $current) {
            if ((Get-Item -LiteralPath $current -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw 'VM paths must not traverse reparse points.'
            }
        } elseif (-not $MayNotExist) { throw 'VM path does not exist.' }
    }
    return $full.TrimEnd('\')
}

function Get-MoVmSettingsFingerprint([string]$Path) {
    $full = Assert-MoVmPlainDosPath $Path -MayNotExist
    if (-not (Test-Path -LiteralPath $full)) {
        return [pscustomobject]@{ present = $false; size = $null; sha256 = $null }
    }
    if (-not (Test-Path -LiteralPath $full -PathType Leaf)) { throw 'Settings path must be a file.' }
    return [pscustomobject]@{
        present = $true
        size = (Get-Item -LiteralPath $full -Force).Length
        sha256 = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash
    }
}

function Assert-MoVmSettingsPreserved([object]$Before, [object]$After) {
    if ($Before.present -ne $After.present -or $Before.size -ne $After.size -or
        $Before.sha256 -cne $After.sha256) { throw 'Upgrade changed the user settings file.' }
}

function Assert-MoVmChangedUpgradeKit([string]$KitRoot) {
    $root = Assert-MoVmPlainDosPath $KitRoot
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { throw 'Upgrade kit must be a directory.' }
    $path = Join-Path $root 'vm-changed-upgrade-kit.json'
    $null = Assert-MoVmPlainDosPath $path
    $kit = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    $names = @(
        'format', 'kind', 'development_only', 'redistributable', 'install_execution_authorized',
        'wix_version', 'base_version', 'upgrade_version', 'base_product_code', 'upgrade_product_code',
        'base_bundle_sha256', 'upgrade_bundle_sha256',
        'base_stage_manifest_sha256', 'upgrade_stage_manifest_sha256', 'pair_sha256', 'files'
    )
    $actual = @($kit.PSObject.Properties.Name)
    if ($actual.Count -ne $names.Count -or @($names | Where-Object { $_ -cnotin $actual }).Count -or
        $kit.format -ne 1 -or $kit.kind -cne 'mo-installer-vm-changed-upgrade-kit' -or
        $kit.wix_version -cne '4.0.6+73c89738') { throw 'Invalid changed upgrade kit manifest.' }
    foreach ($name in @('development_only', 'redistributable', 'install_execution_authorized')) {
        $expected = $name -ceq 'development_only'
        if ($kit.$name -isnot [bool] -or $kit.$name -ne $expected) {
            throw 'Changed upgrade kit is not an unauthorized development kit.'
        }
    }
    $expectedFiles = @(
        'mo-setup-base-unsigned.exe', 'mo-setup-upgrade-unsigned.exe',
        'base-stage.json', 'upgrade-stage.json', 'base-linked.json', 'upgrade-linked.json',
        'linked-upgrade-pair-evidence.json', 'mo-tip-registrar.exe', 'vm-test-policy.ps1',
        'changed-upgrade-policy.ps1', 'initialize-disposable-vm.ps1',
        'run-vm-changed-payload-upgrade.ps1'
    )
    $entries = @(Get-ChildItem -LiteralPath $root -Force)
    if (@($entries | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }).Count -or
        @($entries | Where-Object { $_.PSIsContainer }).Count) {
        throw 'Changed upgrade kit must be flat and contain no reparse points.'
    }
    $files = @($kit.files.PSObject.Properties)
    $actualFiles = @($entries | Where-Object { $_.Name -cne 'vm-changed-upgrade-kit.json' })
    if ($files.Count -ne $expectedFiles.Count -or $actualFiles.Count -ne $expectedFiles.Count -or
        @($files | Where-Object { $_.Name -cnotin $expectedFiles }).Count -or
        @($actualFiles | Where-Object { $_.Name -cnotin $expectedFiles }).Count) {
        throw 'Changed upgrade kit inventory name mismatch.'
    }
    foreach ($name in $expectedFiles) {
        $entry = $kit.files.$name
        $file = Join-Path $root $name
        if (@($entry.PSObject.Properties).Count -ne 2 -or
            ($entry.size -isnot [int] -and $entry.size -isnot [long]) -or $entry.size -lt 0 -or
            $entry.sha256 -cnotmatch '^[A-F0-9]{64}$' -or
            (Get-Item -LiteralPath $file -Force).Length -ne $entry.size -or
            (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -cne $entry.sha256) {
            throw "Changed upgrade kit hash/size mismatch: $name"
        }
    }
    $pair = Get-Content (Join-Path $root 'linked-upgrade-pair-evidence.json') -Raw | ConvertFrom-Json
    Assert-MoChangedUpgradePair $pair
    foreach ($name in @('base_version', 'upgrade_version', 'base_product_code', 'upgrade_product_code',
        'base_bundle_sha256', 'upgrade_bundle_sha256', 'base_stage_manifest_sha256', 'upgrade_stage_manifest_sha256')) {
        if ($kit.$name -cne $pair.$name) { throw "Changed upgrade kit/pair mismatch: $name" }
    }
    $bindings = @{
        'mo-setup-base-unsigned.exe' = $pair.base_bundle_sha256
        'mo-setup-upgrade-unsigned.exe' = $pair.upgrade_bundle_sha256
        'base-stage.json' = $pair.base_stage_manifest_sha256
        'upgrade-stage.json' = $pair.upgrade_stage_manifest_sha256
        'base-linked.json' = $pair.base_linked_evidence_sha256
        'upgrade-linked.json' = $pair.upgrade_linked_evidence_sha256
        'linked-upgrade-pair-evidence.json' = $kit.pair_sha256
    }
    foreach ($name in $bindings.Keys) {
        if ((Get-FileHash (Join-Path $root $name)).Hash -cne $bindings[$name]) {
            throw "Changed upgrade kit primary binding mismatch: $name"
        }
    }
    foreach ($side in @('base', 'upgrade')) {
        $linked = Get-Content (Join-Path $root "$side-linked.json") -Raw | ConvertFrom-Json
        if (@($linked.PSObject.Properties).Count -ne 16 -or $linked.format -ne 3 -or
            $linked.development_only -ne $true -or $linked.install_executed -ne $false -or
            $linked.build_flavor -cne 'DevelopmentTest' -or $linked.fault_injection_included -ne $true -or
            $linked.wix_version -cne $kit.wix_version -or
            $linked.msi_upgrade_code -cne $pair.msi_upgrade_code -or
            $linked.bundle_upgrade_code -cne $pair.bundle_upgrade_code -or
            $linked.product_version -cne $pair.($side + '_version') -or
            $linked.msi_product_code -cne $pair.($side + '_product_code') -or
            $linked.bundle_id -cne $pair.($side + '_bundle_id') -or
            $linked.bundle_sha256 -cne $pair.($side + '_bundle_sha256') -or
            $linked.stage_manifest_sha256 -cne $pair.($side + '_stage_manifest_sha256')) {
            throw "Changed upgrade linked receipt mismatch: $side"
        }
        $contract = Get-MoVmPayloadContract (Join-Path $root "$side-stage.json")
        if ($side -ceq 'upgrade') {
            $registrar = Get-Item (Join-Path $root 'mo-tip-registrar.exe')
            $expected = $contract['bin/mo-tip-registrar.exe']
            if ($null -eq $expected -or $registrar.Length -ne $expected.size -or
                (Get-FileHash $registrar.FullName).Hash -cne $expected.sha256) {
                throw 'Changed upgrade registrar does not match the upgrade payload.'
            }
        }
    }
    return $kit
}
