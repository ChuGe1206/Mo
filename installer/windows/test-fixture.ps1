# Test-only pre-delete traversal: accept incomplete/empty fixture directories but
# never follow reparse points. Callers must separately validate exact root scope.
# Dot-sourcing performs no mutation.
function Assert-MoOwnedFixtureTree([string]$Directory) {
    $pending = [Collections.Generic.Stack[string]]::new()
    $pending.Push((Assert-MoPlainPath $Directory))
    while ($pending.Count) {
        foreach ($entry in Get-ChildItem -LiteralPath $pending.Pop() -Force) {
            if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Unexpected fixture reparse point; refusing recursive cleanup.' }
            if ($entry.PSIsContainer) { $pending.Push($entry.FullName) }
        }
    }
}
