Set-StrictMode -Version Latest

function Test-MoVmAbsoluteDosPath([string]$Path) {
    return $Path -match '^[A-Za-z]:\\' -and $Path.IndexOf([char]0) -lt 0
}

function ConvertFrom-MoVmRegistrarStatus([string[]]$Lines) {
    $state = @{}
    foreach ($line in $Lines) {
        if ($line -notmatch '^([^=]+)=(.*)$' -or $state.ContainsKey($Matches[1])) {
            throw "Invalid or duplicate registrar status line: $line"
        }
        $state[$Matches[1]] = $Matches[2]
    }
    $names = @(
        'com.x64', 'com.x86',
        'profile.registered', 'profile.enabled', 'profile.active',
        'user.finalizer', 'user.finalizer.transaction'
    )
    if ($state.Count -ne $names.Count) { throw 'Registrar status field count mismatch.' }
    foreach ($name in $names) {
        if (-not $state.ContainsKey($name)) { throw "Registrar status is missing $name." }
    }
    foreach ($name in @('profile.registered', 'profile.enabled', 'profile.active')) {
        if ($state[$name] -notin @('true', 'false')) { throw "Invalid registrar boolean: $name" }
    }
    if ($state['user.finalizer'] -notin @('missing', 'v1', 'invalid')) {
        throw 'Invalid current-user finalizer marker state.'
    }
    $transactions = @(
        'missing', 'invalid',
        'mo-user-finalizer-install-v1-disabled',
        'mo-user-finalizer-install-v1-enabled',
        'mo-user-finalizer-repair-v1-disabled',
        'mo-user-finalizer-repair-v1-enabled',
        'mo-user-finalizer-remove-v1-disabled',
        'mo-user-finalizer-remove-v1-enabled'
    )
    if ($state['user.finalizer.transaction'] -notin $transactions) {
        throw 'Invalid current-user finalizer transaction state.'
    }
    return $state
}

function Assert-MoVmLifecycleState(
    [Collections.IDictionary]$State,
    [ValidateSet('Clean', 'Installed', 'Repaired', 'Uninstalled')]
    [string]$Phase
) {
    $expected = switch ($Phase) {
        'Clean' { [ordered]@{
            'com.x64' = 'missing'; 'com.x86' = 'missing'
            'profile.registered' = 'false'; 'profile.enabled' = 'false'; 'profile.active' = 'false'
            'user.finalizer' = 'missing'; 'user.finalizer.transaction' = 'missing'
        } }
        'Installed' { [ordered]@{
            'com.x64' = 'missing'; 'com.x86' = 'missing'
            'profile.registered' = 'true'; 'profile.enabled' = 'true'; 'profile.active' = 'false'
            'user.finalizer' = 'v1'
            'user.finalizer.transaction' = 'mo-user-finalizer-install-v1-disabled'
        } }
        'Repaired' { [ordered]@{
            'com.x64' = 'missing'; 'com.x86' = 'missing'
            'profile.registered' = 'true'; 'profile.enabled' = 'true'; 'profile.active' = 'false'
            'user.finalizer' = 'v1'
            'user.finalizer.transaction' = 'mo-user-finalizer-repair-v1-enabled'
        } }
        'Uninstalled' { [ordered]@{
            'com.x64' = 'missing'; 'com.x86' = 'missing'
            'profile.registered' = 'false'; 'profile.enabled' = 'false'; 'profile.active' = 'false'
            'user.finalizer' = 'missing'
            # Burn has no ExePackage commit callback. The remove receipt remains
            # so rollback can restore the enabled bit if the later MSI removal fails.
            'user.finalizer.transaction' = 'mo-user-finalizer-remove-v1-enabled'
        } }
    }
    if ($State.Count -ne $expected.Count) { throw "$Phase registrar state field count mismatch." }
    foreach ($name in $expected.Keys) {
        if (-not $State.Contains($name) -or $State[$name] -cne $expected[$name]) {
            $actual = if ($State.Contains($name)) { $State[$name] } else { '<missing>' }
            throw "$Phase state mismatch for ${name}: expected '$($expected[$name])', found '$actual'."
        }
    }
}

