# Third-party source policy

`manifest.toml` is a human-reviewable Phase 0 lock draft, not a generated SBOM and not a legal conclusion.

Before any release:

1. Replace every unresolved commit with a fetched, verified commit.
2. Record archive and generated-output SHA-256 values.
3. Generate SPDX JSON and `THIRD_PARTY_NOTICES` from the actual distribution graph.
4. Preserve upstream license and attribution files verbatim.
5. Publish exact corresponding source and build scripts for GPL-derived resource packs.
6. Keep fetched source outside the Cargo workspace unless a crate explicitly builds it.

