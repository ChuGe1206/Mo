#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$OutputDirectory,
    [string]$PackageDirectory,
    [switch]$AllowDownload
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'staging-policy.ps1')
. (Join-Path $PSScriptRoot 'wix-toolchain.ps1')

$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$output = Assert-MoNewBuildOutput $OutputDirectory $repo
$lock = Read-MoWixToolchainLock
if ($PackageDirectory -and $AllowDownload) {
    throw 'Choose either an existing package directory or explicit download, not both.'
}
if (-not $PackageDirectory -and -not $AllowDownload) {
    throw 'Provide -PackageDirectory or explicitly pass -AllowDownload.'
}

New-Item -ItemType Directory -Path $output | Out-Null
$packages = Join-Path $output 'packages'
New-Item -ItemType Directory -Path $packages | Out-Null
$sourcePackages = if ($PackageDirectory) {
    Assert-MoPlainPath $PackageDirectory
} else {
    $packages
}

foreach ($package in $lock.Data['packages']) {
    $name = "$($package['id']).4.0.6.nupkg"
    $source = Join-Path $sourcePackages $name
    $destination = Join-Path $packages $name
    if ($AllowDownload) {
        $url = "https://api.nuget.org/v3-flatcontainer/$($package['id'])/4.0.6/$name"
        Invoke-WebRequest -Uri $url -OutFile $destination -UseBasicParsing
    } else {
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            throw "Locked WiX package is missing: $source"
        }
        Copy-Item -LiteralPath $source -Destination $destination
    }
    $item = Get-Item -LiteralPath $destination -Force
    if ($item.Length -ne $package['size'] -or
        (Get-MoSha512Base64 $destination) -cne $package['sha512']) {
        throw "Locked WiX package verification failed: $name"
    }
}

$cli = Join-Path $output 'cli'
$extensions = Join-Path $output 'extensions'
New-Item -ItemType Directory -Path $cli,$extensions | Out-Null
tar -xf (Join-Path $packages 'wix.4.0.6.nupkg') -C $cli 'tools/net6.0/any'
if ($LASTEXITCODE -ne 0) { throw 'Failed to extract the locked WiX CLI package.' }
foreach ($id in @('Bal', 'Util', 'Dependency')) {
    $packageId = "wixtoolset.$($id.ToLowerInvariant()).wixext"
    $dll = "WixToolset.$id.wixext.dll"
    tar -xf (Join-Path $packages "$packageId.4.0.6.nupkg") -C $extensions "wixext4/$dll"
    if ($LASTEXITCODE -ne 0) { throw "Failed to extract the locked $id extension." }
    Move-Item -LiteralPath (Join-Path $extensions "wixext4\$dll") -Destination (Join-Path $extensions $dll)
}
$wixextDirectory = Join-Path $extensions 'wixext4'
if (Test-Path -LiteralPath $wixextDirectory) {
    Remove-Item -LiteralPath $wixextDirectory
}

$wix = Join-Path $cli 'tools\net6.0\any\wix.exe'
$version = (& $wix --version | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $version -cne '4.0.6+73c89738') {
    throw "Extracted WiX version mismatch: $version"
}

$manifest = [ordered]@{
    format = 1
    kind = 'mo-wix-development-toolchain'
    development_only = $true
    redistributable = $false
    version = $lock.Data['version']
    source_commit = $lock.Data['source_commit']
    lock_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $lock.Path).Hash
    files = Get-MoStageInventory $output
}
$json = $manifest | ConvertTo-Json -Depth 8
[IO.File]::WriteAllText(
    (Join-Path $output 'wix-toolchain.json'),
    $json + "`n",
    [Text.UTF8Encoding]::new($false))
$null = Assert-MoWixToolchain $output
Write-Host "Verified locked WiX development toolchain: $output"
