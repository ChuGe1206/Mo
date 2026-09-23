# mo-settings

`mo-settings` owns Mo's ordinary-user settings contract. It is intentionally
independent from Rime YAML/Lua and does not generate executable configuration.

The installed file is `LocalAppData\Mo\Profile\settings-v1.mo`, derived from the
Windows Known Folder root rather than an environment variable. The v1 document
is UTF-8, deterministic, limited to 16 KiB and strict about versions, fields and
values. Only an absent file selects product defaults; malformed or newer data is
reported to the caller.

Writes use a new file in the same directory, flush it, then atomically replace
the destination (`MoveFileExW` with replace/write-through on Windows). The future
settings frontend owns parent-directory creation and user-facing error recovery.

This crate currently establishes storage and validation only. Applying schema or
Rime options, generating a signed machine data slot, and the graphical settings
frontend remain separate acceptance stages.