function Get-MoVmPayloadContract(
    [string]$ManifestPath,
    [int]$ExpectedFileCount = 131
) {
    if (-not (Test-MoVmAbsoluteDosPath $ManifestPath) -or
        -not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
        throw 'Stage manifest must be an existing absolute file.'
    }
    $manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
    if ($manifest.format -ne 1 -or $manifest.kind -cne 'mo-windows-development-stage' -or
        $manifest.development_only -ne $true -or $manifest.redistributable -ne $false -or
        $manifest.installable -ne $false -or $manifest.payload_root -cne 'payload/Mo') {
        throw 'VM test requires the exact non-installable development stage manifest contract.'
    }
    $contract = [Collections.Specialized.OrderedDictionary]::new(
        [StringComparer]::OrdinalIgnoreCase)
    foreach ($property in $manifest.files.PSObject.Properties) {
        if (-not $property.Name.StartsWith('payload/Mo/', [StringComparison]::Ordinal)) { continue }
        $relative = $property.Name.Substring('payload/Mo/'.Length)
        $segments = $relative.Split('/')
        if ($relative -notmatch '^[A-Za-z0-9._/-]+$' -or $segments.Count -eq 0 -or
            @($segments | Where-Object { -not $_ -or $_ -in @('.', '..') -or $_.EndsWith('.') }).Count -ne 0) {
            throw "Unsafe payload contract path: $relative"
        }
        $entry = $property.Value
        if (($entry.size -isnot [int] -and $entry.size -isnot [long]) -or
            $entry.size -lt 0 -or $entry.sha256 -notmatch '^[A-F0-9]{64}$') {
            throw "Invalid payload contract entry: $relative"
        }
        if ($contract.Contains($relative)) { throw "Duplicate payload path: $relative" }
        $contract.Add($relative, [pscustomobject]@{
            size = [long]$entry.size
            sha256 = [string]$entry.sha256
        })
    }
    if ($contract.Count -ne $ExpectedFileCount) {
        throw "Payload contract count mismatch: expected $ExpectedFileCount, found $($contract.Count)."
    }
    return $contract
}

