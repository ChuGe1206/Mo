#Requires -Version 5.1
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'vm-workspace-policy.ps1')
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $repo ('build/mo-workspace-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$tests = 0
function Reject([scriptblock]$Action, [string]$Pattern) {
    try { & $Action; throw 'Expected rejection was absent.' } catch {
        if ($_.Exception.Message -notmatch $Pattern) { throw }
    }
    $script:tests++
}
try {
    $source = Join-Path $fixture 'drive'
    $workspace = Join-Path $source 'MoDev'
    foreach ($name in @('MoKit0110', 'MoEvidence0911', 'MoDesktopEvidence0100', 'Program Files', 'Windows', 'Mo', 'MoPersonal')) {
        New-Item -ItemType Directory -Path (Join-Path $source $name) | Out-Null
        [IO.File]::WriteAllBytes((Join-Path $source "$name/fixture.bin"), [byte[]](0,1,2,255))
    }
    $plan = @(Get-MoWorkspacePlan $source $workspace)
    if ($plan.Count -ne 3 -or (Test-Path $workspace)) { throw 'Preview modified directories or included unrelated data.' }
    $tests++
    Reject { Get-MoWorkspacePlan $source (Join-Path $fixture 'outside') } 'child'
    Reject { Assert-MoWorkspacePath '../escape' } 'absolute'
    Reject { Assert-MoWorkspacePath '\\server\share' } 'absolute'
    $conflict = Join-Path $workspace 'Archive/Legacy/MoKit0110'
    New-Item -ItemType Directory -Path $conflict -Force | Out-Null
    Reject { Get-MoWorkspacePlan $source $workspace } 'already exists'
    Remove-Item -LiteralPath $conflict
    $junction = Join-Path $source 'MoEvidence0911/link'
    New-Item -ItemType Junction -Path $junction -Target (Join-Path $source 'Windows') | Out-Null
    Reject { Get-MoWorkspacePlan $source $workspace } 'Reparse'
    # Windows PowerShell 5.1 Remove-Item can throw NullReferenceException for
    # directory junctions. Delete this verified fixture link itself, nonrecursively.
    if (-not [IO.Path]::GetFullPath($junction).StartsWith([IO.Path]::GetFullPath($fixture) + [IO.Path]::DirectorySeparatorChar)) {
        throw 'Junction cleanup escaped fixture.'
    }
    [IO.Directory]::Delete($junction)
    $plan = @(Get-MoWorkspacePlan $source $workspace)
    $plan[0].destination = Join-Path $fixture 'forged'
    Reject { Invoke-MoWorkspaceMigration $source $workspace $plan } 'plan changed'
    $plan = @(Get-MoWorkspacePlan $source $workspace)
    $journal = Invoke-MoWorkspaceMigration $source $workspace $plan
    $receipt = Get-Content -LiteralPath $journal -Raw | ConvertFrom-Json
    if (-not $receipt.completed -or $receipt.moves.Count -ne 3) { throw 'Incomplete migration receipt.' }
    foreach ($entry in $receipt.moves) {
        if (Test-Path -LiteralPath $entry.source) { throw 'Legacy source still present.' }
        $bytes = [IO.File]::ReadAllBytes((Join-Path $entry.destination 'fixture.bin'))
        if (($bytes -join ',') -cne '0,1,2,255' -or $entry.state -cne 'moved') { throw 'Migration changed fixture bytes.' }
    }
    foreach ($name in @('Program Files','Windows','Mo','MoPersonal')) {
        if (-not (Test-Path (Join-Path $source "$name/fixture.bin"))) { throw 'Unrelated directory changed.' }
    }
    $tests++
    if (@(Get-MoWorkspacePlan $source $workspace).Count -ne 0) { throw 'Repeated run would move archives again.' }
    $tests++
    $parsed = $null; $errors = $null
    $null = [Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'organize-vm-workspace.ps1'), [ref]$parsed, [ref]$errors)
    if ($errors.Count) { throw 'Organizer AST failed.' }
    $tests++
    Write-Host "VM workspace policy tests passed: $tests. Synthetic directories only."
} finally {
    $resolved = [IO.Path]::GetFullPath($fixture)
    $boundary = [IO.Path]::GetFullPath((Join-Path $repo 'build')) + '\'
    if (-not $resolved.StartsWith($boundary, [StringComparison]::OrdinalIgnoreCase)) { throw 'Fixture cleanup escaped build.' }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
