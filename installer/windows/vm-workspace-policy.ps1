Set-StrictMode -Version Latest

function Assert-MoWorkspacePath([string]$Path) {
    if ($Path -notmatch '^[A-Za-z]:\\' -or $Path.Substring(2).Contains(':')) {
        throw 'Workspace paths must be absolute local DOS paths.'
    }
    $full = [IO.Path]::GetFullPath($Path).TrimEnd('\')
    $current = $full.Substring(0, 2) + '\'
    $tail = if ($full.Length -gt 3) { $full.Substring(3) } else { '' }
    foreach ($part in $tail.Split('\', [StringSplitOptions]::RemoveEmptyEntries)) {
        $current = Join-Path $current $part
        if (Test-Path -LiteralPath $current) {
            if ((Get-Item -LiteralPath $current -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw 'Workspace paths must not traverse reparse points.'
            }
        }
    }
    return $full
}

function Get-MoWorkspacePlan([string]$SourceRoot, [string]$WorkspaceRoot) {
    $sourceBase = Assert-MoWorkspacePath $SourceRoot
    $workspace = Assert-MoWorkspacePath $WorkspaceRoot
    if (-not $workspace.StartsWith($sourceBase + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Workspace must be a child of the source root on the same volume.'
    }
    $plan = @()
    foreach ($item in Get-ChildItem -LiteralPath ($sourceBase + '\') -Directory -Force) {
        if ($item.Name -notmatch '^Mo(?:DesktopEvidence|MatrixEvidence|Evidence|Kit|Matrix|Msi|Probe|ChangedEvidence|Changed)(?:[0-9]+|V[0-9]+)?$') { continue }
        $source = Assert-MoWorkspacePath $item.FullName
        $destination = Assert-MoWorkspacePath (Join-Path $workspace ("Archive/Legacy/" + $item.Name))
        if ((Split-Path -Parent $source).TrimEnd('\') -ine $sourceBase -or
            -not $destination.StartsWith($workspace + '\Archive\Legacy\', [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Migration path escapes the intended workspace.'
        }
        if (Test-Path -LiteralPath $destination) { throw "Migration destination already exists: $destination" }
        $pending = [Collections.Generic.Stack[string]]::new()
        $pending.Push($source)
        while ($pending.Count -gt 0) {
            foreach ($entry in Get-ChildItem -LiteralPath $pending.Pop() -Force) {
                if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse point in legacy tree.' }
                if ($entry.PSIsContainer) { $pending.Push($entry.FullName) }
            }
        }
        $plan += [pscustomobject]@{ source = $source; destination = $destination; state = 'planned' }
    }
    return $plan
}

function Invoke-MoWorkspaceMigration([string]$SourceRoot, [string]$WorkspaceRoot, [object[]]$Plan) {
    # Recompute immediately before moving; never trust a persisted or caller-built plan.
    $fresh = @(Get-MoWorkspacePlan $SourceRoot $WorkspaceRoot)
    if (($fresh | ConvertTo-Json -Depth 4 -Compress) -cne ($Plan | ConvertTo-Json -Depth 4 -Compress)) {
        throw 'Workspace migration plan changed; inspect again.'
    }
    $workspace = Assert-MoWorkspacePath $WorkspaceRoot
    foreach ($entry in $fresh) {
        if ((Get-Location).Path.StartsWith($entry.source, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Current directory is inside a legacy folder.'
        }
        foreach ($process in Get-Process) {
            $image = $null
            try { $image = $process.Path } catch { continue }
            if ($image -and $image.StartsWith($entry.source + '\', [StringComparison]::OrdinalIgnoreCase)) {
                throw 'A process is running from a legacy folder.'
            }
        }
    }
    foreach ($name in @('Packages', 'Evidence', 'Archive/Legacy', 'Manifests')) {
        New-Item -ItemType Directory -Path (Join-Path $workspace $name) -Force | Out-Null
    }
    $receipt = [ordered]@{
        format = 1; kind = 'mo-vm-workspace-migration'
        created_utc = [DateTime]::UtcNow.ToString('o')
        workspace = $workspace; completed = $false; moves = $fresh; failure = $null
    }
    $journal = Join-Path $workspace ('Manifests/migration-' + [Guid]::NewGuid().ToString('N') + '.json')
    $receipt | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $journal -Encoding UTF8
    try {
        foreach ($entry in $fresh) {
            $null = Assert-MoWorkspacePath $entry.source
            $null = Assert-MoWorkspacePath $entry.destination
            if (Test-Path -LiteralPath $entry.destination) { throw 'Migration destination appeared; refusing overwrite.' }
            Move-Item -LiteralPath $entry.source -Destination $entry.destination -ErrorAction Stop
            $entry.state = 'moved'
            $receipt | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $journal -Encoding UTF8
        }
        $receipt.completed = $true
    } catch {
        $receipt.failure = $_.Exception.Message
        throw
    } finally {
        $receipt | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $journal -Encoding UTF8
        Write-Host "Migration receipt: $journal"
    }
    return $journal
}
