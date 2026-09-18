[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$LibrimeDistDir,
    [Parameter(Mandatory)][string]$OfficialDistDir,
    [Parameter(Mandatory)][string]$SharedDataDir,
    [Parameter(Mandatory)][string]$UserDataDir,
    [Parameter(Mandatory)][string]$OpenccDataDir,
    [string]$LegacyDistDir,
    [string]$BrokerPath,
    [ValidatePattern('^[A-Za-z0-9._-]+$')][string]$RustToolchain = 'stable'
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $repoRoot 'tools/opencc-data.ps1')
$pack = Assert-MoCompiledOpenccData $OpenccDataDir
$shared = (Resolve-Path -LiteralPath $SharedDataDir).Path
$user = (Resolve-Path -LiteralPath $UserDataDir).Path
$preparedDll = (Resolve-Path -LiteralPath (Join-Path $LibrimeDistDir 'lib/rime.dll')).Path
$officialDll = (Resolve-Path -LiteralPath (Join-Path $OfficialDistDir 'lib/rime.dll')).Path
if ($BrokerPath) { $BrokerPath = (Resolve-Path -LiteralPath $BrokerPath).Path }
if ($LegacyDistDir) { $legacyDll = (Resolve-Path -LiteralPath (Join-Path $LegacyDistDir 'lib/rime.dll')).Path }
$cases = @('success', 'failure', 'missing')
if ($LegacyDistDir) { $cases += 'legacy' }
foreach ($case in $cases) {
    $fixture = Join-Path $user ('mo-preparation-墨-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $fixture | Out-Null
    try {
        Copy-Item -LiteralPath (Join-Path $user 'build') -Destination (Join-Path $fixture 'build') -Recurse
        Copy-MoCompiledOpenccData $pack $fixture
        $runtime = Join-Path $fixture 'runtime-搬迁'
        New-Item -ItemType Directory -Path $runtime | Out-Null
        Copy-Item -LiteralPath $preparedDll -Destination (Join-Path $runtime 'rime.dll')
        Copy-Item -LiteralPath (Join-Path $LibrimeDistDir 'lib/opencc') -Destination (Join-Path $runtime 'opencc') -Recurse
        New-Item -ItemType File -Path (Join-Path $fixture 'mo-preparation-fixture') | Out-Null
        if ($case -eq 'failure') {
            # Corrupt only a brand-new test copy, never the source/compiled pack.
            Move-Item -LiteralPath (Join-Path $runtime 'opencc/emoji.ocd2') -Destination (Join-Path $runtime 'opencc/emoji.ocd2.absent')
        }
        $dll = switch ($case) { 'missing' { $officialDll } 'legacy' { $legacyDll } default { Join-Path $runtime 'rime.dll' } }
        $outcome = if ($case -eq 'legacy') { 'missing' } else { $case }
        Push-Location $repoRoot
        try {
            & cargo "+$RustToolchain" run --quiet -p mo-rime --example preparation_probe -- $dll $shared $fixture $outcome
            if ($LASTEXITCODE -ne 0) { throw "Native preparation case failed: $case" }
        } finally { Pop-Location }
        if ($BrokerPath -and $case -ne 'success') {
            $startInfo = [Diagnostics.ProcessStartInfo]::new()
            $startInfo.FileName = $BrokerPath
            $startInfo.UseShellExecute = $false
            $startInfo.CreateNoWindow = $true
            $startInfo.RedirectStandardError = $true
            $startInfo.RedirectStandardOutput = $true
            foreach ($argument in @('--rime-prepared', $dll, $shared, $fixture)) {
                [void]$startInfo.ArgumentList.Add($argument)
            }
            $process = [Diagnostics.Process]::Start($startInfo)
            if ($null -eq $process) { throw 'Preparation failure Broker did not start.' }
            try {
                $stderr = $process.StandardError.ReadToEndAsync()
                $stdout = $process.StandardOutput.ReadToEndAsync()
                if (-not $process.WaitForExit(35000)) { throw 'Preparation failure did not stop Broker within startup budget.' }
                $logs = $stderr.GetAwaiter().GetResult() + $stdout.GetAwaiter().GetResult()
                if ($process.ExitCode -eq 0 -or -not $logs.Contains('mo_rime_prepare_resources_v2', [StringComparison]::Ordinal) -or $logs.Contains('Mo broker listening', [StringComparison]::Ordinal)) {
                    throw 'Broker failed for an unrelated reason or announced readiness after preparation failure.'
                }
                Write-Host "Broker $case preparation rejected before readiness."
            } finally {
                if (-not $process.HasExited) {
                    $process.Kill($true)
                    if (-not $process.WaitForExit(5000)) { throw 'Owned preparation Broker did not exit after cleanup.' }
                }
                $process.Dispose()
            }
        }
    } finally {
        $resolved = (Resolve-Path -LiteralPath $fixture).Path
        if (-not $resolved.StartsWith($user.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Refusing to clean a preparation fixture outside its supplied root.'
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
Write-Host "$($cases.Count) native preparation boundary cases passed; no Windows input registration changed."