function Assert-MoVmInstalledPayload(
    [string]$InstallRoot,
    [Collections.IDictionary]$Contract
) {
    if (-not (Test-MoVmAbsoluteDosPath $InstallRoot) -or
        -not (Test-Path -LiteralPath $InstallRoot -PathType Container)) {
        throw "Installed payload root is missing: $InstallRoot"
    }
    $root = (Resolve-Path -LiteralPath $InstallRoot).Path.TrimEnd('\')
    $items = @(Get-ChildItem -LiteralPath $root -Recurse -Force)
    if (@($items | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }).Count) {
        throw 'Installed payload contains a reparse point.'
    }
    $files = @($items | Where-Object { -not $_.PSIsContainer })
    if ($files.Count -ne $Contract.Count) {
        throw "Installed payload file count mismatch: expected $($Contract.Count), found $($files.Count)."
    }
    $actualFiles = [Collections.Specialized.OrderedDictionary]::new(
        [StringComparer]::OrdinalIgnoreCase)
    foreach ($file in $files) {
        $relative = $file.FullName.Substring($root.Length + 1).Replace('\', '/')
        if ($actualFiles.Contains($relative)) { throw "Duplicate installed payload path: $relative" }
        $actualFiles.Add($relative, $file)
    }
    $expectedDirectories = [Collections.Generic.HashSet[string]]::new(
        [StringComparer]::OrdinalIgnoreCase)
    foreach ($key in $Contract.Keys) {
        $relative = [string]$key
        if (-not $actualFiles.Contains($relative)) {
            throw "Installed payload file is missing: $relative"
        }
        $item = $actualFiles[$relative]
        $actualRelative = $item.FullName.Substring($root.Length + 1).Replace('\', '/')
        if ($actualRelative -cne $relative) { throw "Installed payload path case mismatch: $relative" }
        if ($item.Length -ne $Contract[$key].size -or
            (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash -cne $Contract[$key].sha256) {
            throw "Installed payload hash/size mismatch: $relative"
        }
        $parent = [IO.Path]::GetDirectoryName($relative.Replace('/', '\'))
        while ($parent) {
            [void]$expectedDirectories.Add($parent.Replace('\', '/'))
            $parent = [IO.Path]::GetDirectoryName($parent)
        }
    }
    $actualDirectories = @($items | Where-Object { $_.PSIsContainer } | ForEach-Object {
        $_.FullName.Substring($root.Length + 1).Replace('\', '/')
    })
    if ($actualDirectories.Count -ne $expectedDirectories.Count -or
        @($actualDirectories | Where-Object { -not $expectedDirectories.Contains($_) }).Count) {
        throw 'Installed payload directory set mismatch.'
    }
}

function Get-MoVmMachineComState {
    $path = 'SOFTWARE\Classes\CLSID\{B4911146-2A27-47AA-9D12-109B6AE10A70}\InprocServer32'
    $result = @{}
    foreach ($item in @(
        @('x64', [Microsoft.Win32.RegistryView]::Registry64),
        @('x86', [Microsoft.Win32.RegistryView]::Registry32)
    )) {
        $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey(
            [Microsoft.Win32.RegistryHive]::LocalMachine, $item[1])
        try {
            $key = $base.OpenSubKey($path, $false)
            if ($null -eq $key) {
                $result[$item[0]] = $null
            } else {
                try {
                    $result[$item[0]] = [pscustomobject]@{
                        path = [string]$key.GetValue($null, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
                        threading_model = [string]$key.GetValue('ThreadingModel', $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
                    }
                } finally { $key.Dispose() }
            }
        } finally { $base.Dispose() }
    }
    return $result
}

function Assert-MoVmMachineComState(
    [ValidateSet('Absent', 'Installed')][string]$Phase,
    [string]$InstallRoot
) {
    $state = Get-MoVmMachineComState
    if ($Phase -eq 'Absent') {
        if ($null -ne $state.x64 -or $null -ne $state.x86) {
            throw 'Mo machine COM registration was expected to be absent in both views.'
        }
        return
    }
    $expected = @{
        x64 = Join-Path $InstallRoot 'tip\x64\mo-tip.dll'
        x86 = Join-Path $InstallRoot 'tip\x86\mo-tip.dll'
    }
    foreach ($architecture in @('x64', 'x86')) {
        $actual = $state[$architecture]
        if ($null -eq $actual -or $actual.path -cne $expected[$architecture] -or
            $actual.threading_model -cne 'Apartment') {
            throw "Mo machine COM $architecture registration mismatch."
        }
    }
}

function Get-MoVmMachineIdentity {
    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey(
        [Microsoft.Win32.RegistryHive]::LocalMachine,
        [Microsoft.Win32.RegistryView]::Registry64)
    try {
        $key = $base.OpenSubKey('SOFTWARE\Microsoft\Cryptography', $false)
        if ($null -eq $key) { throw 'MachineGuid registry key is missing.' }
        try { $machineGuid = [string]$key.GetValue('MachineGuid') }
        finally { $key.Dispose() }
        $bios = $base.OpenSubKey('HARDWARE\DESCRIPTION\System\BIOS', $false)
        if ($null -eq $bios) { throw 'System BIOS identity registry key is missing.' }
        try {
            $manufacturer = [string]$bios.GetValue('SystemManufacturer')
            $model = [string]$bios.GetValue('SystemProductName')
        } finally { $bios.Dispose() }
    } finally { $base.Dispose() }
    if ($null -eq ('MoVmVolumeIdentity' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class MoVmVolumeIdentity {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool GetVolumeInformation(
        string rootPathName,
        StringBuilder volumeNameBuffer,
        int volumeNameSize,
        out uint volumeSerialNumber,
        out uint maximumComponentLength,
        out uint fileSystemFlags,
        StringBuilder fileSystemNameBuffer,
        int fileSystemNameSize);
}
'@
    }
    $volumeName = [Text.StringBuilder]::new(261)
    $fileSystemName = [Text.StringBuilder]::new(261)
    [uint32]$serial = 0
    [uint32]$maximumComponentLength = 0
    [uint32]$fileSystemFlags = 0
    $root = $env:SystemDrive.TrimEnd('\') + '\'
    if (-not [MoVmVolumeIdentity]::GetVolumeInformation(
        $root, $volumeName, $volumeName.Capacity, [ref]$serial,
        [ref]$maximumComponentLength, [ref]$fileSystemFlags,
        $fileSystemName, $fileSystemName.Capacity)) {
        throw "Unable to read system volume identity: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
    }
    if (-not $machineGuid -or -not $manufacturer -or -not $model) {
        throw 'Unable to read disposable VM machine identity.'
    }
    return [ordered]@{
        computer_name = [Environment]::MachineName
        user_sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
        machine_guid = $machineGuid
        system_drive = $env:SystemDrive
        system_drive_serial = $serial.ToString('X8')
        manufacturer = $manufacturer
        model = $model
    }
}

function Assert-MoDisposableVmSentinel([string]$SentinelPath) {
    # Path.IsPathFullyQualified is unavailable on Windows PowerShell 5.1's
    # .NET Framework runtime; the harness intentionally accepts DOS paths only.
    if (-not (Test-MoVmAbsoluteDosPath $SentinelPath) -or
        -not (Test-Path -LiteralPath $SentinelPath -PathType Leaf)) {
        throw 'Disposable VM sentinel is missing.'
    }
    $sentinel = Get-Content -LiteralPath $SentinelPath -Raw | ConvertFrom-Json
    $identity = Get-MoVmMachineIdentity
    if ($sentinel.format -ne 1 -or $sentinel.purpose -cne 'mo-destructive-installer-test-vm' -or
        $sentinel.vm_id -notmatch '^[0-9a-f]{32}$' -or
        $sentinel.computer_name -cne $identity.computer_name -or
        $sentinel.user_sid -cne $identity.user_sid -or
        $sentinel.machine_guid -cne $identity.machine_guid -or
        $sentinel.system_drive -cne $identity.system_drive -or
        $sentinel.system_drive_serial -cne $identity.system_drive_serial -or
        $sentinel.manufacturer -cne $identity.manufacturer -or
        $sentinel.model -cne $identity.model -or
        ("$($identity.manufacturer) $($identity.model)") -notmatch
            '(?i)virtual|vmware|virtualbox|hyper-v|kvm|sandbox') {
        throw 'Disposable VM sentinel does not match this machine.'
    }
    return $sentinel
}

function Assert-MoVmTestKit([string]$KitRoot) {
    if (-not (Test-MoVmAbsoluteDosPath $KitRoot) -or
        -not (Test-Path -LiteralPath $KitRoot -PathType Container)) {
        throw 'VM test kit root must be an existing absolute DOS directory.'
    }
    $root = (Resolve-Path -LiteralPath $KitRoot).Path
    $manifestPath = Join-Path $root 'vm-test-kit.json'
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw 'VM test kit manifest is missing.'
    }
    $kit = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if (@($kit.PSObject.Properties).Count -ne 9 -or $kit.format -ne 1 -or
        $kit.kind -cne 'mo-installer-vm-test-kit' -or $kit.development_only -ne $true -or
        $kit.redistributable -ne $false -or $kit.install_execution_authorized -ne $false -or
        $kit.wix_version -cne '4.0.6+73c89738' -or
        $kit.bundle_sha256 -notmatch '^[A-F0-9]{64}$' -or
        $kit.stage_manifest_sha256 -notmatch '^[A-F0-9]{64}$') {
        throw 'Invalid VM test kit manifest.'
    }
    $expectedFiles = @(
        'initialize-disposable-vm.ps1', 'mo-stage.json', 'mo-tip-registrar.exe',
        'mo-setup-development-unsigned.exe', 'run-vm-installer-lifecycle.ps1',
        'vm-test-policy.ps1'
    )
    $properties = @($kit.files.PSObject.Properties)
    if ($properties.Count -ne $expectedFiles.Count -or
        @($properties | Where-Object { $_.Name -cnotin $expectedFiles }).Count) {
        throw 'VM test kit inventory name set mismatch.'
    }
    $actualFiles = @(Get-ChildItem -LiteralPath $root -File | Where-Object {
        $_.Name -cne 'vm-test-kit.json'
    })
    if ($actualFiles.Count -ne $expectedFiles.Count -or
        @($actualFiles | Where-Object { $_.Name -cnotin $expectedFiles }).Count) {
        throw 'VM test kit contains an unexpected or missing file.'
    }
    if (@(Get-ChildItem -LiteralPath $root -Directory -Force).Count) {
        throw 'VM test kit must not contain directories.'
    }
    foreach ($name in $expectedFiles) {
        $property = $properties | Where-Object { $_.Name -ceq $name }
        if ($null -eq $property -or
            ($property.Value.size -isnot [int] -and $property.Value.size -isnot [long]) -or
            $property.Value.size -lt 0 -or $property.Value.sha256 -notmatch '^[A-F0-9]{64}$') {
            throw "Invalid VM test kit inventory entry: $name"
        }
        $path = Join-Path $root $name
        if ((Get-Item -LiteralPath $path).Length -ne $property.Value.size -or
            (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -cne $property.Value.sha256) {
            throw "VM test kit inventory mismatch: $name"
        }
    }
    $bundle = Join-Path $root 'mo-setup-development-unsigned.exe'
    $stageManifest = Join-Path $root 'mo-stage.json'
    if ((Get-FileHash -LiteralPath $bundle -Algorithm SHA256).Hash -cne $kit.bundle_sha256 -or
        (Get-FileHash -LiteralPath $stageManifest -Algorithm SHA256).Hash -cne
            $kit.stage_manifest_sha256) {
        throw 'VM test kit primary hash mismatch.'
    }
    return $kit
}

function Assert-MoVmMatrixTestKit([string]$KitRoot) {
    if (-not (Test-MoVmAbsoluteDosPath $KitRoot) -or
        -not (Test-Path -LiteralPath $KitRoot -PathType Container)) {
        throw 'VM matrix test kit root must be an existing absolute DOS directory.'
    }
    $root = (Resolve-Path -LiteralPath $KitRoot).Path
    $manifestPath = Join-Path $root 'vm-matrix-test-kit.json'
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw 'VM matrix test kit manifest is missing.'
    }
    $kit = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if (@($kit.PSObject.Properties).Count -ne 16 -or $kit.format -ne 1 -or
        $kit.kind -cne 'mo-installer-vm-matrix-test-kit' -or
        $kit.development_only -ne $true -or $kit.redistributable -ne $false -or
        $kit.install_execution_authorized -ne $false -or
        $kit.wix_version -cne '4.0.6+73c89738' -or
        [version]$kit.base_version -ge [version]$kit.upgrade_version) {
        throw 'Invalid VM matrix test kit manifest.'
    }
    foreach ($name in @('base_bundle_sha256', 'upgrade_bundle_sha256', 'stage_manifest_sha256')) {
        if ($kit.$name -notmatch '^[A-F0-9]{64}$') { throw "Invalid VM matrix hash: $name" }
    }
    foreach ($name in @('msi_upgrade_code', 'bundle_upgrade_code', 'base_product_code', 'upgrade_product_code')) {
        if ($kit.$name -notmatch '^\{[A-F0-9]{8}-[A-F0-9]{4}-[A-F0-9]{4}-[A-F0-9]{4}-[A-F0-9]{12}\}$') {
            throw "Invalid VM matrix GUID: $name"
        }
    }
    if ($kit.base_product_code -ceq $kit.upgrade_product_code) {
        throw 'VM matrix product codes must differ.'
    }
    $expectedFiles = @(
        'initialize-disposable-vm.ps1', 'mo-stage.json', 'mo-tip-registrar.exe',
        'mo-setup-base-unsigned.exe', 'mo-setup-upgrade-unsigned.exe',
        'run-vm-installer-matrix.ps1', 'vm-test-policy.ps1'
    )
    $properties = @($kit.files.PSObject.Properties)
    $actualFiles = @(Get-ChildItem -LiteralPath $root -File | Where-Object {
        $_.Name -cne 'vm-matrix-test-kit.json'
    })
    if ($properties.Count -ne $expectedFiles.Count -or
        @($properties | Where-Object { $_.Name -cnotin $expectedFiles }).Count -or
        $actualFiles.Count -ne $expectedFiles.Count -or
        @($actualFiles | Where-Object { $_.Name -cnotin $expectedFiles }).Count) {
        throw 'VM matrix test kit inventory name set mismatch.'
    }
    if (@(Get-ChildItem -LiteralPath $root -Directory -Force).Count) {
        throw 'VM matrix test kit must not contain directories.'
    }
    foreach ($name in $expectedFiles) {
        $property = $properties | Where-Object { $_.Name -ceq $name }
        $path = Join-Path $root $name
        if ($null -eq $property -or
            ($property.Value.size -isnot [int] -and $property.Value.size -isnot [long]) -or
            $property.Value.size -lt 0 -or $property.Value.sha256 -notmatch '^[A-F0-9]{64}$' -or
            (Get-Item -LiteralPath $path).Length -ne $property.Value.size -or
            (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -cne $property.Value.sha256) {
            throw "VM matrix test kit inventory mismatch: $name"
        }
    }
    if ((Get-FileHash -LiteralPath (Join-Path $root 'mo-setup-base-unsigned.exe') -Algorithm SHA256).Hash -cne
            $kit.base_bundle_sha256 -or
        (Get-FileHash -LiteralPath (Join-Path $root 'mo-setup-upgrade-unsigned.exe') -Algorithm SHA256).Hash -cne
            $kit.upgrade_bundle_sha256 -or
        (Get-FileHash -LiteralPath (Join-Path $root 'mo-stage.json') -Algorithm SHA256).Hash -cne
            $kit.stage_manifest_sha256) {
        throw 'VM matrix test kit primary hash mismatch.'
    }
    return $kit
}
