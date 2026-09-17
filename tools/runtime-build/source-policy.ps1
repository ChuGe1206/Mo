function Assert-MoRuntimeSourcePaths([string]$CachePath, [hashtable]$Expected) {
    $lines = Get-Content -LiteralPath $CachePath
    foreach ($key in $Expected.Keys) {
        $pattern = '^' + [regex]::Escape($key) + ':[A-Z_]+=(.+)$'
        $records = @($lines | Where-Object { $_ -cmatch $pattern })
        if ($records.Count -ne 1) { throw 'Runtime source path missing or duplicated in CMake cache.' }
        [void]($records[0] -cmatch $pattern)
        $actual = [IO.Path]::GetFullPath($Matches[1])
        if (-not [StringComparer]::OrdinalIgnoreCase.Equals($actual, [IO.Path]::GetFullPath($Expected[$key]))) {
            throw 'Runtime source path selected a foreign header or library.'
        }
    }
}
