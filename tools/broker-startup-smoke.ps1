[CmdletBinding()]
param(
    [ValidatePattern('^[A-Za-z0-9._-]+$')]
    [string]$RustToolchain = 'stable'
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    & cargo "+$RustToolchain" build --release -p mo-broker --bin mo-broker
    if ($LASTEXITCODE -ne 0) { throw "Release Broker build failed: $LASTEXITCODE" }
} finally {
    Pop-Location
}

$broker = Join-Path $repoRoot 'target\release\mo-broker.exe'

function Assert-Rejected([string[]]$Arguments, [string]$ExpectedError, [string]$Label) {
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $broker
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $Arguments) { [void]$startInfo.ArgumentList.Add($argument) }
    $process = [System.Diagnostics.Process]::Start($startInfo)
    if ($null -eq $process) { throw "Failed to start Broker for $Label" }
    try {
        $errorTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(5000)) { throw "$Label did not reject within 5 seconds" }
        if ($process.ExitCode -eq 0) { throw "$Label unexpectedly succeeded" }
        $errorText = $errorTask.GetAwaiter().GetResult()
        if (-not $errorText.Contains($ExpectedError, [System.StringComparison]::Ordinal)) {
            throw "$Label returned an unexpected error: $errorText"
        }
        Write-Host "$Label rejected as expected."
    } finally {
        if (-not $process.HasExited) { $process.Kill($true) }
        $process.Dispose()
    }
}

Assert-Rejected @('--fake') 'installed mo-broker accepts no command-line arguments' 'Release fake mode'
Assert-Rejected @('--rime', 'C:\untrusted\rime.dll', 'C:\untrusted\shared', 'C:\untrusted\user') 'installed mo-broker accepts no command-line arguments' 'Release caller-selected runtime'
Assert-Rejected @('--installed') 'installed mo-broker accepts no command-line arguments' 'Release unknown argument'
Assert-Rejected @() 'Broker must run from its fixed installed path' 'Repository release image'
Write-Host 'Release Broker startup policy passed without installing files or modifying system/user registration.'
