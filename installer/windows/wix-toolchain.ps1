Set-StrictMode -Version Latest

function Get-MoSha512Base64([string]$Path) {
    $hex = (Get-FileHash -Algorithm SHA512 -LiteralPath $Path).Hash
    $bytes = [byte[]]::new($hex.Length / 2)
    for ($index = 0; $index -lt $bytes.Length; $index++) {
        $bytes[$index] = [Convert]::ToByte($hex.Substring($index * 2, 2), 16)
    }
    return [Convert]::ToBase64String($bytes)
}

function Read-MoWixToolchainLock {
    $lockPath = Join-Path $PSScriptRoot 'wix-toolchain-lock.json'
    $lock = Read-MoStageJson $lockPath
    if ($lock.Count -ne 4 -or $lock['format'] -ne 1 -or
        $lock['version'] -cne '4.0.6' -or
        $lock['source_commit'] -cne '73c897383236ddbbbc6ba257634013c1269ceec2' -or
        $lock['packages'] -isnot [object[]] -or $lock['packages'].Count -ne 4) {
        throw 'Invalid WiX toolchain lock metadata.'
    }
    $expectedIds = @(
        'wix',
        'wixtoolset.bal.wixext',
        'wixtoolset.util.wixext',
        'wixtoolset.dependency.wixext'
    )
    for ($index = 0; $index -lt $expectedIds.Count; $index++) {
        $package = $lock['packages'][$index]
        if ($package -isnot [Collections.IDictionary] -or $package.Count -ne 3 -or
            $package['id'] -cne $expectedIds[$index] -or
            ($package['size'] -isnot [int] -and $package['size'] -isnot [long]) -or
            $package['size'] -le 0 -or $package['size'] -gt 33554432 -or
            $package['sha512'] -isnot [string] -or
            $package['sha512'] -cnotmatch '^[A-Za-z0-9+/]{86}==$') {
            throw "Invalid WiX package lock entry: $($expectedIds[$index])"
        }
    }
    return [pscustomobject]@{
        Path = (Resolve-Path -LiteralPath $lockPath).Path
        Data = $lock
    }
}

function Assert-MoWixToolchain([string]$Directory) {
    $root = Assert-MoPlainPath $Directory
    $manifestPath = Join-Path $root 'wix-toolchain.json'
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw 'WiX toolchain completion manifest is missing.'
    }
    $manifest = Read-MoStageJson $manifestPath
    Assert-MoDevelopmentMetadata $manifest 1
    $lock = Read-MoWixToolchainLock
    if ($manifest.Count -ne 8 -or
        $manifest['kind'] -cne 'mo-wix-development-toolchain' -or
        $manifest['version'] -cne $lock.Data['version'] -or
        $manifest['source_commit'] -cne $lock.Data['source_commit'] -or
        $manifest['lock_sha256'] -cne (Get-FileHash -Algorithm SHA256 -LiteralPath $lock.Path).Hash -or
        $manifest['files'] -isnot [Collections.IDictionary]) {
        throw 'WiX toolchain manifest contract mismatch.'
    }
    Assert-MoInventory $root $manifest['files'] @('wix-toolchain.json')

    $wix = Join-Path $root 'cli\tools\net6.0\any\wix.exe'
    $extensions = [ordered]@{
        Bal = Join-Path $root 'extensions\WixToolset.Bal.wixext.dll'
        Util = Join-Path $root 'extensions\WixToolset.Util.wixext.dll'
        Dependency = Join-Path $root 'extensions\WixToolset.Dependency.wixext.dll'
    }
    foreach ($file in @($wix) + @($extensions.Values)) {
        if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
            throw "WiX toolchain file is missing: $file"
        }
    }
    $version = (& $wix --version | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or $version -cne '4.0.6+73c89738') {
        throw "Unexpected WiX executable version: $version"
    }
    return [pscustomobject]@{
        Root = $root
        Wix = $wix
        Extensions = $extensions
    }
}
