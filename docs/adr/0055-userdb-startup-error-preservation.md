# ADR 0055: Reject user dictionary startup errors without automatic recovery

Status: Accepted for development runtime ABI 3 (2026-10-04); installed-host adoption pending.

## Context

Pinned LevelDB may ignore WAL read errors with paranoid checks disabled. Direct API tests returned success while losing visibility of all 128 synchronous synthetic records. Enabling strict checking alone is insufficient: upstream UserDictionary::Load schedules recovery, whose fallback may repair, rename, remove or recreate the database.

## Contract

Use strict LevelDB recovery checking. An ordinary runtime open failure returns failure and schedules no recovery task. Input-free preparation must confirm successful Load and current loaded state for exactly one required rime_ice main user dictionary. Optional stable text dictionaries keep their existing optional behavior. Preserve database files for a separate explicit backup/recovery workflow. Propagate preparation failure via return values and the existing noexcept C boundary; Broker must not announce readiness.

The initial isolated patch implemented this contract without log reuse; the initial evidence predates integration as a runtime-build input. Sharing-denied WAL reads and WAL checksum corruption reject through actual native/Actor/Broker paths; the finite original data-file sets/hashes remain unchanged across the rejection attempts, excluding diagnostic LOG/LOG.old/LOCK. Removing the synthetic fault permits reopening and reading all 32 synchronously seeded records.

## Adoption gates and limits

The existing v2 symbol cannot distinguish old behavior from this stricter contract. The ABI 3 follow-up defines that identity gate, integrates runtime-build/receipts/CI, rebuilds a fresh stage, and verifies normal preferences/learning/schema behavior. Installed Windows 10 adoption remains a separate gate. An explicit recovery UX/backup policy remains separate work. This does not add input replay, exactly-once commits, synchronous normal learning, generic corruption repair or a power-loss guarantee.

In the initial proposal turn, full workspace watchdog tests and TIP latency still failed. The accepted runtime, installed VM, deadlines and Phase 0 acceptance status remain unchanged. Evidence and reproduction: [Win10 user dictionary errors](../phase-0/WIN10-USERDB-ERRORS-EVIDENCE.md).

## 2026-10-04 follow-up

ADR 0056 identifies the observed post-abort exit delay and replaces the Broker
lifecycle exit path. Default and trace workspace tests now pass without relaxing
the three-second assertion. The previous failures remain in the proposal's
historical evidence. At that follow-up this native user dictionary policy remained Proposed;
ABI/provenance/CI, new stage and installed-host adoption gates remain open.

## ABI 3 integration

The tracked builder snapshots and applies `userdb-preserve.patch` after the core
learning patch, with explicit vendor Git/work-tree paths. A source guard checks
strict recovery, ordinary Load's lack of recovery scheduling, the remembered
Load result and exactly one healthy main dictionary. Actual fault tests cover
the supported error behavior; source guards do not prove arbitrary future code.

Only `mo_rime_prepare_resources_v3` is exported/resolved. Old v1/v2 and official
DLLs cannot satisfy prepared mode. The safe backend checks presence before
creating its private anchor; the deployment helper also requires v3.
Format 2 provenance requires ABI 3 and
`userdb_policy = strict-open-no-auto-recovery-v1` with its patch hash bound to
the own-source inventory. Runtime and completed-stage checks enforce these
fields; resealing the inventory does not make incorrect policy fields valid.
Provenance remains a development consistency record, not an authenticated trust
root or release authorization.

A fresh full pinned runtime build and release stage pass normal preparation,
relocation, five-schema/character-mode/Emoji and private-normal-private learning
regressions on Win10. The actual Actor/Broker error matrix preserves both sets of
32 synchronous synthetic records. CI includes the source guard, real fault matrix,
and normal regressions; a remote CI result is not implied by local verification.
Strict TIP latency and installed-host acceptance remain independently required.
See [ABI 3 evidence](../phase-0/WIN10-USERDB-ABI3-EVIDENCE.md).
