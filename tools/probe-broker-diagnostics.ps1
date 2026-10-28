# Loading this helper has no side effects. The caller must pass its own child,
# with stderr already being drained asynchronously; never pass a discovered PID.
function Complete-MoOwnedProbeBrokerDiagnostics {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)][Threading.Tasks.Task[string]]$Log,
        [switch]$DiagnosticGrace
    )

    # Harness cleanup budgets only: the TIP's 50/400 ms deadlines are unchanged.
    $diagnosticGraceMilliseconds = 1000
    $childExitTimeoutMilliseconds = 3000
    $logDrainTimeoutMilliseconds = 1000
    $maximumTranscriptCharacters = 65536

    if ($DiagnosticGrace -and -not $Process.HasExited) {
        # A failed native request may still be executing in the Actor. Killing
        # immediately can cut the asynchronous metadata logger mid-record.
        # Wait once, after the probe has returned, then stop our exact child.
        $null = $Process.WaitForExit($diagnosticGraceMilliseconds)
    }
    if (-not $Process.HasExited) { $Process.Kill($true) }
    if (-not $Process.WaitForExit($childExitTimeoutMilliseconds)) {
        throw 'Owned probe Broker did not exit within the cleanup budget.'
    }
    if (-not $Log.Wait($logDrainTimeoutMilliseconds)) {
        throw 'Owned probe Broker stderr did not close within the drain budget.'
    }
    $transcript = $Log.GetAwaiter().GetResult()
    if ($transcript.Length -gt $maximumTranscriptCharacters) {
        return '[earlier diagnostics omitted]' + "`n" +
            $transcript.Substring($transcript.Length - $maximumTranscriptCharacters)
    }
    return $transcript
}
