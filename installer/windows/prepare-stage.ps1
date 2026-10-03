#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$RuntimeBuildDirectory,
    [string]$RimeIceSourceDir,
    [string]$RimeIceArchivePath,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidatePattern('^[A-Za-z0-9._-]+$')][string]$RustToolchain = 'stable',
    [switch]$DevelopmentFaultInjection
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$output = Assert-MoNewBuildOutput $OutputDirectory $repo
# The sole cmd.exe invocation below establishes the MSVC environment. Percent
# expansion occurs even inside quotes, and a developer shell may enable delayed
# expansion. Refuse such path characters rather than interpolate them into cmd.
if ($output -match '[%!\"]') { throw 'Native compiler path contains unsupported shell expansion characters.' }
$runtimeRoot = Assert-MoPlainPath $RuntimeBuildDirectory
$runtime = Assert-MoStageRuntime $runtimeRoot $repo
$iceCommit = '6810e8916d160498620a16fef2135956fecbd485'
$iceArchiveHash = 'CD1895FBC961131A62F23277F636C27A6FB941DAC66DAF43C4A10D4E9E6ADAD3'
$iceInput = Resolve-MoPinnedSourceInput $RimeIceSourceDir $RimeIceArchivePath $iceArchiveHash
$hasIceSource = $iceInput.Kind -ceq 'Checkout'
$source = $null
$sourceArchive = $null
if ($hasIceSource) {
    $source = $iceInput.Path
    $head = & git -c "safe.directory=$source" -C $source rev-parse HEAD
    if ($LASTEXITCODE -ne 0 -or $head -cne $iceCommit) { throw 'Rime Ice source commit mismatch.' }
} else {
    $sourceArchive = $iceInput.Path
}
# Reject compiler/profile injection in the build environment. Tool installations,
# Cargo home and OS remain trusted build-host prerequisites, not a hermetic sandbox.
foreach ($variable in Get-ChildItem Env:) {
    if ($variable.Name -match '^(RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|RUSTC|RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|CARGO_PROFILE_.*|CARGO_BUILD_RUSTFLAGS|CARGO_TARGET_.*_RUSTFLAGS|CL|_CL_|LINK|_LINK_)$') {
        throw "Unset build override before staging: $($variable.Name)"
    }
}
$vswhere = Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio/Installer/vswhere.exe'
$visualStudio = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -ne 0 -or -not $visualStudio) { throw 'Visual Studio C++ toolchain not found.' }
if ($visualStudio -match '[%!\"]') { throw 'Native compiler path contains unsupported shell expansion characters.' }
$msbuild = Join-Path $visualStudio 'MSBuild/Current/Bin/MSBuild.exe'
if (-not (Test-Path -LiteralPath $msbuild -PathType Leaf)) { throw 'MSBuild not found.' }
$previousAutoInstall = [Environment]::GetEnvironmentVariable('RUSTUP_AUTO_INSTALL', 'Process')
try {
    $env:RUSTUP_AUTO_INSTALL = '0'
    $installedToolchains = & rustup toolchain list
} finally { [Environment]::SetEnvironmentVariable('RUSTUP_AUTO_INSTALL', $previousAutoInstall, 'Process') }
if ($LASTEXITCODE -ne 0 -or -not @($installedToolchains | Where-Object {
    $_ -match ('^' + [regex]::Escape($RustToolchain) + '(-x86_64-pc-windows-msvc)?(\s|$)')
}).Count) { throw 'Explicit Rust toolchain is not installed; refusing automatic download.' }
& cargo "+$RustToolchain" --version
if ($LASTEXITCODE -ne 0) { throw 'Explicit Rust toolchain unavailable; no automatic download performed.' }
$rustVersion = & rustc "+$RustToolchain" --version
if ($LASTEXITCODE -ne 0 -or $rustVersion -cnotmatch '^rustc 1\.97\.1 ') { throw 'Staging requires the pinned Rust 1.97.1 compiler.' }
# Same process-only PATH normalization used by the native build probes.
$processPath = [Environment]::GetEnvironmentVariable('PATH', 'Process')
Remove-Item Env:Path -ErrorAction SilentlyContinue
$env:Path = $processPath
$working = Join-Path $output 'working'
$stage = Join-Path $output 'stage'
$payload = Join-Path $stage 'payload/Mo'
$evidence = Join-Path $stage 'evidence'
$moSource = Join-Path $working 'mo-source'
$iceSource = Join-Path $working 'rime-ice-source'
New-Item -ItemType Directory -Path $moSource, $iceSource, $evidence, $payload | Out-Null
$script:stageCommandCount = 0
function Checked([string]$Program, [string[]]$Arguments) {
    $script:stageCommandCount++
    $log = Join-Path $working ('command-{0:D2}.log' -f $script:stageCommandCount)
    & $Program @Arguments *>&1 | Tee-Object -FilePath $log | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -ne 0) { throw "Staging command failed ($LASTEXITCODE); log: $log" }
}
function Copy-StageFile([string]$InputPath, [string]$Destination) {
    $inputFull = Assert-MoPlainPath $InputPath
    if (Test-Path -LiteralPath $Destination) { throw 'Refusing to replace a staged file.' }
    New-Item -ItemType Directory -Path (Split-Path -Parent $Destination) -Force | Out-Null
    Copy-Item -LiteralPath $inputFull -Destination $Destination
    if ((Get-FileHash -LiteralPath $inputFull).Hash -ine (Get-FileHash -LiteralPath $Destination).Hash) { throw 'Staging copy changed bytes.' }
}
# Snapshot sources before any compilation; no stale object reuse and no arbitrary
# prebuilt Broker/TIP parameters. Only explicit files/trees enter this snapshot.
foreach ($name in @('Cargo.toml', 'Cargo.lock')) { Copy-StageFile (Join-Path $repo $name) (Join-Path $moSource $name) }
foreach ($name in Get-MoStageFiles (Join-Path $repo 'crates')) {
    if ($name -cnotmatch '\.(rs|toml|md)$') { throw 'Unexpected Cargo source snapshot file.' }
    Copy-StageFile (Join-Path $repo "crates/$name") (Join-Path $moSource "crates/$name")
}
$nativeSource = Join-Path $moSource 'native/windows-tip'
foreach ($directory in @('src', 'include', 'probe')) {
    foreach ($name in Get-MoStageFiles (Join-Path $repo "native/windows-tip/$directory")) {
        Copy-StageFile (Join-Path $repo "native/windows-tip/$directory/$name") (Join-Path $nativeSource "$directory/$name")
    }
}
foreach ($name in @('MoTip.vcxproj', 'MoTipRegistrar.vcxproj', 'MoTipAbiProbe.vcxproj',
        'MoStackSymbolResolver.vcxproj')) {
    Copy-StageFile (Join-Path $repo "native/windows-tip/$name") (Join-Path $nativeSource $name)
}
foreach ($name in @('prepare-stage.ps1', 'staging-policy.ps1', 'deploy-data.cpp')) {
    Copy-StageFile (Join-Path $PSScriptRoot $name) (Join-Path $moSource "installer/windows/$name")
}
$moSourceInventory = Get-MoStageInventory $moSource
$stageIceArchive = Join-Path $evidence 'rime-ice-source.tar'
if ($hasIceSource) {
    Checked 'git' @('-c', "safe.directory=$source", '-C', $source, 'archive', '--format=tar', $iceCommit, '-o', $stageIceArchive)
} else {
    Copy-StageFile $sourceArchive $stageIceArchive
}
if ((Get-FileHash -LiteralPath $stageIceArchive).Hash -ine $iceArchiveHash) { throw 'Rime Ice source archive mismatch.' }
Checked 'tar' @('-xf', $stageIceArchive, '-C', $iceSource)
# Do not consume ignored build/, mutable working files, upstream platform skins,
# updater recipes, .git files or user dictionaries. All selected data is from tar.
$shared = Join-Path $payload 'data/rime-ice'
$rootDataNames = @('default.yaml', 'rime_ice.dict.yaml', 'rime_ice.schema.yaml', 'melt_eng.dict.yaml', 'melt_eng.schema.yaml',
    'radical_pinyin.dict.yaml', 'radical_pinyin.schema.yaml', 'symbols_v.yaml', 'symbols_caps_v.yaml', 'custom_phrase.txt',
    'double_pinyin.schema.yaml', 'double_pinyin_abc.schema.yaml', 'double_pinyin_flypy.schema.yaml',
    'double_pinyin_jiajia.schema.yaml', 'double_pinyin_mspy.schema.yaml', 'double_pinyin_sogou.schema.yaml',
    'double_pinyin_ziguang.schema.yaml', 't9.schema.yaml')
