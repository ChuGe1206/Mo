#Requires -Version 7.0
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'probe-broker-diagnostics.ps1')
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('mo-probe-diagnostics-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null
$checks = 0
function Require([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
    $script:checks++
}
function Invoke-Fixture([string]$Name, [string]$Body, [bool]$Grace) {
    $path = Join-Path $testRoot ($Name + '.ps1')
    # All output is synthetic diagnostic metadata, never user input.
    $Body | Set-Content -LiteralPath $path -Encoding utf8NoBOM
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = Join-Path $PSHOME 'pwsh.exe'
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardError = $true
    foreach ($argument in @('-NoProfile', '-NonInteractive', '-File', $path)) {
        [void]$info.ArgumentList.Add($argument)
    }
    $child = [Diagnostics.Process]::Start($info)
    try {
        $ready = $child.StandardError.ReadLineAsync()
        Require ($ready.Wait(10000) -and $ready.Result -ceq 'fixture-ready') 'Fixture readiness failed.'
        $log = $child.StandardError.ReadToEndAsync()
        $timer = [Diagnostics.Stopwatch]::StartNew()
        $transcript = Complete-MoOwnedProbeBrokerDiagnostics -Process $child -Log $log -DiagnosticGrace:$Grace
        $timer.Stop()
        Require $child.HasExited 'Owned child survived cleanup.'
        Require $log.IsCompletedSuccessfully 'Stderr was not drained.'
        Require ($timer.ElapsedMilliseconds -lt 6000) 'Cleanup exceeded its total budget plus scheduling allowance.'
        return $transcript
    } finally {
        if (-not $child.HasExited) { $child.Kill($true); $null = $child.WaitForExit(3000) }
        $child.Dispose()
    }
}
try {
    $delayed = Invoke-Fixture 'late-line' @'
[Console]::Error.WriteLine('fixture-ready')
Start-Sleep -Milliseconds 120
[Console]::Error.Write('MO_LATENCY op=')
Start-Sleep -Milliseconds 80
[Console]::Error.WriteLine('dispatch queue_us=1 engine_us=2 dropped=0')
exit 7
'@ $true
    Require ($delayed.Trim() -ceq 'MO_LATENCY op=dispatch queue_us=1 engine_us=2 dropped=0') 'Late diagnostic line was lost or cut short.'

    $hanging = Invoke-Fixture 'late-then-hang' @'
[Console]::Error.WriteLine('fixture-ready')
Start-Sleep -Milliseconds 120
[Console]::Error.WriteLine('MO_LATENCY op=dispatch queue_us=3 engine_us=4 dropped=0')
Start-Sleep -Seconds 60
'@ $true
    Require ($hanging.Contains('engine_us=4')) 'Live child diagnostic was lost before bounded termination.'

    $immediate = Invoke-Fixture 'no-grace' @'
[Console]::Error.WriteLine('fixture-ready')
Start-Sleep -Seconds 60
[Console]::Error.WriteLine('unexpected-late-output')
'@ $false
    Require (-not $immediate.Contains('unexpected-late-output')) 'Non-trace cleanup waited for late work.'

    $large = Invoke-Fixture 'bounded-transcript' @'
[Console]::Error.WriteLine('fixture-ready')
[Console]::Error.Write('x' * 100000)
[Console]::Error.WriteLine('diagnostic-tail')
'@ $true
    Require ($large.StartsWith('[earlier diagnostics omitted]')) 'Oversized transcript was not marked.'
    Require ($large.TrimEnd().EndsWith('diagnostic-tail')) 'Transcript tail was not retained.'
    Require ($large.Length -le 65600) 'Retained transcript exceeded its bound.'

    # Execute the production smoke function without its build/deployment entry
    # point. Synthetic executables exercise its actual failure/cleanup ordering.
    $smokePath = Join-Path $PSScriptRoot 'tip-rime-smoke.ps1'
    $tokens = $null; $parseErrors = $null
    $ast = [Management.Automation.Language.Parser]::ParseFile($smokePath, [ref]$tokens, [ref]$parseErrors)
    Require ($parseErrors.Count -eq 0) 'Smoke script parse failed.'
    foreach ($name in @('Remove-ProbeUser', 'Invoke-RimeBrokerProbe')) {
        $function = $ast.FindAll({ param($node)
            $node -is [Management.Automation.Language.FunctionDefinitionAst]
        }, $true) | Where-Object Name -CEQ $name
        Require (@($function).Count -eq 1) "Production function missing: $name"
        . ([scriptblock]::Create($function.Extent.Text))
    }
    $user = Join-Path $testRoot 'users'
    New-Item -ItemType Directory -Path (Join-Path $user 'build') -Force | Out-Null
    $brokerPath = Join-Path $PSHOME 'pwsh.exe'
    $rimeMode = '-File'
    $dynamicLibrary = Join-Path $testRoot 'broker.ps1'
    $shared = 'synthetic-shared'
    $OpenccDataDir = $null
    $LatencyTrace = $true
    @'
[Console]::Error.WriteLine('Mo broker listening on 16 protected pipe slots')
Start-Sleep -Milliseconds 500
[Console]::Error.WriteLine('MO_LATENCY op=dispatch queue_us=5 engine_us=6 dropped=0')
Start-Sleep -Seconds 60
'@ | Set-Content -LiteralPath $dynamicLibrary -Encoding utf8NoBOM
    $probeScript = Join-Path $testRoot 'failing-probe.ps1'
    'exit 7' | Set-Content -LiteralPath $probeScript -Encoding utf8NoBOM
    $failure = $null
    try {
        Invoke-RimeBrokerProbe 'fixture' $brokerPath @('-NoProfile', '-File', $probeScript) 'synthetic probe'
    } catch { $failure = $_.Exception.Message }
    Require ($null -ne $failure -and $failure.Contains('fixture synthetic probe failed: 7')) 'Original probe failure was lost.'
    Require ($failure.Contains('MO_LATENCY op=dispatch queue_us=5 engine_us=6 dropped=0')) 'Production smoke discarded late Broker diagnostics.'
    Require (@(Get-ChildItem -LiteralPath $user -Directory -Filter 'mo-smoke-*').Count -eq 0) 'Production cleanup left disposable probe data.'

    function Remove-ProbeUser([string]$ProbeUser) { throw 'synthetic cleanup failure' }
    $failure = $null
    $cleanupTranscript = & {
        try {
            Invoke-RimeBrokerProbe 'fixture' $brokerPath @('-NoProfile', '-File', $probeScript) 'synthetic probe'
        } catch { $script:cleanupCaseFailure = $_.Exception.Message }
    } 6>&1 | Out-String
    Require ($script:cleanupCaseFailure.Contains('fixture synthetic probe failed: 7')) 'Cleanup failure replaced the original probe exception.'
    Require (-not $script:cleanupCaseFailure.Contains('synthetic cleanup failure')) 'Original exception was replaced by finally cleanup.'
    Require ($cleanupTranscript.Contains('MO_RIME_PROBE_CLEANUP_FAILURE') -and
        $cleanupTranscript.Contains('synthetic cleanup failure')) 'Secondary cleanup failure was not retained.'

    'exit 0' | Set-Content -LiteralPath $probeScript -Encoding utf8NoBOM
    $cleanupOnlyFailure = $null
    try {
        Invoke-RimeBrokerProbe 'fixture' $brokerPath @('-NoProfile', '-File', $probeScript) 'synthetic probe'
    } catch { $cleanupOnlyFailure = $_.Exception.Message }
    Require ($null -ne $cleanupOnlyFailure) 'Successful probe hid a cleanup failure.'
    Require ($cleanupOnlyFailure.Contains('synthetic cleanup failure')) 'Cleanup-only failure lost its cause.'
    Write-Host "Owned probe diagnostic checks passed: $checks"
} finally {
    $resolved = [IO.Path]::GetFullPath($testRoot)
    $tempPrefix = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (-not $resolved.StartsWith($tempPrefix, [StringComparison]::OrdinalIgnoreCase) -or
        (Get-Item -LiteralPath $resolved).Attributes -band [IO.FileAttributes]::ReparsePoint -or
        @(Get-ChildItem -LiteralPath $resolved -Recurse -Force | Where-Object {
            $_.Attributes -band [IO.FileAttributes]::ReparsePoint
        }).Count -ne 0) { throw 'Unsafe fixture cleanup path.' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
# The intentionally failing synthetic probe sets LASTEXITCODE to 7. Report
# this test script's success only after all assertions and cleanup complete.
exit 0
