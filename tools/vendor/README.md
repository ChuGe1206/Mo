# Pinned upstream fetch

Run from the repository root:

```powershell
powershell -NoProfile -File tools/vendor/fetch-pinned.ps1
```

The script refuses to overwrite an existing destination. It checks out exact commits and verifies deterministic `git archive` hashes from `third_party/manifest.toml`.

Fetched trees are disposable build inputs under `build/` and are intentionally not committed. A future release pipeline must mirror the verified source archives and preserve their upstream license files.
