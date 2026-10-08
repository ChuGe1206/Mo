#Requires -Version 5.1
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'changed-upgrade-policy.ps1')
$script:testCount = 0

function New-InstalledState {
    return [ordered]@{
        'com.x64' = 'missing'; 'com.x86' = 'missing'
        'profile.registered' = 'true'; 'profile.enabled' = 'true'; 'profile.active' = 'false'
        'user.finalizer' = 'v1'; 'user.finalizer.transaction' = 'mo-user-finalizer-install-v1-disabled'
    }
}

function Run-Case(
    [string]$Mode,
    [bool]$ExpectedSuccess,
    [string]$FailurePattern,
    [bool]$ExpectedAttempt
) {
    $script:scenario = [ordered]@{
        mode = $Mode; calls = [Collections.Generic.List[string]]::new()
        settings_reads = 0; default_reads = 0
    }
    $flowResult = [ordered]@{
        completed = $true; base_payload_verified = $true; final_state_verified = $true
    }
    $operations = [ordered]@{
        Preflight = {
            $script:scenario.calls.Add('preflight')
            if ($script:scenario.mode -ceq 'preflight') { throw 'synthetic preflight failure' }
        }
        ValidateBase = {
            $script:scenario.calls.Add('base')
            if ($script:scenario.mode -ceq 'base') { throw 'synthetic base failure' }
        }
        ReadSettings = {
            $script:scenario.settings_reads++
            $script:scenario.calls.Add("settings$($script:scenario.settings_reads)")
            if ($script:scenario.settings_reads -eq 1 -and $script:scenario.mode -ceq 'settings-read') {
                throw 'synthetic settings read failure'
            }
            if ($script:scenario.mode -ceq 'absent-success') {
                return [pscustomobject]@{ present = $false; size = $null; sha256 = $null }
            }
            if ($script:scenario.mode -ceq 'settings-created' -and $script:scenario.settings_reads -eq 1) {
                return [pscustomobject]@{ present = $false; size = $null; sha256 = $null }
            }
            $hash = 'A' * 64
            if ($script:scenario.mode -ceq 'settings-changed' -and $script:scenario.settings_reads -eq 2) {
                $hash = 'B' * 64
            }
            return [pscustomobject]@{ present = $true; size = 1; sha256 = $hash }
        }
        ReadDefault = {
            $script:scenario.default_reads++
            $script:scenario.calls.Add("default$($script:scenario.default_reads)")
            if ($script:scenario.mode -ceq 'default-read') { throw 'synthetic default read failure' }
            if ($script:scenario.mode -ceq 'default-changed' -and $script:scenario.default_reads -eq 2) {
                return 'synthetic-other-default'
            }
            return 'synthetic-default'
        }
        Install = {
            $script:scenario.calls.Add('install')
            switch -CaseSensitive ($script:scenario.mode) {
                'install-throw' { throw 'synthetic installer invocation failure' }
                'primary-and-final' { throw 'synthetic primary failure' }
                'exit1603' { return 1603 }
                'exit3010' { return 3010 }
                'no-exit' { return $null }
                'multiple-exits' { return @(0, 0) }
                default { return 0 }
            }
        }
        ValidateUpgrade = {
            $script:scenario.calls.Add('upgrade')
            if ($script:scenario.mode -ceq 'upgrade') { throw 'synthetic upgrade verification failure' }
        }
        ReadFinalState = {
            $script:scenario.calls.Add('final')
            if ($script:scenario.mode -cin @('final-read', 'primary-and-final')) {
                throw 'synthetic final read failure'
            }
            $state = New-InstalledState
            if ($script:scenario.mode -ceq 'final-active') { $state['profile.active'] = 'true' }
            return $state
        }
    }
    $caught = $null
    try { Invoke-MoVmChangedUpgradeTransaction $flowResult $operations } catch { $caught = $_ }
    if ($ExpectedSuccess) {
        if ($null -ne $caught -or -not $flowResult.completed -or
            -not $flowResult.final_state_verified -or $null -ne $flowResult.failure) {
            throw "Expected successful flow: $Mode"
        }
        $expectedOrder = 'preflight,base,settings1,default1,install,upgrade,settings2,default2,final'
        if (($script:scenario.calls -join ',') -cne $expectedOrder) { throw "Wrong successful order: $Mode" }
        foreach ($name in @('base_payload_verified', 'upgrade_payload_verified',
            'install_tree_security_audited', 'settings_preserved', 'default_override_unchanged')) {
            if (-not $flowResult[$name]) { throw "Missing successful check: $name" }
        }
    } else {
        if ($null -eq $caught -or $caught.Exception.Message -notmatch $FailurePattern -or
            $flowResult.completed -or $flowResult.failure -notmatch $FailurePattern -or
            $flowResult.final_state_verified) { throw "Expected failed flow: $Mode" }
    }
    if ($flowResult.installer_invocation_attempted -ne $ExpectedAttempt -or
        @($script:scenario.calls | Where-Object { $_ -ceq 'install' }).Count -ne [int]$ExpectedAttempt -or
        @($script:scenario.calls | Where-Object { $_ -ceq 'final' }).Count -ne 1) {
        throw "Wrong invocation/final audit count: $Mode"
    }
    if ($Mode -cin @('exit1603','exit3010','install-throw','no-exit','multiple-exits','primary-and-final') -and
        $script:scenario.calls.Contains('upgrade')) { throw "Upgrade checked after failed installer: $Mode" }
    if ($Mode -ceq 'settings-changed' -or $Mode -ceq 'settings-created') {
        if ($flowResult.settings_preserved -or $flowResult.default_override_unchanged) {
            throw 'Failed settings protection reported success.'
        }
    }
    if ($Mode -ceq 'default-changed' -and
        (-not $flowResult.settings_preserved -or $flowResult.default_override_unchanged)) {
        throw 'Default mutation check reported incorrect flags.'
    }
    if ($Mode -ceq 'final-active' -and $flowResult.final_state['profile.active'] -cne 'true') {
        throw 'Failed final state observation was lost.'
    }
    if ($Mode -ceq 'primary-and-final' -and
        ($flowResult.failure -cne 'synthetic primary failure' -or
        $flowResult.final_state_failure -cne 'synthetic final read failure')) {
        throw 'Final audit masked the primary failure.'
    }
    if ($Mode -ceq 'exit3010' -and $flowResult.exit_code -ne 3010) { throw 'Reboot exit lost.' }
    $script:testCount++
    Write-Host "PASS $Mode"
}

