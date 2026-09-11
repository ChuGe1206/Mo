[CmdletBinding()]
param(
    [string]$Destination = (Join-Path (Get-Location) "build/vendor-source")
)

$ErrorActionPreference = "Stop"

$destinationPath = [System.IO.Path]::GetFullPath($Destination)
if (Test-Path -LiteralPath $destinationPath) {
    throw "Destination already exists; refusing to overwrite: $destinationPath"
}

$librimeCommit = "33e78140250125871856cdc5b42ddc6a5fcd3cd4"
$rimeIceCommit = "6810e8916d160498620a16fef2135956fecbd485"
$expectedLibrimeArchive = "B22594E1FCF55DF5BBF60E76DC49200671410E98540BE265F68465D03678C722"
$expectedRimeIceArchive = "CD1895FBC961131A62F23277F636C27A6FB941DAC66DAF43C4A10D4E9E6ADAD3"

New-Item -ItemType Directory -Path $destinationPath | Out-Null

$librimePath = Join-Path $destinationPath "librime"
$rimeIcePath = Join-Path $destinationPath "rime-ice"

git clone --filter=blob:none --no-checkout https://github.com/rime/librime.git $librimePath
git -C $librimePath checkout --detach $librimeCommit

git clone --filter=blob:none --no-checkout https://github.com/iDvel/rime-ice.git $rimeIcePath
git -C $rimeIcePath checkout --detach $rimeIceCommit

$actualLibrimeCommit = (git -C $librimePath rev-parse HEAD).Trim()
$actualRimeIceCommit = (git -C $rimeIcePath rev-parse HEAD).Trim()
if ($actualLibrimeCommit -ne $librimeCommit) {
    throw "librime commit mismatch: $actualLibrimeCommit"
}
if ($actualRimeIceCommit -ne $rimeIceCommit) {
    throw "rime-ice commit mismatch: $actualRimeIceCommit"
}

$librimeArchive = Join-Path $destinationPath "librime.tar"
$rimeIceArchive = Join-Path $destinationPath "rime-ice.tar"
git -C $librimePath archive --format=tar HEAD -o $librimeArchive
git -C $rimeIcePath archive --format=tar HEAD -o $rimeIceArchive

$actualLibrimeArchive = (Get-FileHash -Algorithm SHA256 -LiteralPath $librimeArchive).Hash
$actualRimeIceArchive = (Get-FileHash -Algorithm SHA256 -LiteralPath $rimeIceArchive).Hash
if ($actualLibrimeArchive -ne $expectedLibrimeArchive) {
    throw "librime archive hash mismatch: $actualLibrimeArchive"
}
if ($actualRimeIceArchive -ne $expectedRimeIceArchive) {
    throw "rime-ice archive hash mismatch: $actualRimeIceArchive"
}

[pscustomobject]@{
    LibrimeCommit = $actualLibrimeCommit
    LibrimeArchiveSha256 = $actualLibrimeArchive
    RimeIceCommit = $actualRimeIceCommit
    RimeIceArchiveSha256 = $actualRimeIceArchive
    Destination = $destinationPath
}