foreach ($name in $rootDataNames) { Copy-StageFile (Join-Path $iceSource $name) (Join-Path $shared $name) }
foreach ($directory in @('cn_dicts', 'en_dicts', 'lua')) {
    foreach ($name in Get-MoStageFiles (Join-Path $iceSource $directory)) {
        if (($directory -eq 'cn_dicts' -and $name -cnotmatch '^[A-Za-z0-9_.-]+\.yaml$') -or
            ($directory -eq 'en_dicts' -and $name -cnotmatch '^[A-Za-z0-9_.-]+\.(yaml|txt)$') -or
            ($directory -eq 'lua' -and $name -cnotmatch '^[A-Za-z0-9_./-]+\.(lua|db)$')) { throw 'Unexpected locked source data file.' }
        Copy-StageFile (Join-Path $iceSource "$directory/$name") (Join-Path $shared "$directory/$name")
    }
}
Copy-StageFile (Join-Path $iceSource 'LICENSE') (Join-Path $evidence 'rime-ice-LICENSE')
Copy-StageFile (Join-Path $iceSource 'others/docs/Credits.md') (Join-Path $evidence 'rime-ice-Credits.md')
Copy-StageFile (Join-Path $runtimeRoot 'dist/mo-build-provenance.json') (Join-Path $evidence 'runtime-provenance.json')
Copy-StageFile (Join-Path $runtimeRoot 'dist/lib/rime.dll') (Join-Path $payload 'runtime/librime/rime.dll')
foreach ($name in Get-MoRuntimeResourceNames) {
    Copy-StageFile (Join-Path $runtimeRoot "dist/lib/opencc/$name") (Join-Path $payload "runtime/librime/opencc/$name")
}
$headerDirectory = Join-Path $working 'rime-include'
Copy-StageFile (Join-Path $runtimeRoot 'dist/include/rime_api.h') (Join-Path $headerDirectory 'rime_api.h')
$deployer = Join-Path $working 'mo-deploy-data.exe'
$dev = Join-Path $visualStudio 'Common7/Tools/VsDevCmd.bat'
$compile = "`"$dev`" -no_logo -arch=x64 -host_arch=x64 >nul && cl.exe /nologo /std:c++17 /utf-8 /MT /EHsc /W4 /WX /sdl /GS /guard:cf /I`"$headerDirectory`" `"$moSource/installer/windows/deploy-data.cpp`" /Fo:`"$working\\`" /Fe:`"$deployer`" /link /guard:cf /DYNAMICBASE /NXCOMPAT"
Checked $env:ComSpec @('/d', '/s', '/c', $compile)
$deployUser = Join-Path $working 'deploy-user'
New-Item -ItemType Directory -Path $deployUser | Out-Null
New-Item -ItemType File -Path (Join-Path $deployUser 'mo-data-build-fixture') | Out-Null
$dataSources = Get-MoStageInventory $shared
Push-Location $working
try { Checked $deployer @((Join-Path $payload 'runtime/librime/rime.dll'), $shared, $deployUser, (Join-Path $shared 'build')) }
finally { Pop-Location }
Assert-MoPrebuiltData (Join-Path $shared 'build')
[ordered]@{ format = 1; development_only = $true; redistributable = $false;
    rime_ice_commit = $iceCommit; source_archive_sha256 = $iceArchiveHash;
    runtime_dll_sha256 = $runtime['dll_sha256']; sources = $dataSources;
    outputs = (Get-MoStageInventory $shared); deployer_sha256 = (Get-FileHash -LiteralPath $deployer).Hash
} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $evidence 'rime-data.json') -Encoding utf8NoBOM
# Force release debug-assertions off, explicit target, no features, offline and
# locked. Fresh target-dir means the stage cannot reuse prior diagnostic objects.
Push-Location $moSource
try {
    Checked 'cargo' @("+$RustToolchain", 'build', '--locked', '--offline', '--release', '--no-default-features',
        '--target', 'x86_64-pc-windows-msvc', '--target-dir', (Join-Path $working 'rust-target'),
        '--config', 'build.rustflags=["-C","target-feature=+crt-static"]',
        '--config', 'profile.release.debug-assertions=false',
        '-p', 'mo-broker', '--bin', 'mo-broker',
        '-p', 'mo-settings-app', '--bin', 'mo-settings')
} finally { Pop-Location }
$broker = Join-Path $working 'rust-target/x86_64-pc-windows-msvc/release/mo-broker.exe'
$settings = Join-Path $working 'rust-target/x86_64-pc-windows-msvc/release/mo-settings.exe'
# Independent binary check, not just trusting the build command/receipt.
$start = [Diagnostics.ProcessStartInfo]::new()
$start.FileName = $broker; $start.UseShellExecute = $false; $start.CreateNoWindow = $true
$start.RedirectStandardError = $true; [void]$start.ArgumentList.Add('--fake')
$process = [Diagnostics.Process]::Start($start)
try {
    $errorTask = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(5000) -or $process.ExitCode -eq 0 -or
        -not $errorTask.GetAwaiter().GetResult().Contains('installed mo-broker accepts no command-line arguments', [StringComparison]::Ordinal)) {
        throw 'Fresh Broker did not reject diagnostic startup.'
    }
} finally { if (-not $process.HasExited) { $process.Kill($true) }; $process.Dispose() }
Copy-StageFile $broker (Join-Path $payload 'bin/mo-broker.exe')
Copy-StageFile $settings (Join-Path $payload 'bin/mo-settings.exe')
foreach ($platform in @('x64', 'Win32')) {
    $nativeOut = Join-Path $working "native/$platform"
    foreach ($project in @('MoTip', 'MoTipAbiProbe', 'MoTipRegistrar')) {
        $objectOut = Join-Path $working "native-obj/$platform/$project"
        $faultOption = if ($DevelopmentFaultInjection) { 'true' } else { 'false' }
        Checked $msbuild @((Join-Path $nativeSource "$project.vcxproj"), '/nologo', '/t:Rebuild', '/m',
            '/p:Configuration=Release', "/p:Platform=$platform", '/p:MoLatencyTrace=false',
            "/p:MoDevelopmentFaultInjection=$faultOption",
            "/p:OutDir=$nativeOut\", "/p:IntDir=$objectOut\")
    }
    # Default probe explicitly checks the diagnostics IID returns E_NOINTERFACE.
    Checked (Join-Path $nativeOut 'mo_tip_abi_probe.exe') @((Join-Path $nativeOut 'mo_tip.dll'))
    $transactionMarker = Join-Path $working "native/$platform/machine-profile-transaction.marker"
    Checked (Join-Path $nativeOut 'mo_tip_registrar.exe') @('self-test-machine-transaction', $transactionMarker)
    if (Test-Path -LiteralPath $transactionMarker) { throw 'Registrar transaction self-test left a marker.' }
    Checked (Join-Path $nativeOut 'mo_tip_registrar.exe') @('self-test-user-finalizer-policy')
    $builtRegistrar = Join-Path $nativeOut 'mo_tip_registrar.exe'
    $beforeFaultProbe = @(& $builtRegistrar status)
    if ($LASTEXITCODE -ne 0) { throw 'Staged registrar failure-injection preflight status failed.' }
    $faultProbe = @(& $builtRegistrar development-test-fail-fixed 2>&1)
    $faultExitCode = $LASTEXITCODE
    $faultText = $faultProbe -join "`n"
    if ($DevelopmentFaultInjection) {
        if ($faultExitCode -eq 0 -or $faultText -cnotmatch '(?m)^Operation failed: 0x80004005$' -or
            $faultText -match '(?m)^Usage:$') {
            throw 'Staged registrar development fault command mismatch.'
        }
    } elseif ($faultExitCode -eq 0 -or $faultText -cnotmatch '(?m)^Usage:$' -or
        $faultText -cnotmatch '(?m)^Operation failed: 0x80070057$' -or
        $faultText -match '0x80004005') {
        throw 'Staged production-shape registrar exposes development fault injection.'
    }
    $afterFaultProbe = @(& $builtRegistrar status)
    if ($LASTEXITCODE -ne 0 -or ($beforeFaultProbe -join "`n") -cne ($afterFaultProbe -join "`n")) {
        throw 'Staged registrar flavor probe changed observable state.'
    }
    $architecture = if ($platform -eq 'x64') { 'x64' } else { 'x86' }
    Copy-StageFile (Join-Path $nativeOut 'mo_tip.dll') (Join-Path $payload "tip/$architecture/mo-tip.dll")
    if ($platform -eq 'x64') { Copy-StageFile (Join-Path $nativeOut 'mo_tip_registrar.exe') (Join-Path $payload 'bin/mo-tip-registrar.exe') }
}
# Ensure no input changed during compilation and no post-copy runtime changed.
Assert-MoInventory $moSource $moSourceInventory
$null = Assert-MoStageRuntime $runtimeRoot $repo
$images = [ordered]@{}
foreach ($name in @('bin/mo-broker.exe', 'bin/mo-settings.exe', 'bin/mo-tip-registrar.exe',
    'tip/x64/mo-tip.dll', 'tip/x86/mo-tip.dll')) {
    $images[$name] = (Get-FileHash -LiteralPath (Join-Path $payload $name)).Hash
}
$probeImages = [ordered]@{}
foreach ($platform in @('x64', 'Win32')) {
    $probeImages[$platform] = (Get-FileHash -LiteralPath (Join-Path $working "native/$platform/mo_tip_abi_probe.exe")).Hash
}
[ordered]@{ format = 1; development_only = $true; redistributable = $false;
    rust_toolchain = $RustToolchain; rust_target = 'x86_64-pc-windows-msvc'; rust_profile = 'release';
    debug_assertions = $false; latency_trace = $false; native_platforms = @('x64', 'Win32');
    source_files = $moSourceInventory; images = $images; abi_probe_images = $probeImages
} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $evidence 'build-receipt.json') -Encoding utf8NoBOM
Assert-MoStagePayloadNames (Get-MoStageFiles $payload)
# Completion marker last; earlier failures leave no valid final stage. This is a
# development consistency manifest, never a signed release authorization.
[ordered]@{ format = 1; kind = 'mo-windows-development-stage'; development_only = $true;
    redistributable = $false; installable = $false; payload_root = 'payload/Mo';
    rime_ice_commit = $iceCommit; rime_ice_archive_sha256 = $iceArchiveHash;
    files = (Get-MoStageInventory $stage)
} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $stage 'mo-stage.pending.json') -Encoding utf8NoBOM
$null = Assert-MoPreparedStage $stage 'mo-stage.pending.json'
Move-Item -LiteralPath (Join-Path $stage 'mo-stage.pending.json') -Destination (Join-Path $stage 'mo-stage.json')
$null = Assert-MoPreparedStage $stage
Write-Host "Verified non-installable development stage: $stage"
Write-Warning 'No MSI/Setup produced. Signatures, full notices/SBOM and real installer transaction tests remain release gates.'