Run-Case 'success' $true '' $true
Run-Case 'absent-success' $true '' $true
foreach ($case in @(
    @('preflight','preflight failure',$false),
    @('base','base failure',$false),
    @('settings-read','settings read failure',$false),
    @('default-read','default read failure',$false),
    @('install-throw','installer invocation failure',$true),
    @('exit1603','reboot: 1603',$true),
    @('exit3010','reboot: 3010',$true),
    @('no-exit','one integer exit code',$true),
    @('multiple-exits','one integer exit code',$true),
    @('upgrade','upgrade verification failure',$true),
    @('settings-changed','changed the user settings',$true),
    @('settings-created','changed the user settings',$true),
    @('default-changed','default input method override',$true),
    @('final-read','final read failure',$true),
    @('final-active','profile.active',$true),
    @('primary-and-final','primary failure',$true)
)) {
    Run-Case $case[0] $false $case[1] $case[2]
}
# Exercise the actual entry-point operation bodies through in-memory adapters.
# The entry point itself is never run past its VM guard on this host.
function Run-ProductionAdapterCase([bool]$FailFinal) {
    $script:productionInstalled = $false
    $script:productionStatusReads = 0
    $script:productionFailFinal = $FailFinal
    $script:productionInstallCalls = 0
    $kit = [pscustomobject]@{ base_product_code = 'synthetic-base'; upgrade_product_code = 'synthetic-upgrade' }
    $baseContract = 'synthetic-base-contract'
    $upgradeContract = 'synthetic-upgrade-contract'
    $installRoot = 'C:\synthetic-install'
    $settingsPath = 'C:\synthetic-settings.mo'
    $bundle = 'C:\synthetic kit\mo-setup.exe'
    $evidence = 'C:\synthetic evidence'
    function Get-Process {
        [CmdletBinding()]param([string[]]$Name)
        if (($Name -join ',') -cne 'mo-broker,mo-settings') { throw 'Unexpected process query.' }
    }
    function Test-Path {
        [CmdletBinding()]param([string]$LiteralPath)
        if ($LiteralPath -cne 'C:\synthetic-settings.mo.write-lock') { throw 'Unexpected path query.' }
        return $false
    }
    function Assert-ProductState([string]$Code, [int]$Expected) {
        $actual = if ($Code -ceq 'synthetic-base') {
            if ($script:productionInstalled) { -1 } else { 5 }
        } elseif ($Code -ceq 'synthetic-upgrade') {
            if ($script:productionInstalled) { 5 } else { -1 }
        } else { throw 'Wrong product identity.' }
        if ($actual -ne $Expected) { throw 'Wrong production MSI expectation.' }
    }
    function Read-RegistrarState {
        $script:productionStatusReads++
        if ($script:productionFailFinal -and $script:productionStatusReads -eq 3) {
            throw 'synthetic production final failure'
        }
        return New-InstalledState
    }
    function Assert-MoVmMachineComState([string]$State, [string]$Root) {
        if ($State -cne 'Installed' -or $Root -cne 'C:\synthetic-install') { throw 'Wrong COM query.' }
    }
    function Assert-MoVmInstalledPayload([string]$Root, [object]$Contract) {
        $expected = if ($script:productionInstalled) { 'synthetic-upgrade-contract' } else { 'synthetic-base-contract' }
        if ($Root -cne 'C:\synthetic-install' -or $Contract -cne $expected) { throw 'Wrong production payload contract.' }
    }
    function Assert-MoVmInstalledSecurity([string]$Root) {
        if ($Root -cne 'C:\synthetic-install') { throw 'Wrong ACL query.' }
    }
    function Get-MoVmSettingsFingerprint([string]$Path) {
        if ($Path -cne 'C:\synthetic-settings.mo') { throw 'Wrong settings query.' }
        return [pscustomobject]@{ present = $true; size = 1; sha256 = ('A' * 64) }
    }
    function Get-WinDefaultInputMethodOverride { return 'synthetic-default' }
    function Start-Process {
        [CmdletBinding()]param([string]$FilePath, [string[]]$ArgumentList, [switch]$Wait, [switch]$PassThru)
        if ($FilePath -cne 'C:\synthetic kit\mo-setup.exe' -or -not $Wait -or -not $PassThru -or
            ($ArgumentList -join '|') -cne '/install|/quiet|/norestart|/log|"C:\synthetic evidence\upgrade.log"') {
            throw 'Wrong Burn invocation or missing synchronous completion.'
        }
        $script:productionInstallCalls++
        $script:productionInstalled = $true
        return [pscustomobject]@{ ExitCode = 0 }
    }
    $tokens = $null; $errors = $null
    $ast = [Management.Automation.Language.Parser]::ParseFile(
        (Join-Path $PSScriptRoot 'run-vm-changed-payload-upgrade.ps1'), [ref]$tokens, [ref]$errors)
    if ($errors.Count) { throw 'Invalid production driver AST.' }
    $assignments = @($ast.FindAll({
        param($node)
        $node -is [Management.Automation.Language.AssignmentStatementAst] -and
        $node.Left -is [Management.Automation.Language.VariableExpressionAst] -and
        $node.Left.VariablePath.UserPath -ceq 'operations'
    }, $true))
    if ($assignments.Count -ne 1) { throw 'Expected exactly one production operation set.' }
    $table = $assignments[0].Right.Find({
        param($node) $node -is [Management.Automation.Language.HashtableAst]
    }, $true)
    $productionOperations = [ordered]@{}
    foreach ($pair in $table.KeyValuePairs) {
        $body = $pair.Item2.Find({
            param($node) $node -is [Management.Automation.Language.ScriptBlockExpressionAst]
        }, $true).ScriptBlock.Extent.Text
        $productionOperations[$pair.Item1.Value] = [scriptblock]::Create($body.Substring(1, $body.Length - 2))
    }
    $adapterResult = [ordered]@{}
    $caught = $null
    try { Invoke-MoVmChangedUpgradeTransaction $adapterResult $productionOperations } catch { $caught = $_ }
    if ($script:productionInstallCalls -ne 1 -or $script:productionStatusReads -ne 3) {
        throw 'Wrong production invocation or final read count.'
    }
    if ($FailFinal) {
        if ($null -eq $caught -or $caught.Exception.Message -cne 'synthetic production final failure' -or
            $adapterResult.completed -or $adapterResult.final_state_verified) {
            throw 'Production final audit failure reported success.'
        }
    } elseif ($null -ne $caught -or -not $adapterResult.completed -or -not $adapterResult.final_state_verified) {
        throw "Production operations failed: $($caught.Exception.Message)"
    }
    $script:testCount++
    Write-Host "PASS production operation bodies final_failure=$FailFinal"
}
Run-ProductionAdapterCase $false
Run-ProductionAdapterCase $true

$script:invalidInstallCalled = $false
$badOperations = [ordered]@{ Install = { $script:invalidInstallCalled = $true; return 0 } }
$rejected = $false
try { Invoke-MoVmChangedUpgradeTransaction ([ordered]@{}) $badOperations } catch {
    if ($_.Exception.Message -notmatch 'Invalid upgrade operation set'){throw}
    $rejected = $true
}
if (-not $rejected -or $script:invalidInstallCalled) { throw 'Invalid operation set executed an installer callback.' }
$script:testCount++
Write-Host "Upgrade transaction tests passed: $script:testCount. In-memory callbacks only; no files, registry or installers changed."
