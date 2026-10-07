# mo-settings-app

Native Rust/Win32 per-user settings frontend. It exposes input scheme, character
set, candidate theme/comments, Emoji, local learning and privacy preferences
through validated runtime contracts; planned candidate page size remains preserved.

The controller distinguishes absent, corrupt and future documents. Only the explicit
restore-defaults action may replace unreadable data. Saving verifies and creates the
Known Folder Mo/Profile directory one component at a time, rejecting reparse points.

Ordinary saves compare the latest stored semantic document with the window's loaded
snapshot under the shared Mo write guard. A stale window cannot overwrite another
window's choices or a future document. It reports reload-required/busy status and
retains its existing snapshot. Reload observes the latest data; explicit recovery
uses the same guard without a snapshot precondition. See ADR 0057.

The executable is mo-settings.exe. It stays asInvoker and never writes Rime YAML/Lua.
Installed desktop behavior and visual acceptance remain separate from controller tests.
