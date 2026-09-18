[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $repoRoot ('build/mo-resource-file-墨-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$links = @((Join-Path $fixture 'root-link'), (Join-Path $fixture 'opencc/linked.json'))
try {
    New-Item -ItemType File -Path (Join-Path $fixture 'mo-resource-file-fixture') | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $fixture 'opencc'), (Join-Path $fixture 'target') | Out-Null
    New-Item -ItemType Junction -Path $links[0] -Target (Join-Path $fixture 'opencc') | Out-Null
    New-Item -ItemType Junction -Path $links[1] -Target (Join-Path $fixture 'target') | Out-Null
    $vswhere = Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio/Installer/vswhere.exe'
    $visualStudio = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($LASTEXITCODE -ne 0 -or -not $visualStudio) { throw 'MSVC toolchain not found.' }
    $dev = Join-Path $visualStudio 'Common7/Tools/VsDevCmd.bat'
    $native = Join-Path $repoRoot 'native/librime/preparation'
    $probe = Join-Path $fixture 'resource-file-probe.exe'
    $compile = "`"$dev`" -no_logo -arch=x64 -host_arch=x64 >nul && cl.exe /nologo /std:c++17 /utf-8 /MT /EHsc /W4 /WX /sdl /GS /guard:cf /I`"$native`" `"$PSScriptRoot/resource-file-probe.cpp`" `"$native/mo_resource_file.cpp`" /Fo:`"$fixture\\`" /Fe:`"$probe`" /link /guard:cf /DYNAMICBASE /NXCOMPAT"
    & $env:ComSpec /d /s /c $compile
    if ($LASTEXITCODE -ne 0) { throw 'Native resource boundary probe build failed.' }
    & $probe $fixture
    if ($LASTEXITCODE -ne 0) { throw 'Native resource boundary probe failed.' }
} finally {
    $resolved = (Resolve-Path -LiteralPath $fixture).Path
    if (-not $resolved.StartsWith((Join-Path $repoRoot 'build').TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Refusing resource probe cleanup outside repository build.'
    }
    # Remove only the links themselves before recursively cleaning this new fixture.
    foreach ($link in $links) {
        if (Test-Path -LiteralPath $link) {
            $entry = Get-Item -LiteralPath $link -Force
            if (-not ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Fixture junction changed type.' }
            Remove-Item -LiteralPath $link -Force
        }
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
