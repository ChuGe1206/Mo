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
        if ($package.DocumentElement.GetAttribute('RequiredVersion') -cne '4.0.6' -or
            $null -eq $packageNode -or $packageNode.GetAttribute('Scope') -cne 'perMachine') {
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
        $failureProperty = $package.SelectSingleNode(
            '//w:Property[@Id="MO_TEST_FAIL_AFTER_MACHINE_PROFILE"]', $namespace)
        if ($null -eq $failureProperty -or $failureProperty.GetAttribute('Secure') -cne 'yes' -or
            $failureProperty.HasAttribute('Value')) {
            throw 'Package failure property must be secure and default-unset.'
        }
    }

    $expectedActions = [ordered]@{
        RollbackInstallMachineProfile = @('rollback-install-machine-profile-fixed', 'rollback', 'ignore')
        InstallNewMachineProfile = @('install-new-machine-profile-fixed', 'deferred', 'check')
        InstallMachineProfile = @('install-machine-profile-fixed', 'deferred', 'check')
        DevelopmentFailAfterMachineProfile = @('development-test-fail-fixed', 'deferred', 'check')
        CommitInstallMachineProfile = @('commit-install-machine-profile-fixed', 'commit', 'check')
        RollbackRemoveMachineProfile = @('rollback-remove-machine-profile-fixed', 'rollback', 'ignore')
        RemoveMachineProfile = @('remove-machine-profile-fixed', 'deferred', 'check')
        CommitRemoveMachineProfile = @('commit-remove-machine-profile-fixed', 'commit', 'check')
    }
    Pass 'eight elevated embedded transaction actions' {
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
        if ($sequence.Count -ne 9) { throw 'Unexpected conditional custom-action sequence count.' }
        $contracts = @(
            @('RollbackRemoveMachineProfile', 'Before', 'RemoveMachineProfile', 'REMOVE~="ALL" AND NOT UPGRADINGPRODUCTCODE'),
            @('RemoveMachineProfile', 'Before', 'CommitRemoveMachineProfile', 'REMOVE~="ALL" AND NOT UPGRADINGPRODUCTCODE'),
            @('CommitRemoveMachineProfile', 'Before', 'RemoveFiles', 'REMOVE~="ALL" AND NOT UPGRADINGPRODUCTCODE'),
            @('RollbackInstallMachineProfile', 'After', 'InstallFiles', 'NOT REMOVE~="ALL"'),
            @('InstallNewMachineProfile', 'After', 'RollbackInstallMachineProfile', 'NOT Installed AND NOT WIX_UPGRADE_DETECTED AND NOT REMOVE~="ALL"'),
            @('InstallMachineProfile', 'After', 'InstallNewMachineProfile', '(Installed OR WIX_UPGRADE_DETECTED) AND NOT REMOVE~="ALL"'),
            @('DevelopmentFailAfterMachineProfile', 'After', 'InstallMachineProfile', 'MO_TEST_FAIL_AFTER_MACHINE_PROFILE = "1" AND NOT REMOVE~="ALL"'),
            @('CommitInstallMachineProfile', 'After', 'DevelopmentFailAfterMachineProfile', 'NOT REMOVE~="ALL"'),
            @('CommitInstallMachineProfile', 'After', 'InstallMachineProfile', 'NOT REMOVE~="ALL"')
        )
        foreach ($contract in $contracts) {
            $matches = @($sequence | Where-Object {
                $_.GetAttribute('Action') -ceq $contract[0] -and
                $_.GetAttribute($contract[1]) -ceq $contract[2] -and
                $_.GetAttribute('Condition') -ceq $contract[3]
            })
            if ($matches.Count -ne 1) {
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
            (Get-MoWixFileId 'bin/mo-settings.exe') -cne 'SettingsX64File' -or
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
        if ($bundle.DocumentElement.GetAttribute('RequiredVersion') -cne '4.0.6') {
            throw 'Bundle does not require the locked WiX authoring version.'
        }
        $packages = @($bundle.SelectNodes('/w:Wix/w:Bundle/w:Chain/*', $bundleNamespace))
        if ($packages.Count -ne 3 -or $packages[0].LocalName -cne 'MsiPackage' -or
            $packages[0].GetAttribute('Id') -cne 'MoMachinePackage' -or
            $packages[0].GetAttribute('Visible') -cne 'no') {
            throw 'Bundle machine package contract mismatch.'
        }
        $msiProperty = $packages[0].SelectSingleNode('w:MsiProperty', $bundleNamespace)
        if ($null -eq $msiProperty -or
            $msiProperty.GetAttribute('Name') -cne 'MO_TEST_FAIL_AFTER_MACHINE_PROFILE' -or
            $msiProperty.GetAttribute('Value') -cne '[MoTestFailAfterMachineProfile]' -or
            $msiProperty.GetAttribute('Condition') -cne 'MoTestFailAfterMachineProfile = 1') {
            throw 'Bundle machine failure-injection property contract mismatch.'
        }
        $finalizer = $packages[1]
        $expected = [ordered]@{
            Id = 'MoCurrentUserFinalizer'
            After = 'MoMachinePackage'
            SourceFile = '$(var.UserFinalizerExe)'
            DetectCondition = 'MoUserFinalizerMarker = "mo-user-finalizer-v1"'
            InstallArguments = 'burn-user-finalizer'
            RepairArguments = 'burn-user-finalizer'
            UninstallArguments = 'burn-user-finalizer'
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
                throw "Bundle finalizer must author the fixed Burn protocol prefix: $attribute"
            }
        }
        $commandLines = @($finalizer.SelectNodes('w:CommandLine', $bundleNamespace))
        $expectedCommandLines = [ordered]@{
            'WixBundleAction = 4' = [ordered]@{
                InstallArgument = 'rollback-remove-current-user-fixed'
                UninstallArgument = 'remove-current-user-fixed'
            }
            'WixBundleAction = 6' = [ordered]@{
                InstallArgument = 'install-current-user-fixed'
                UninstallArgument = 'rollback-install-current-user-fixed'
            }
            'WixBundleAction = 8' = [ordered]@{
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
        $injector = $packages[2]
        $injectorExpected = [ordered]@{
            Id = 'MoDevelopmentFailureInjection'
            After = 'MoCurrentUserFinalizer'
            SourceFile = '$(var.FailureInjectorExe)'
            InstallCondition = 'MoTestFailAfterUserFinalizer = 1'
            DetectCondition = '0'
            InstallArguments = 'development-test-fail-fixed'
            CacheId = 'MoDevelopmentFailureInjection.v1'
            PerMachine = 'no'
            Cache = 'keep'
            Permanent = 'yes'
            Vital = 'yes'
        }
        if ($injector.LocalName -cne 'ExePackage') { throw 'Bundle rollback probe is not an ExePackage.' }
        foreach ($attribute in $injectorExpected.Keys) {
            if ($injector.GetAttribute($attribute) -cne $injectorExpected[$attribute]) {
                throw "Bundle rollback probe mismatch: $attribute"
            }
        }
    }

    Pass 'bundle failure injection is hidden, explicit and default-off' {
        $variables = @($bundle.SelectNodes('/w:Wix/w:Bundle/w:Variable', $bundleNamespace))
        $expectedNames = @('MoTestFailAfterMachineProfile', 'MoTestFailAfterUserFinalizer')
        if ($variables.Count -ne 2 -or
            @($expectedNames | Where-Object {
                $_ -cnotin @($variables | ForEach-Object { $_.GetAttribute('Name') })
            }).Count) {
            throw 'Unexpected Bundle variable set.'
        }
        foreach ($variable in $variables) {
            if ($variable.GetAttribute('Name') -cnotin $expectedNames -or
                $variable.GetAttribute('Type') -cne 'numeric' -or
                $variable.GetAttribute('Value') -cne '0' -or
                $variable.GetAttribute('Hidden') -cne 'yes' -or
                $variable.GetAttribute('Overridable', 'http://wixtoolset.org/schemas/v4/wxs/bal') -cne 'yes') {
                throw "Unsafe Bundle failure-injection variable: $($variable.GetAttribute('Name'))"
            }
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

    Pass 'bundle build binds locked toolchain, staged finalizer and linked verifier' {
        $buildSource = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'build.ps1') -Raw
        foreach ($contract in @(
            'Assert-MoWixToolchain',
            "[ValidateSet('DevelopmentTest', 'ProductionShape')]",
            'AllowProductionShapeBuild',
            "if (`$BuildFlavor -eq 'DevelopmentTest') { '1' } else { '0' }",
            'IncludeFaultInjection=',
            '$toolchain.Extensions.Bal',
            '$toolchain.Extensions.Util',
            '$toolchain.Extensions.Dependency',
            "'-sw1140'",
            'UserFinalizerExe=',
            'FailureInjectorExe=',
            "Join-Path `$payload 'bin/mo-tip-registrar.exe'",
            'mo-development-failure-injection.exe',
            "Join-Path `$PSScriptRoot 'verify-linked-installer.ps1'",
            '-ValidateMsi:$ValidateMsi'
        )) {
            if (-not $buildSource.Contains($contract, [StringComparison]::Ordinal)) {
                throw "Bundle build input contract is missing: $contract"
            }
        }
    }

    Pass 'production shape excludes development fault injection at every layer' {
        $packageSource = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'Package.wxs') -Raw
        $bundleSource = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'Bundle.wxs') -Raw
        $registrarSource = Get-Content -LiteralPath (
            Join-Path $repo 'native/windows-tip/src/registrar.cpp') -Raw
        $projectSource = Get-Content -LiteralPath (
            Join-Path $repo 'native/windows-tip/MoTipRegistrar.vcxproj') -Raw
        $cmakeSource = Get-Content -LiteralPath (
            Join-Path $repo 'native/windows-tip/CMakeLists.txt') -Raw
        foreach ($source in @($packageSource, $bundleSource)) {
            if ($source -notmatch '<\?if \$\(var\.IncludeFaultInjection\) = 1 \?>' -or
                $source -notmatch '<\?endif\?>') {
                throw 'WiX fault-injection authoring is not preprocessor-gated.'
            }
        }
        if ($registrarSource -notmatch '#if defined\(MO_DEVELOPMENT_FAULT_INJECTION\)[\s\S]+development-test-fail-fixed[\s\S]+#endif' -or
            $projectSource -notmatch "'\$\(MoDevelopmentFaultInjection\)' == 'true'" -or
            $projectSource -notmatch 'MO_DEVELOPMENT_FAULT_INJECTION' -or
            $cmakeSource -notmatch 'option\(MO_DEVELOPMENT_FAULT_INJECTION[\s\S]+OFF\)' -or
            $cmakeSource -notmatch 'target_compile_definitions\(mo_tip_registrar PRIVATE MO_DEVELOPMENT_FAULT_INJECTION\)') {
            throw 'Native registrar fault injection is not default-off and compile-time-gated.'
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
            'burn-user-finalizer',
            'development-test-fail-fixed',
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
        $crossComponentCom = Join-Path $fixture 'Payload.cross-component-com.wxs'
        Copy-Item -LiteralPath $first -Destination $crossComponentCom
        $bad = Read-MoWixDocument $crossComponentCom
        $badNamespace = [Xml.XmlNamespaceManager]::new($bad.NameTable)
        $badNamespace.AddNamespace('w', 'http://wixtoolset.org/schemas/v4/wxs')
        $bad.SelectSingleNode(
            '//w:Component[@Id="TipX86ComRegistryComponent"]/w:RegistryKey/w:RegistryValue[not(@Name)]',
            $badNamespace).SetAttribute('Value', '[#TipX86File]')
        $bad.Save($crossComponentCom)
        Reject 'x86 COM cross-component file reference' {
            Assert-MoWixPayloadFragment $stage $crossComponentCom
        } 'x86 COM value contract mismatch'
        Pass 'settings Start-menu shortcut is advertised and file-owned' {
            $document = Read-MoWixDocument $first
            $ns = [Xml.XmlNamespaceManager]::new($document.NameTable)
            $ns.AddNamespace('w', 'http://wixtoolset.org/schemas/v4/wxs')
            $shortcut = $document.SelectSingleNode(
                '//w:File[@Id="SettingsX64File"]/w:Shortcut[@Id="MoSettingsStartMenuShortcut"]', $ns)
            if ($null -eq $shortcut -or $shortcut.GetAttribute('Directory') -cne 'ProgramMenuFolder' -or
                $shortcut.GetAttribute('Advertise') -cne 'yes') {
                throw 'Settings Start-menu shortcut contract mismatch.'
            }
        }

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
