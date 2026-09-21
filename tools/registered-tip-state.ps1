# Shared development-test transaction. Dot-source only; no mutation on load.

function ConvertFrom-MoRegistrarStatus([string[]]$Lines) {
    $state = @{}
    foreach ($line in $Lines) {
        if ($line -notmatch '^([^=]+)=(.*)$' -or $state.ContainsKey($Matches[1])) {
            throw "Invalid or duplicate registrar status line: $line"
        }
        $state[$Matches[1]] = $Matches[2]
    }
    foreach ($name in @('com.x64', 'com.x86', 'profile.registered', 'profile.enabled', 'profile.active', 'user.finalizer', 'user.finalizer.transaction')) {
        if (-not $state.ContainsKey($name)) { throw "Registrar status is missing $name" }
    }
    foreach ($name in @('profile.registered', 'profile.enabled', 'profile.active')) {
        if ($state[$name] -notin @('true', 'false')) { throw "Invalid registrar boolean: $name" }
    }
    if ($state['user.finalizer'] -notin @('missing', 'v1', 'invalid')) {
        throw 'Invalid registrar user.finalizer state.'
    }
    $validTransactions = @(
        'missing', 'invalid',
        'mo-user-finalizer-install-v1-disabled',
        'mo-user-finalizer-install-v1-enabled',
        'mo-user-finalizer-repair-v1-disabled',
        'mo-user-finalizer-repair-v1-enabled',
        'mo-user-finalizer-remove-v1-disabled',
        'mo-user-finalizer-remove-v1-enabled'
    )
    if ($state['user.finalizer.transaction'] -notin $validTransactions) {
        throw 'Invalid registrar user.finalizer.transaction state.'
    }
    return $state
}

function New-MoRegistrarCommand([string]$RegistrarPath) {
    if (-not (Test-Path -LiteralPath $RegistrarPath -PathType Leaf)) {
        throw "Missing registrar: $RegistrarPath. Build the x64/Win32 probes first."
    }
    return {
        param([string]$Operation, [string[]]$OperationArguments)
        $lines = @(& $RegistrarPath $Operation @OperationArguments)
        if ($LASTEXITCODE -ne 0) { throw "Registrar $Operation failed: $LASTEXITCODE" }
        if ($Operation -eq 'status') { return ConvertFrom-MoRegistrarStatus $lines }
    }.GetNewClosure()
}

function Assert-MoRegisteredCleanState([hashtable]$State) {
    $expected = @{
        'com.x64' = 'missing'; 'com.x86' = 'missing'
        'profile.registered' = 'true'; 'profile.enabled' = 'false'; 'profile.active' = 'false'
        'user.finalizer' = 'missing'
        'user.finalizer.transaction' = 'missing'
    }
    foreach ($name in $expected.Keys) {
        if ($State[$name] -ne $expected[$name]) {
            throw "Registered test requires $name=$($expected[$name]); found $($State[$name]). Prepare the machine profile with tools\machine-profile.ps1 -Action Register in an administrator PowerShell, then run the test in a normal PowerShell."
        }
    }
}

function Assert-MoRegisteredUserPreflight([string]$RegistrarPath) {
    $moIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $moPrincipal = [Security.Principal.WindowsPrincipal]::new($moIdentity)
    if ($moPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Registered user tests and their Broker must run without elevation. Use administrator PowerShell only for machine profile preparation/cleanup.'
    }
    $command = New-MoRegistrarCommand $RegistrarPath
    Assert-MoRegisteredCleanState (& $command 'status' @())
}

function Invoke-MoRegisteredTransaction(
    [scriptblock]$RegistrarCommand,
    [string]$TipX64,
    [string]$TipX86,
    [scriptblock]$ProbeBody
) {
    # Testable transaction core; production entry points perform the privilege
    # and artifact preflight separately. The command adapter throws on failure.
    Assert-MoRegisteredCleanState (& $RegistrarCommand 'status' @())
    $comAttempted = $false
    $enableAttempted = $false
    $primaryFailure = $null
    $cleanupFailures = [Collections.Generic.List[string]]::new()
    try {
        # Mark before invoking: a failed native call can still mutate state.
        # Initial user state was absent/disabled, so partial writes belong here.
        $comAttempted = $true
        & $RegistrarCommand 'register-com-user' @($TipX64, $TipX86)
        $registered = & $RegistrarCommand 'status' @()
        if ($registered['com.x64'] -ne $TipX64 -or $registered['com.x86'] -ne $TipX86) {
            throw 'COM registration readback does not match both test binaries.'
        }
        $enableAttempted = $true
        & $RegistrarCommand 'enable-current-user' @()
        $enabled = & $RegistrarCommand 'status' @()
        if ($enabled['profile.enabled'] -ne 'true') { throw 'Profile enable readback failed.' }
        & $ProbeBody
    } catch {
        $primaryFailure = $_
    } finally {
        # Each cleanup is attempted independently even if an earlier one fails.
        if ($enableAttempted) {
            try { & $RegistrarCommand 'disable-current-user' @() }
            catch { $cleanupFailures.Add($_.Exception.Message) }
        }
        if ($comAttempted) {
            try {
                $current = & $RegistrarCommand 'status' @()
                foreach ($item in @(@('com.x64', $TipX64), @('com.x86', $TipX86))) {
                    if ($current[$item[0]] -notin @('missing', $item[1])) {
                        throw "Refusing COM cleanup: $($item[0]) changed to a foreign path. Manual ownership review is required."
                    }
                }
                & $RegistrarCommand 'unregister-com-user' @()
            } catch { $cleanupFailures.Add($_.Exception.Message) }
        }
        try { Assert-MoRegisteredCleanState (& $RegistrarCommand 'status' @()) }
        catch { $cleanupFailures.Add($_.Exception.Message) }
    }
    if ($cleanupFailures.Count -ne 0) {
        $primary = if ($null -ne $primaryFailure) { " Probe/setup failed: $($primaryFailure.Exception.Message)." } else { '' }
        throw "Registered test cleanup failed; success must not be reported.$primary Cleanup: $($cleanupFailures -join ' | ')"
    }
    if ($null -ne $primaryFailure) { throw $primaryFailure }
}

function Invoke-MoRegisteredUserTest(
    [string]$RegistrarPath,
    [string]$TipX64,
    [string]$TipX86,
    [scriptblock]$ProbeBody
) {
    Assert-MoRegisteredUserPreflight $RegistrarPath
    foreach ($artifact in @($TipX64, $TipX86)) {
        if (-not (Test-Path -LiteralPath $artifact -PathType Leaf) -or -not [IO.Path]::IsPathFullyQualified($artifact)) {
            throw "Registered test requires an existing absolute TIP path: $artifact"
        }
    }
    Invoke-MoRegisteredTransaction (New-MoRegistrarCommand $RegistrarPath) $TipX64 $TipX86 $ProbeBody
}
