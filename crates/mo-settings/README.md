# mo-settings

Typed, versioned product settings, independent of Rime YAML/Lua. The fixed
Known Folder path is LocalAppData/Mo/Profile/settings-v1.mo. The UTF-8 v1
document is bounded to 16 KiB and strict about versions, fields and values.
Only an absent file selects defaults; malformed or newer data is reported.

SettingsRuntime keeps a revisioned last-known-good snapshot. The Broker exposes
validated settings through IPC; runtime preference and desktop acceptance
contracts are recorded in the Phase 0 status and ADRs 0045–0051.

Writes create and flush a same-directory temporary file before atomic replacement.
Use save_atomic_if_unchanged with the loaded semantic snapshot for ordinary edits;
it rejects changed, deleted, newly corrupt or future data. save_atomic explicitly
replaces the document and is reserved for recovery or deliberate initialization.

Both APIs exclusively create a reserved .write-lock sidecar across the comparison
and replacement. On Windows, zero sharing and delete-on-close release the owned
guard even when its process exits. Busy writes fail immediately; foreign markers
are never adopted or deleted. This serializes cooperating Mo writers, not arbitrary
external programs. Other platforms clean up on ordinary drop; their crash behavior
has not been validated. See ADR 0057 for the Win10 regression evidence.
