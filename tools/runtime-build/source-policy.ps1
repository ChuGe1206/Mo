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

function Assert-MoLuaMachineDataPolicy([string]$SourcePath) {
    $text = Get-Content -LiteralPath $SourcePath -Raw
    $match = [regex]::Match($text, '(?s)static void lua_init\(lua_State \*L\) \{(?<body>.*?)\r?\n\}\r?\n\r?\nstatic void rime_lua_initialize')
    if (-not $match.Success) { throw 'Lua initialization policy function is missing or ambiguous.' }
    $body = $match.Groups['body'].Value
    foreach ($required in @(
        'const auto shared_dir = COMPAT<rime::Deployer>::get_shared_data_dir();',
        'lua_setfield(L, -2, "path");',
        'lua_pushliteral(L, "");',
        'lua_setfield(L, -2, "cpath");',
        'const auto shared_file = shared_dir + LUA_DIRSEP "rime.lua";'
    )) {
        if ($body.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw 'Lua machine-data policy is incomplete.' }
    }
    foreach ($forbidden in @('user_dir', 'get_user_data_dir', 'lua_getfield(L, -2, "path")', 'lua_concat(L, 2)')) {
        if ($body.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) { throw 'Lua initialization retains a user/environment search path.' }
    }
    if ([regex]::Matches($body, 'luaL_dofile\(').Count -ne 1) { throw 'Lua initialization must have exactly one machine rime.lua entry point.' }
}
