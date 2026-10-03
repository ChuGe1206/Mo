# Loading this helper does not start/stop any process or change input state.
function Invoke-MoBrokerFaultProbe(
    [string]$BrokerPath,
    [string[]]$BrokerArguments,
    [string]$ProbePath,
    [string]$TipPath,
    [switch]$RimeIce,
    [switch]$LatencyTrace,
    [switch]$ActivatingTestHost
) {
    foreach ($artifact in @($BrokerPath, $ProbePath, $TipPath)) {
        if (-not [IO.Path]::IsPathFullyQualified($artifact) -or
            -not (Test-Path -LiteralPath $artifact -PathType Leaf)) {
            throw "Missing absolute fault-probe artifact: $artifact"
        }
    }

    function Start-OwnedFaultBroker {
        $info = [Diagnostics.ProcessStartInfo]::new()
        $info.FileName = $BrokerPath
        $info.UseShellExecute = $false
        $info.CreateNoWindow = $true
        $info.RedirectStandardError = $true
        foreach ($argument in $BrokerArguments) { [void]$info.ArgumentList.Add($argument) }
        $child = [Diagnostics.Process]::Start($info)
        if ($null -eq $child) { throw 'Could not start owned fault Broker.' }
        try {
            $ready = $child.StandardError.ReadLineAsync()
            if (-not $ready.Wait(30000) -or $ready.Result -notmatch '^Mo broker listening on 16 protected pipe slots$') {
                throw 'Fault Broker did not become ready on all protected slots.'
            }
            # Drain for the entire lifetime, including fault log messages.
            $log = $child.StandardError.ReadToEndAsync()
            return @{ Process = $child; Log = $log }
        } catch {
            if (-not $child.HasExited) { $child.Kill($true) }
            if (-not $child.WaitForExit(3000)) { throw 'Unready owned Broker failed to exit.' }
            $child.Dispose()
            throw
        }
    }

    function Stop-OwnedFaultChild([Diagnostics.Process]$Child) {
        if (-not $Child.HasExited) { $Child.Kill($true) }
        if (-not $Child.WaitForExit(3000)) { throw 'Owned fault child failed to exit within cleanup budget.' }
        $Child.Dispose()
    }

    $prefix = 'Local\Mo.Probe.Fault.' + [Guid]::NewGuid().ToString('N')
    $signals = @{}
    $owned = $null
    $probe = $null
    $stdout = $null
    try {
        foreach ($cycle in 0..1) {
            foreach ($stage in @('stop', 'restart')) {
                foreach ($suffix in @('', '.done')) {
                    $name = "$prefix.$stage.$cycle$suffix"
                    $created = $false
                    $event = [Threading.EventWaitHandle]::new(
                        $false, [Threading.EventResetMode]::ManualReset, $name, [ref]$created)
                    if (-not $created) { $event.Dispose(); throw 'Refusing to reuse a fault coordination event.' }
                    $signals["$stage.$cycle$suffix"] = $event
                }
            }
        }
        $owned = Start-OwnedFaultBroker
        $info = [Diagnostics.ProcessStartInfo]::new()
        $info.FileName = $ProbePath
        $info.UseShellExecute = $false
        $info.CreateNoWindow = $true
        $info.RedirectStandardOutput = $true
        $info.RedirectStandardError = $true
        [void]$info.ArgumentList.Add($TipPath)
        [void]$info.ArgumentList.Add($(if ($RimeIce) { '--broker-fault-rime-ice' } else { '--broker-fault' }))
        [void]$info.ArgumentList.Add($prefix)
        if ($ActivatingTestHost) { [void]$info.ArgumentList.Add('--activating-test-host') }
        $probe = [Diagnostics.Process]::Start($info)
        if ($null -eq $probe) { throw 'Could not start fault TIP probe.' }
        $stdout = $probe.StandardOutput.ReadToEndAsync()
        $stderr = $probe.StandardError.ReadToEndAsync()
        foreach ($cycle in 0..1) {
            foreach ($stage in @('stop', 'restart')) {
                $deadline = [DateTime]::UtcNow.AddSeconds(40)
                while (-not $signals["$stage.$cycle"].WaitOne(25)) {
                    if ($probe.HasExited) {
                        if (-not $stderr.Wait(1000)) { throw 'Fault probe stderr did not close.' }
                        throw "TIP exited before $stage cycle ${cycle}: $($stderr.Result)"
                    }
                    if ([DateTime]::UtcNow -ge $deadline) { throw "TIP fault signal timeout: $stage cycle $cycle" }
                }
                if ($stage -eq 'stop') {
                    if ($owned.Process.HasExited) { throw 'Broker exited before deliberate fault injection.' }
                    # Kill exactly the child created above, never a discovered PID.
                    Stop-OwnedFaultChild $owned.Process
                    if (-not $owned.Log.Wait(1000)) { throw 'Stopped Broker stderr did not close.' }
                    $owned = $null
                } else {
                    $owned = Start-OwnedFaultBroker
                }
                [void]$signals["$stage.$cycle.done"].Set()
            }
        }
        if (-not $probe.WaitForExit(10000)) { throw 'TIP fault probe completion timed out.' }
        if (-not $stdout.Wait(1000) -or -not $stderr.Wait(1000)) { throw 'TIP fault probe output did not close.' }
        if ($probe.ExitCode -ne 0) { throw "TIP fault probe failed: $($stderr.Result) $($stdout.Result)" }
        if ($owned.Process.HasExited) { throw 'Recovered Broker exited unexpectedly.' }
        if ($LatencyTrace) {
            # The probe's entire output is drained asynchronously above. Keep
            # successful repeated runs readable; failures retain full metadata.
            $keys = [regex]::Matches($stdout.Result, 'MO_CLIENT request=\d+ kind=5 phase=\d+ error=0 total_us=(\d+)')
            $keyTimes = @($keys | ForEach-Object { [long]$_.Groups[1].Value })
            if ($keyTimes.Count -eq 0) { throw 'Trace-enabled probe returned no key timings.' }
            $stats = $keyTimes | Measure-Object -Minimum -Maximum
            Write-Host "MO_KEY_TIMES count=$($keyTimes.Count) min_us=$($stats.Minimum) max_us=$($stats.Maximum)"
            Write-Host (($stdout.Result -split '\r?\n' | Where-Object { $_ -and -not $_.StartsWith('MO_CLIENT ') -and -not $_.StartsWith('MO_DISPATCH ') }) -join "`n")
        } else { Write-Host $stdout.Result.Trim() }
        # Successful trace probes include bounded owner-termination baselines.
        # Preserve them too, so a failure stack has a same-binary comparison.
        if (-not [string]::IsNullOrWhiteSpace($stderr.Result)) { Write-Host $stderr.Result.Trim() }
    } catch {
        $failureMessage = $_.Exception.Message
        # Preserve the bounded native probe transcript on failure. Previously
        # its drained stdout (including successful keys before a UI reset) was
        # discarded, leaving only the final generic visibility error.
        if ($null -ne $probe -and $probe.HasExited -and $null -ne $stdout -and $stdout.Wait(1000)) {
            $transcript = $stdout.Result
            if ($transcript.Length -gt 65536) { $transcript = $transcript.Substring($transcript.Length - 65536) }
            $failureMessage += "`nProbe stdout (last 65536 characters): $transcript"
        }
        if ($null -ne $owned) {
            try {
                Stop-OwnedFaultChild $owned.Process
                if ($owned.Log.Wait(1000)) { $failureMessage += "`nOwned Broker stderr: $($owned.Log.Result)" }
                $owned = $null
            } catch { $failureMessage += "`nBroker failure cleanup: $($_.Exception.Message)" }
        }
        # Persist failure evidence in redirected harness logs too: a terminating
        # error may otherwise be rendered only by the outer PowerShell host.
        Write-Host "MO_FAULT_FAILURE $failureMessage"
        throw $failureMessage
    } finally {
        # Cleanup tasks are independent: one failure must not leave another child.
        $cleanupErrors = @()
        if ($null -ne $probe) {
            try { Stop-OwnedFaultChild $probe } catch { $cleanupErrors += $_.Exception.Message }
        }
        if ($null -ne $owned) {
            try { Stop-OwnedFaultChild $owned.Process } catch { $cleanupErrors += $_.Exception.Message }
        }
        foreach ($event in $signals.Values) { $event.Dispose() }
        if ($cleanupErrors.Count -gt 0) { throw "Fault cleanup failed: $($cleanupErrors -join '; ')" }
    }
}
