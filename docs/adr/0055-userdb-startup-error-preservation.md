# ADR 0055: Reject user dictionary startup errors without automatic recovery

Status: Proposed, independently verified on Windows 10 (2026-10-04).

## Context

Pinned LevelDB may ignore WAL read errors with paranoid checks disabled. Direct API tests returned success while losing visibility of all 128 synchronous synthetic records. Enabling strict checking alone is insufficient: upstream UserDictionary::Load schedules recovery, whose fallback may repair, rename, remove or recreate the database.

## Proposed contract

Use strict LevelDB recovery checking. An ordinary runtime open failure returns failure and schedules no recovery task. Input-free preparation must confirm successful Load and current loaded state for exactly one required rime_ice main user dictionary. Optional stable text dictionaries keep their existing optional behavior. Preserve database files for a separate explicit backup/recovery workflow. Propagate preparation failure via return values and the existing noexcept C boundary; Broker must not announce readiness.

The isolated patch implements this contract, without log reuse or a new production build input. Sharing-denied WAL reads and WAL checksum corruption reject through actual native/Actor/Broker paths; the finite original data-file sets/hashes remain unchanged across the rejection attempts, excluding diagnostic LOG/LOG.old/LOCK. Removing the synthetic fault permits reopening and reading all 32 synchronously seeded records.

## Adoption gates and limits

The existing v2 symbol cannot distinguish old behavior from this stricter contract. Before adoption, define a new ABI/provenance identity gate, integrate runtime-build/receipts/CI, rebuild isolated stages, and verify normal preferences/learning/schema behavior plus installed Windows 10 hosts. An explicit recovery UX/backup policy remains separate work. This does not add input replay, exactly-once commits, synchronous normal learning, generic corruption repair or a power-loss guarantee.

Full workspace watchdog tests and TIP latency still fail in this turn. The accepted runtime, installed VM, deadlines and Phase 0 acceptance status remain unchanged. Evidence and reproduction: [Win10 user dictionary errors](../phase-0/WIN10-USERDB-ERRORS-EVIDENCE.md).

## 2026-10-04 follow-up

ADR 0056 identifies the observed post-abort exit delay and replaces the Broker
lifecycle exit path. Default and trace workspace tests now pass without relaxing
the three-second assertion. The previous failures remain in the proposal's
historical evidence. This native user dictionary policy is still Proposed;
ABI/provenance/CI, new stage and installed-host adoption gates remain open.
