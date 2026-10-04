[CmdletBinding()]
param([Parameter(Mandatory)][string]$RuntimeBuildDirectory, [string]$LegacyRuntimeBuildDirectory)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $PSScriptRoot 'source-policy.ps1')
. (Join-Path $repo 'installer/windows/staging-policy.ps1')
$runtime = (Resolve-Path -LiteralPath $RuntimeBuildDirectory).Path
$source = Join-Path $runtime 'inputs/librime'
Assert-MoUserDbStartupPolicy $source
$null = Assert-MoStageRuntime $runtime $repo
if ($LegacyRuntimeBuildDirectory) {
    $message = $null
    try { $null = Assert-MoStageRuntime $LegacyRuntimeBuildDirectory $repo } catch { $message = $_.Exception.Message }
    if ($message -notlike '*Runtime ABI/plugin policy mismatch*') { throw 'Old ABI runtime was not rejected at the contract gate.' }
    Write-Host 'Old ABI runtime rejected before source snapshot checks.'
}
$fixture = Join-Path $repo ('build/mo-userdb-source-policy-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$mutations = @(
    @('src/rime/dict/level_db.cc', 'options.paranoid_checks = true;', 'options.paranoid_checks = false;'),
    @('src/rime/dict/user_dictionary.cc', 'bool UserDictionary::Load() {', ('bool UserDictionary::Load() {' + [char]10 + '  deployer.ScheduleTask(task);')),
    @('src/rime/dict/user_dictionary.cc', 'bool UserDictionary::Load() {', ('bool UserDictionary::Load() {' + [char]10 + '  deployer.StartWork();')),
    @('src/rime/gear/memory.cc', 'user_dict_ready_ = user_dict_->Load();', 'user_dict_->Load();'),
    @('src/rime/gear/memory.cc', 'user_dict_ && user_dict_ready_ && user_dict_->loaded()', 'user_dict_ && user_dict_->loaded()'),
    @('src/rime/engine.cc', 'if (!memory->UserDictionaryReady()) { return false; }', ''),
    @('src/rime/engine.cc', 'main_user_dictionaries != 1', 'main_user_dictionaries == 0'),
    @('src/rime/engine.cc', 'user_dict->name() == "rime_ice"', 'user_dict->name() == "optional"')
)
try {
    foreach ($name in @($mutations | ForEach-Object { $_[0] } | Select-Object -Unique)) {
        $path = Join-Path $fixture $name
        New-Item -ItemType Directory -Path (Split-Path -Parent $path) -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $source $name) -Destination $path
    }
    Assert-MoUserDbStartupPolicy $fixture
    foreach ($mutation in $mutations) {
        $path = Join-Path $fixture $mutation[0]
        $bytes = [IO.File]::ReadAllBytes($path)
        $text = [IO.File]::ReadAllText($path)
        if (-not $text.Contains($mutation[1], [StringComparison]::Ordinal)) { throw 'Mutation source missing.' }
        [IO.File]::WriteAllText($path, $text.Replace($mutation[1], $mutation[2]), [Text.UTF8Encoding]::new($false))
        $rejected = $false
        try { Assert-MoUserDbStartupPolicy $fixture } catch { $rejected = $true }
        [IO.File]::WriteAllBytes($path, $bytes)
        if (-not $rejected) { throw 'User DB source policy accepted a regressed contract.' }
    }
    Write-Host 'User DB source policy: positive fixture and eight regressions passed.'
} finally {
    $resolved = (Resolve-Path -LiteralPath $fixture).Path
    if (-not $resolved.StartsWith((Join-Path $repo 'build').TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe fixture cleanup.' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
