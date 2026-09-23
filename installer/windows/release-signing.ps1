# Authenticode boundary helpers. These only describe and verify the required
# sequence; they never access a certificate, timestamp service or sign a file.
Set-StrictMode -Version Latest

function Get-MoUnsignedSignatureRecord([string]$Path, [string]$Role, [int]$Order) {
    $resolved = Assert-MoPlainPath $Path
    if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) { throw "Signing input is missing: $Role" }
    $signature = Get-AuthenticodeSignature -LiteralPath $resolved
    if ($signature.Status -ne [Management.Automation.SignatureStatus]::NotSigned) {
        throw "Development signing input must be unsigned: $Role ($($signature.Status))"
    }
    return [ordered]@{
        order = $Order; role = $Role; path = $resolved
        size = (Get-Item $resolved).Length
        sha256 = (Get-FileHash $resolved -Algorithm SHA256).Hash
        observed_authenticode_status = 'NotSigned'
    }
}

function Get-MoReleaseSigningModel(
    [string]$StageDirectory,
    [string]$LinkedEvidencePath,
    [string]$MsiPath,
    [string]$BundlePath
) {
    $stage = Assert-MoPlainPath $StageDirectory
    $null = Assert-MoPreparedStage $stage
    $evidencePath = Assert-MoPlainPath $LinkedEvidencePath
    $evidence = Read-MoStageJson $evidencePath
    $required = @('format', 'development_only', 'install_executed', 'build_flavor',
        'fault_injection_included', 'wix_version', 'product_version', 'msi_product_code',
        'msi_upgrade_code', 'bundle_id', 'bundle_upgrade_code', 'known_link_warning',
        'msi_ice_validated', 'stage_manifest_sha256', 'msi_sha256', 'bundle_sha256')
    if ($evidence.Count -ne $required.Count -or
        @($required | Where-Object { -not $evidence.Contains($_) }).Count -or
        $evidence['format'] -ne 3 -or $evidence['development_only'] -ne $true -or
        $evidence['install_executed'] -ne $false -or $evidence['build_flavor'] -cne 'ProductionShape' -or
        $evidence['fault_injection_included'] -ne $false) {
        throw 'Signing plan requires exact ProductionShape linked evidence.'
    }
    $manifestHash = (Get-FileHash (Join-Path $stage 'mo-stage.json') -Algorithm SHA256).Hash
    if ($evidence['stage_manifest_sha256'] -cne $manifestHash) {
        throw 'Signing linked evidence is detached from the verified stage.'
    }
    $msi = Assert-MoPlainPath $MsiPath
    $bundle = Assert-MoPlainPath $BundlePath
    if ((Get-FileHash $msi -Algorithm SHA256).Hash -cne $evidence['msi_sha256'] -or
        (Get-FileHash $bundle -Algorithm SHA256).Hash -cne $evidence['bundle_sha256']) {
        throw 'Signing installer hash does not match linked evidence.'
    }
    $payload = Join-Path $stage 'payload/Mo'
    $inner = [Collections.Generic.List[object]]::new()
    $order = 0
    foreach ($item in @(
        @('broker-x64', 'bin/mo-broker.exe'),
        @('settings-x64', 'bin/mo-settings.exe'),
        @('registrar-and-finalizer-x64', 'bin/mo-tip-registrar.exe'),
        @('tip-x64', 'tip/x64/mo-tip.dll'),
        @('tip-x86', 'tip/x86/mo-tip.dll'),
        @('librime-runtime-x64', 'runtime/librime/rime.dll')
    )) {
        $order++
        $inner.Add((Get-MoUnsignedSignatureRecord (Join-Path $payload $item[1]) $item[0] $order))
    }
    $outer = @(
        (Get-MoUnsignedSignatureRecord $msi 'windows-installer-msi-baseline' 4),
        (Get-MoUnsignedSignatureRecord $bundle 'burn-bundle-baseline' 6)
    )
    return [ordered]@{
        stage = $stage; evidence_path = $evidencePath; linked_evidence = $evidence
        stage_manifest_sha256 = $manifestHash; inner = $inner.ToArray(); outer = $outer
    }
}

function ConvertTo-MoReleaseSigningPlan([Collections.IDictionary]$Model) {
    return [ordered]@{
        format = 1; kind = 'mo-authenticode-release-signing-plan'
        development_only = $true; signing_executed = $false; release_authorized = $false
        requires_rebuild_after_inner_signing = $true
        stage_manifest_sha256 = $Model['stage_manifest_sha256']
        linked_evidence_sha256 = (Get-FileHash $Model['evidence_path'] -Algorithm SHA256).Hash
        unsigned_inner_payloads = @($Model['inner'] | ForEach-Object {
            [ordered]@{ order = $_['order']; role = $_['role']; size = $_['size']; sha256 = $_['sha256']; observed_authenticode_status = $_['observed_authenticode_status'] }
        })
        unsigned_linked_baselines = @($Model['outer'] | ForEach-Object {
            [ordered]@{ order = $_['order']; role = $_['role']; size = $_['size']; sha256 = $_['sha256']; observed_authenticode_status = $_['observed_authenticode_status'] }
        })
        required_sequence = @(
            [ordered]@{ order = 1; action = 'sign-inner-pe'; targets = @('broker-x64', 'settings-x64', 'registrar-and-finalizer-x64', 'tip-x64', 'tip-x86', 'librime-runtime-x64') },
            [ordered]@{ order = 2; action = 'timestamp-and-verify-inner-pe'; requirement = 'every target must have a valid trusted Authenticode signature and RFC3161 timestamp' },
            [ordered]@{ order = 3; action = 'reseal-stage-and-rebuild-msi'; requirement = 'never mutate the existing sealed stage; create and independently verify a new signed stage' },
            [ordered]@{ order = 4; action = 'sign-timestamp-and-verify-msi'; requirement = 'MSI must embed only signed inner payloads' },
            [ordered]@{ order = 5; action = 'rebuild-bundle'; requirement = 'Bundle must embed the signed MSI and signed registrar/finalizer' },
            [ordered]@{ order = 6; action = 'sign-timestamp-and-verify-bundle'; requirement = 'Bundle is the final signed artifact' }
        )
        prohibited_shortcuts = @(
            'signing an MSI or Bundle that embeds unsigned inner payloads',
            'signing files in place inside an already sealed stage',
            'reusing this unsigned linked baseline as a release artifact',
            'treating a successful signature operation as release authorization'
        )
    }
}
