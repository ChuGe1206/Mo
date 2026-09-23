# mo-settings-app

Native Rust/Win32 settings frontend for Mo. It deliberately exposes only
settings whose runtime behavior is already validated. The current interactive
surface can change the candidate theme; engine-backed preferences are displayed
as read-only “under development” values.

The controller distinguishes an absent document from corrupt or future data.
Only an explicit **restore defaults** action may replace an unreadable document.
Saving creates `LocalAppData\Mo\Profile` one component at a time, rejects files
and reparse points, and delegates atomic replacement to `mo-settings`.

The executable is `mo-settings.exe`. It is an `asInvoker`, per-user settings
tool and must never request elevation or write Rime YAML/Lua.
