function Assert-MoRuntimeStrictSources([string]$ProjectPath, [string[]]$Sources) {
    [xml]$project = Get-Content -LiteralPath $ProjectPath -Raw
    $entries = @($project.SelectNodes("//*[local-name()='ClCompile' and @Include]"))
    $condition = "'`$(Configuration)|`$(Platform)'=='Release|x64'"
    foreach ($source in $Sources) {
        $expected = [IO.Path]::GetFullPath($source)
        $records = @($entries | Where-Object {
            [StringComparer]::OrdinalIgnoreCase.Equals([IO.Path]::GetFullPath($_.GetAttribute('Include')), $expected)
        })
        if ($records.Count -ne 1) { throw 'Runtime strict source missing or duplicated in generated project.' }
        foreach ($property in @{ WarningLevel = 'Level4'; TreatWarningAsError = 'true' }.GetEnumerator()) {
            $nodes = @($records[0].SelectNodes("./*[local-name()='$($property.Key)']"))
            if ($nodes.Count -ne 1 -or $nodes[0].InnerText -cne $property.Value -or
                ($nodes[0].HasAttribute('Condition') -and $nodes[0].GetAttribute('Condition').Replace(' ', '') -cne $condition)) {
                throw 'Runtime strict source properties mismatch in generated project.'
            }
        }
    }
}

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
