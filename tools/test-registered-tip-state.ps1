[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'registered-tip-state.ps1')

# This suite uses only an in-memory registrar. No COM/profile/registry mutation.
$caseCount = 0
function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

$valid = @(
    'com.x64=missing', 'com.x86=missing',
    'profile.registered=true', 'profile.enabled=false', 'profile.active=false',
    'user.finalizer=missing', 'user.finalizer.transaction=missing'
)
Assert-MoRegisteredCleanState (ConvertFrom-MoRegistrarStatus $valid)
$caseCount++
foreach ($bad in @(
    ($valid + 'com.x64=foreign'),
    ($valid | Where-Object { $_ -notmatch '^profile.active=' }),
    ($valid -replace 'profile.enabled=false', 'profile.enabled=maybe'),
    ($valid -replace 'user.finalizer=missing', 'user.finalizer=unknown'),
    ($valid -replace 'user.finalizer.transaction=missing', 'user.finalizer.transaction=unknown'),
    ($valid + 'not-a-status-line')
)) {
    $rejected = $false
    try { [void](ConvertFrom-MoRegistrarStatus $bad) } catch { $rejected = $true }
    Assert-True $rejected 'Malformed registrar status was accepted.'
    $caseCount++
}

foreach ($scenario in @('success', 'body-failure', 'partial-com', 'partial-enable',
    'disable-failure', 'unregister-failure', 'silent-enable-residue', 'silent-com-residue',
    'foreign-com-path', 'dirty-user-state', 'missing-machine-profile')) {
    $fixture = @{
        Scenario = $scenario
        Calls = [Collections.Generic.List[string]]::new()
        BodyCalls = 0
        State = ConvertFrom-MoRegistrarStatus $valid
    }
    if ($scenario -eq 'dirty-user-state') { $fixture.State['com.x64'] = 'foreign.dll' }
    if ($scenario -eq 'missing-machine-profile') { $fixture.State['profile.registered'] = 'false' }
    $command = {
        param([string]$Operation, [string[]]$OperationArguments)
        $fixture.Calls.Add($Operation)
        switch ($Operation) {
            'status' { return $fixture.State.Clone() }
            'register-com-user' {
                $fixture.State['com.x64'] = $OperationArguments[0]
                if ($fixture.Scenario -eq 'partial-com') { throw 'partial COM write' }
                $fixture.State['com.x86'] = $OperationArguments[1]
            }
            'enable-current-user' {
                $fixture.State['profile.enabled'] = 'true'
                if ($fixture.Scenario -eq 'partial-enable') { throw 'partial enable write' }
            }
            'disable-current-user' {
                if ($fixture.Scenario -eq 'disable-failure') { throw 'disable failed' }
                if ($fixture.Scenario -ne 'silent-enable-residue') { $fixture.State['profile.enabled'] = 'false' }
            }
            'unregister-com-user' {
                if ($fixture.Scenario -eq 'unregister-failure') { throw 'unregister failed' }
                if ($fixture.Scenario -ne 'silent-com-residue') {
                    $fixture.State['com.x64'] = 'missing'; $fixture.State['com.x86'] = 'missing'
                }
            }
            default { throw "Unexpected mock operation: $Operation" }
        }
    }.GetNewClosure()
    $body = {
        $fixture.BodyCalls++
        if ($fixture.Scenario -eq 'foreign-com-path') { $fixture.State['com.x64'] = 'foreign.dll' }
        if ($fixture.Scenario -eq 'body-failure') { throw 'original probe failure' }
    }.GetNewClosure()
    $failure = $null
    try { Invoke-MoRegisteredTransaction $command 'x64.dll' 'x86.dll' $body }
    catch { $failure = $_.Exception.Message }
    if ($scenario -eq 'success') {
        Assert-True ($null -eq $failure -and $fixture.BodyCalls -eq 1) 'Successful body failed or did not run exactly once.'
    } else { Assert-True ($null -ne $failure) "Expected rejection/cleanup failure for $scenario" }
    if ($scenario -in @('success', 'body-failure', 'partial-com', 'partial-enable')) {
        Assert-MoRegisteredCleanState $fixture.State
    }
    if ($scenario -eq 'body-failure') { Assert-True ($failure -match 'original probe failure') 'Original failure was lost.' }
    if ($scenario -eq 'partial-com') {
        Assert-True ($fixture.Calls.Contains('unregister-com-user') -and $fixture.BodyCalls -eq 0) 'Partial COM write was not rolled back.'
    }
    if ($scenario -eq 'partial-enable') {
        Assert-True ($fixture.Calls.Contains('disable-current-user') -and $fixture.BodyCalls -eq 0) 'Partial enable write was not rolled back.'
    }
    if ($scenario -eq 'disable-failure') {
        Assert-True ($fixture.Calls.Contains('unregister-com-user')) 'Disable error prevented independent COM cleanup.'
    }
    if ($scenario -in @('disable-failure', 'unregister-failure', 'silent-enable-residue', 'silent-com-residue', 'foreign-com-path')) {
        Assert-True ($failure -match 'cleanup failed') 'Cleanup error/residue was reported as success.'
    }
    if ($scenario -eq 'foreign-com-path') {
        Assert-True (-not $fixture.Calls.Contains('unregister-com-user') -and $fixture.State['com.x64'] -eq 'foreign.dll') 'Foreign COM path was removed.'
    }
    if ($scenario -in @('dirty-user-state', 'missing-machine-profile')) {
        Assert-True ($fixture.Calls.Count -eq 1 -and $fixture.BodyCalls -eq 0) 'Preflight failure mutated state or ran the body.'
    }
    $caseCount++
}
Write-Host "$caseCount in-memory registered-test parser/transaction scenarios passed; no Windows input state was modified."
