#Requires -Version 7.4
[CmdletBinding()]
param([string]$StageDirectory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'test-fixture.ps1')
. (Join-Path $PSScriptRoot 'wix-payload.ps1')

$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $repo ('build/mo-package-authoring-' + [Guid]::NewGuid().ToString('N'))
$null = Assert-MoNewBuildOutput $fixture $repo
New-Item -ItemType Directory -Path $fixture | Out-Null
$script:packageTests = 0

function Pass([string]$Label, [scriptblock]$Action) {
    & $Action
    $script:packageTests++
    Write-Host "PASS $Label"
}

function Reject([string]$Label, [scriptblock]$Action, [string]$ErrorPattern) {
    $rejected = $false
    try { & $Action | Out-Null } catch {
        if ($_.Exception.Message -notmatch $ErrorPattern) {
            throw "Unexpected failure for ${Label}: $($_.Exception.Message)"
        }
        $rejected = $true
    }
    if (-not $rejected) { throw "Expected rejection: $Label" }
    $script:packageTests++
    Write-Host "PASS $Label rejected"
}

try {
    $package = Read-MoWixDocument (Join-Path $PSScriptRoot 'Package.wxs')
    $namespace = [Xml.XmlNamespaceManager]::new($package.NameTable)
    $namespace.AddNamespace('w', 'http://wixtoolset.org/schemas/v4/wxs')
    $packageNode = $package.SelectSingleNode('/w:Wix/w:Package', $namespace)
    Pass 'per-machine package boundary' {
        if ($null -eq $packageNode -or $packageNode.GetAttribute('Scope') -cne 'perMachine') {
            throw 'Package is not per-machine.'
        }
        $group = $package.SelectSingleNode('//w:ComponentGroupRef[@Id="MoPayloadComponents"]', $namespace)
        if ($null -eq $group) { throw 'Package does not consume the generated payload group.' }
        $launch = $package.SelectSingleNode('//w:Launch', $namespace)
        if ($null -eq $launch -or $launch.GetAttribute('Condition') -cne 'NOT RollbackDisabled') {
            throw 'Package does not reject disabled MSI rollback.'
        }
        $binary = $package.SelectSingleNode('//w:Binary[@Id="MoRegistrarCustomAction"]', $namespace)
        if ($null -eq $binary -or
            $binary.GetAttribute('SourceFile') -cne '$(var.StagePayload)\bin\mo-tip-registrar.exe') {
            throw 'Package custom-action binary is not pinned to the verified stage.'
        }
    }

    $expectedActions = [ordered]@{
        RollbackInstallMachineProfile = @('rollback-install-machine-profile-fixed', 'rollback', 'ignore')
        InstallNewMachineProfile = @('install-new-machine-profile-fixed', 'deferred', 'check')
        InstallMachineProfile = @('install-machine-profile-fixed', 'deferred', 'check')
        CommitInstallMachineProfile = @('commit-install-machine-profile-fixed', 'commit', 'check')
        RollbackRemoveMachineProfile = @('rollback-remove-machine-profile-fixed', 'rollback', 'ignore')
        RemoveMachineProfile = @('remove-machine-profile-fixed', 'deferred', 'check')
        CommitRemoveMachineProfile = @('commit-remove-machine-profile-fixed', 'commit', 'check')
    }
    Pass 'seven elevated embedded transaction actions' {
        $actions = @($package.SelectNodes('//w:CustomAction', $namespace))
        if ($actions.Count -ne $expectedActions.Count) { throw 'Unexpected package custom-action count.' }
        foreach ($action in $actions) {
            $id = $action.GetAttribute('Id')
            $expected = $expectedActions[$id]
            if ($null -eq $expected -or $action.GetAttribute('BinaryRef') -cne 'MoRegistrarCustomAction' -or
                $action.GetAttribute('ExeCommand') -cne $expected[0] -or
                $action.GetAttribute('Execute') -cne $expected[1] -or
                $action.GetAttribute('Return') -cne $expected[2] -or
                $action.GetAttribute('Impersonate') -cne 'no') {
                throw "Invalid machine transaction action: $id"
            }
        }
    }

    Pass 'rollback ordering and mutually exclusive lifecycle conditions' {
        $sequence = @($package.SelectNodes('//w:InstallExecuteSequence/w:Custom', $namespace))
        if ($sequence.Count -ne 7) { throw 'Unexpected custom-action sequence count.' }
        $byAction = @{}
        foreach ($item in $sequence) { $byAction[$item.GetAttribute('Action')] = $item }
        $contracts = @(
            @('RollbackRemoveMachineProfile', 'Before', 'RemoveMachineProfile', 'REMOVE~="ALL" AND NOT UPGRADINGPRODUCTCODE'),
            @('RemoveMachineProfile', 'Before', 'CommitRemoveMachineProfile', 'REMOVE~="ALL" AND NOT UPGRADINGPRODUCTCODE'),
            @('CommitRemoveMachineProfile', 'Before', 'RemoveFiles', 'REMOVE~="ALL" AND NOT UPGRADINGPRODUCTCODE'),
            @('RollbackInstallMachineProfile', 'After', 'InstallFiles', 'NOT REMOVE~="ALL"'),
            @('InstallNewMachineProfile', 'After', 'RollbackInstallMachineProfile', 'NOT Installed AND NOT WIX_UPGRADE_DETECTED AND NOT REMOVE~="ALL"'),
            @('InstallMachineProfile', 'After', 'InstallNewMachineProfile', '(Installed OR WIX_UPGRADE_DETECTED) AND NOT REMOVE~="ALL"'),
            @('CommitInstallMachineProfile', 'After', 'InstallMachineProfile', 'NOT REMOVE~="ALL"')
        )
        foreach ($contract in $contracts) {
            $item = $byAction[$contract[0]]
            if ($null -eq $item -or $item.GetAttribute($contract[1]) -cne $contract[2] -or
                $item.GetAttribute('Condition') -cne $contract[3]) {
                throw "Invalid sequence contract: $($contract[0])"
            }
        }
    }

    Pass 'no elevated current-user/default mutation' {
        $commands = @($package.SelectNodes('//w:CustomAction', $namespace) |
            ForEach-Object { $_.GetAttribute('ExeCommand') }) -join "`n"
        if ($commands -match '(?i)current-user|enable|default|register-com-user') {
            throw 'Package elevated action crosses into user/default state.'
        }
    }

    Pass 'deterministic identifier contract' {
        $sample = 'data/rime-ice/default.yaml'
        if ((Get-MoWixFileId $sample) -cne (Get-MoWixFileId $sample) -or
            (Get-MoWixComponentId $sample) -cne (Get-MoWixComponentId $sample) -or
            (Get-MoWixComponentGuid $sample) -cnotmatch '^\{[A-F0-9]{8}-[A-F0-9]{4}-5[A-F0-9]{3}-8[A-F0-9]{3}-[A-F0-9]{12}\}$' -or
            (Get-MoWixFileId $sample) -ceq (Get-MoWixFileId 'data/rime-ice/rime_ice.schema.yaml') -or
            (Get-MoWixFileId 'tip/x64/mo-tip.dll') -cne 'TipX64File' -or
            (Get-MoWixFileId 'tip/x86/mo-tip.dll') -cne 'TipX86File') {
            throw 'Deterministic WiX identifier contract failed.'
        }
    }

    $bundle = Read-MoWixDocument (Join-Path $PSScriptRoot 'Bundle.wxs')
    $bundleNamespace = [Xml.XmlNamespaceManager]::new($bundle.NameTable)
    $bundleNamespace.AddNamespace('w', 'http://wixtoolset.org/schemas/v4/wxs')
    $bundleNamespace.AddNamespace('util', 'http://wixtoolset.org/schemas/v4/wxs/util')
    Pass 'bundle machine/user privilege split' {
        $packages = @($bundle.SelectNodes('/w:Wix/w:Bundle/w:Chain/*', $bundleNamespace))
        if ($packages.Count -ne 2 -or $packages[0].LocalName -cne 'MsiPackage' -or
            $packages[0].GetAttribute('Id') -cne 'MoMachinePackage' -or
            $packages[0].GetAttribute('Visible') -cne 'no') {
            throw 'Bundle machine package contract mismatch.'
        }
        $finalizer = $packages[1]
        $expected = [ordered]@{
            Id = 'MoCurrentUserFinalizer'
            After = 'MoMachinePackage'
            SourceFile = '$(var.UserFinalizerExe)'
            DetectCondition = 'MoUserFinalizerMarker = "mo-user-finalizer-v1"'
            InstallArguments = ''
            RepairArguments = ''
            UninstallArguments = ''
            PerMachine = 'no'
            Cache = 'keep'
            Permanent = 'no'
            Vital = 'yes'
        }
        if ($finalizer.LocalName -cne 'ExePackage') { throw 'Bundle user finalizer is not an ExePackage.' }
        foreach ($attribute in $expected.Keys) {
            if ($finalizer.GetAttribute($attribute) -cne $expected[$attribute]) {
                throw "Bundle user finalizer mismatch: $attribute"
            }
        }
        foreach ($attribute in @('InstallArguments', 'RepairArguments', 'UninstallArguments')) {
            if (-not $finalizer.HasAttribute($attribute)) {
                throw "Bundle finalizer must explicitly author empty base arguments: $attribute"
            }
        }
        $commandLines = @($finalizer.SelectNodes('w:CommandLine', $bundleNamespace))
        $expectedCommandLines = [ordered]@{
            'WixBundleAction = 3' = [ordered]@{
                InstallArgument = 'rollback-remove-current-user-fixed'
                UninstallArgument = 'remove-current-user-fixed'
            }
            'WixBundleAction = 5' = [ordered]@{
                InstallArgument = 'install-current-user-fixed'
                UninstallArgument = 'rollback-install-current-user-fixed'
            }
            'WixBundleAction = 7' = [ordered]@{
                InstallArgument = 'repair-current-user-fixed'
                RepairArgument = 'repair-current-user-fixed'
            }
        }
        if ($commandLines.Count -ne $expectedCommandLines.Count) {
            throw 'Bundle finalizer action/rollback command count mismatch.'
        }
        foreach ($line in $commandLines) {
            $condition = $line.GetAttribute('Condition')
            $command = $expectedCommandLines[$condition]
            if ($null -eq $command) { throw "Unexpected finalizer command condition: $condition" }
            foreach ($attribute in @('InstallArgument', 'RepairArgument', 'UninstallArgument')) {
                $expectedValue = if ($command.Contains($attribute)) { $command[$attribute] } else { '' }
                if ($line.GetAttribute($attribute) -cne $expectedValue) {
                    throw "Bundle finalizer command mismatch: $condition / $attribute"
                }
            }
        }
        $provider = $finalizer.SelectSingleNode('w:Provides', $bundleNamespace)
        if ($null -eq $provider -or
            $provider.GetAttribute('Key') -cne 'Mo.CurrentUserFinalizer.v1' -or
            $provider.GetAttribute('Version') -cne '$(var.ProductVersion)' -or
            $provider.GetAttribute('DisplayName') -cne 'Mo current-user finalizer') {
            throw 'Bundle finalizer dependency provider contract mismatch.'
        }
    }

    Pass 'bundle exact current-user detection marker' {
        $search = $bundle.SelectSingleNode(
            '/w:Wix/w:Bundle/util:RegistrySearch[@Id="DetectMoUserFinalizer"]',
            $bundleNamespace)
        if ($null -eq $search -or
            $search.GetAttribute('Variable') -cne 'MoUserFinalizerMarker' -or
            $search.GetAttribute('Root') -cne 'HKCU' -or
            $search.GetAttribute('Key') -cne 'Software\Classes\Local Settings\Software\Mo\InputMethod\Setup' -or
            $search.GetAttribute('Value') -cne 'UserFinalizer' -or
            $search.GetAttribute('Result') -cne 'value' -or
            $search.GetAttribute('Bitness') -cne 'always64') {
            throw 'Bundle user-finalizer detection contract mismatch.'
        }
    }

    Pass 'bundle build binds staged finalizer and required extensions' {
        $buildSource = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'build.ps1') -Raw
        foreach ($contract in @(
            "`$requiredWixVersion = '4.0.6'",
            "'^4\.0\.6(?:\+.*)?`$'",
            "'WixToolset.Bal.wixext/4.0.6'",
            "'WixToolset.Util.wixext/4.0.6'",
            "'WixToolset.Dependency.wixext/4.0.6'",
            'UserFinalizerExe=',
            "Join-Path `$payload 'bin/mo-tip-registrar.exe'"
        )) {
            if (-not $buildSource.Contains($contract, [StringComparison]::Ordinal)) {
                throw "Bundle build input contract is missing: $contract"
            }
        }
    }

    Pass 'bundle and native finalizer protocol agree' {
        $registrarSource = Get-Content -LiteralPath (
            Join-Path $repo 'native/windows-tip/src/registrar.cpp') -Raw
        $markerMatch = [regex]::Match(
            $registrarSource,
            'constexpr wchar_t kUserFinalizerMarker\[\] = L"([^"]+)";')
        if (-not $markerMatch.Success -or
            $markerMatch.Groups[1].Value -cne 'mo-user-finalizer-v1') {
            throw 'Native user-finalizer marker protocol mismatch.'
        }
        foreach ($command in @(
            'install-current-user-fixed',
            'repair-current-user-fixed',
            'remove-current-user-fixed',
            'rollback-install-current-user-fixed',
            'rollback-remove-current-user-fixed',
            'require_standard_current_user_process',
            'UserFinalizerTransaction',
            'Software\\Classes\\Local Settings\\Software\\Mo\\InputMethod\\Setup',
            'enabled ? 0 : kIlotUninstall'
        )) {
            if (-not $registrarSource.Contains($command, [StringComparison]::Ordinal)) {
                throw "Native user-finalizer protocol is missing: $command"
            }
        }
        if ($registrarSource -match '(?i)ILOT_(DEFPROFILE|DEFUSER4|CLEANINSTALL)') {
            throw 'Native finalizer contains a default/global input mutation flag.'
        }
    }

    if ($StageDirectory) {
        $stage = Assert-MoPlainPath $StageDirectory
        Pass 'real stage preflight' { $null = Assert-MoPreparedStage $stage }
        $first = Join-Path $fixture 'Payload.first.wxs'
        $second = Join-Path $fixture 'Payload.second.wxs'
        Pass 'full payload materialization' { $null = New-MoWixPayloadFragment $stage $first }
        Pass 'deterministic full payload bytes' {
            $null = New-MoWixPayloadFragment $stage $second
            if ((Get-Item -LiteralPath $first).Length -ne (Get-Item -LiteralPath $second).Length -or
                (Get-FileHash -LiteralPath $first).Hash -ine (Get-FileHash -LiteralPath $second).Hash) {
                throw 'Repeated payload authoring bytes differ.'
            }
        }
        Pass 'full payload one-to-one verification' { Assert-MoWixPayloadFragment $stage $first }

        $badSource = Join-Path $fixture 'Payload.bad-source.wxs'
        Copy-Item -LiteralPath $first -Destination $badSource
        $bad = Read-MoWixDocument $badSource
        $badNamespace = [Xml.XmlNamespaceManager]::new($bad.NameTable)
        $badNamespace.AddNamespace('w', 'http://wixtoolset.org/schemas/v4/wxs')
        $bad.SelectSingleNode('//w:File', $badNamespace).SetAttribute('Source', '$(var.StagePayload)\escape.bin')
        $bad.Save($badSource)
        Reject 'mutated payload source' { Assert-MoWixPayloadFragment $stage $badSource } 'contract mismatch'

        $missingReference = Join-Path $fixture 'Payload.missing-reference.wxs'
        Copy-Item -LiteralPath $first -Destination $missingReference
        $bad = Read-MoWixDocument $missingReference
        $badNamespace = [Xml.XmlNamespaceManager]::new($bad.NameTable)
        $badNamespace.AddNamespace('w', 'http://wixtoolset.org/schemas/v4/wxs')
        $reference = $bad.SelectSingleNode('//w:ComponentGroup[@Id="MoPayloadComponents"]/w:ComponentRef', $badNamespace)
        [void]$reference.ParentNode.RemoveChild($reference)
        $bad.Save($missingReference)
        Reject 'missing component reference' { Assert-MoWixPayloadFragment $stage $missingReference } 'count mismatch'

        $redirectedRoot = Join-Path $fixture 'Payload.redirected-root.wxs'
        Copy-Item -LiteralPath $first -Destination $redirectedRoot
        $bad = Read-MoWixDocument $redirectedRoot
        $badNamespace = [Xml.XmlNamespaceManager]::new($bad.NameTable)
        $badNamespace.AddNamespace('w', 'http://wixtoolset.org/schemas/v4/wxs')
        $root = $bad.SelectSingleNode(
            '//w:StandardDirectory[@Id="ProgramFiles64Folder"]', $badNamespace)
        $root.SetAttribute('Id', 'CommonAppDataFolder')
        $bad.Save($redirectedRoot)
        Reject 'redirected install root' {
            Assert-MoWixPayloadFragment $stage $redirectedRoot
        } 'install-root contract mismatch'

        Pass 'evidence excluded from install image' {
            $document = Read-MoWixDocument $first
            $ns = [Xml.XmlNamespaceManager]::new($document.NameTable)
            $ns.AddNamespace('w', 'http://wixtoolset.org/schemas/v4/wxs')
            $sources = @($document.SelectNodes('//w:File', $ns) | ForEach-Object { $_.GetAttribute('Source') })
            if ($sources -match '\\evidence\\|mo-stage\.json') { throw 'Evidence leaked into installed payload.' }
            $expectedCount = @(Get-MoStageFiles (Join-Path $stage 'payload/Mo')).Count
            if ($sources.Count -ne $expectedCount) { throw 'Installed payload count mismatch.' }
        }
    }
    Write-Host "Package authoring tests passed: $script:packageTests. No installation, registration, elevation, signing or network access."
} finally {
    $resolved = Assert-MoPlainPath $fixture
    if (-not $resolved.StartsWith((Join-Path $repo 'build') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Unsafe package fixture cleanup target.'
    }
    Assert-MoOwnedFixtureTree $resolved
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
