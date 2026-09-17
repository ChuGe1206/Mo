# Mo Windows TIP — Phase 0 shell

This directory contains a Mo-owned, C++17 COM/TSF ABI shell. It implements
`IClassFactory`, `ITfTextInputProcessorEx`, `ITfKeyEventSink`, and
`ITfCompositionSink`, and `ITfTextLayoutSink`, including symmetric advise/unadvise and host-owned
composition termination. It also contains a small native Broker client that
uses the versioned MOIP protocol over the protected Windows Named Pipe. TIP
activation performs a 400 ms best-effort handshake and opens an isolated Broker
session; failure leaves the host usable. Key callbacks use a decision cache so
the matching `OnTestKey*` and `OnKey*` pair advances the engine only once, then
apply owned Broker snapshots through synchronous read/write edit sessions.
Missing or failed Broker connections use a bounded, throttled retry and fail
open.

The first candidate view is a separate Mo-owned Win32/GDI presentation module:
vertical display ordinals, mouse paging/selection, owner DPI scaling and monitor
work-area placement. It does not own a language engine or candidate ordering.
The popup uses NOACTIVATE/TOOLWINDOW and MA_NOACTIVATE. UI-element-only hosts do
not receive this self-drawn window. CandidateAction is an additive feature-gated
IPC 1.0 request; old peers keep the unchanged key/Snapshot layout. UI actions
require an exact current session/page revision and an in-page ordinal.

Mouse callbacks queue an owned action with ASYNCDONTCARE/READWRITE and dispatch
only inside the edit lock after revalidating context, focus, generation, token
and revision. New keys or lost focus cancel stale queued actions without an
engine commit. Layout notifications hide the stale view and request an
identity-checked asynchronous read lock to query the caret again. Zero-width
caret rectangles remain valid; unavailable, hidden or clipped layouts hide the
popup. The client scans 16 fixed protected pipe slots, retrying saturation or a
previously authenticated Broker restart within the original hard deadline.
Initial absence of all slots still returns immediately; an unexpected server
identity fails closed rather than being skipped.

`build-probe.ps1` builds x64 and Win32 variants and loads each DLL into a probe
of matching bitness. The probe checks exports, class creation, the
`ITfTextInputProcessorEx`/`ITfKeyEventSink` interfaces, deterministic sink
lifecycle against a controlled manager, fail-open key behavior, and unload
accounting. Its Broker mode supplies a deterministic `ITextStoreACP` backed by
a real Windows EDIT control and verifies preedit/commit writes through TSF
ranges for both fake and rime-ice inputs. Its controlled Broker mode now checks
keyboard commit, candidate popup visibility/nonactivation, mouse paging and
selection, unhandled key-up revision refresh, stale mouse presses, moved text
layout, forced deferred-lock cancellation by newer keys and lost focus, then
reconnection and a third commit. Final EDIT and TSF context text must both
match. This mode never registers or enables the TIP.

`tools/tip-broker-smoke.ps1` starts the x64 Rust Broker and exercises the x64
and Win32 native clients through a real
`Hello -> OpenSession -> KeyEvent -> Snapshot -> CloseSession` exchange. Every
native pipe operation is overlapped and has a hard deadline; ambiguous or
invalid responses reset the connection. The smoke also loads the TIP and
verifies that the test/key callback pair commits exactly once through an edit
session for both x64 and Win32. The production Broker serves 16 simultaneous
connections through independent single-instance pipe slots, keeping its
thread-affine engine on one dedicated thread. The protected DACL is unchanged:
clients cannot create another server instance. Each slot retains its original
server handle across disconnects, avoiding a namespace rearm gap. The native
pool probe holds all 16 clients, checks bounded failure of a 17th, frees and
reuses a middle slot, and commits each original connection's own candidate page
with fake and real rime-ice backends. It also runs three full-pool saturation,
release and recovery cycles, checking fresh preedit and no replay of the prior
word on the next Space. The server now uses overlapped connect/read/write;
assembly and whole-response budgets are bounded, while ordinary idle waits
remain open without millisecond polling. Flush does not wait for peer
consumption. Cancellation is drained before freeing OVERLAPPED or buffers.
The smoke owns and stops the persistent process explicitly. See ADR 0019/0020
for capacity, timeout semantics and remaining engine/shutdown/fault limits.

The optional registered-host probe deliberately separates privileges. From an
elevated PowerShell, run `tools\machine-profile.ps1 -Action Register` once to
create only the machine-wide TSF profile/category. Return to a normal
PowerShell and run `tools\tip-broker-smoke.ps1 -Registered`; it temporarily
owns the two HKCU COM views and current-user enablement, routes keys through the
real system `ITfKeystrokeMgr`, then removes that temporary user state in a
`finally` block. Finish from an elevated PowerShell with
`tools\machine-profile.ps1 -Action Unregister`. The scripts never make Mo the
default input method.

The real `tools\tip-rime-smoke.ps1` also accepts `-Registered` with its ordinary
explicit runtime/data arguments. Both registered entry points reject an
elevated test/Broker and fail preflight before building/deploying when the
machine profile or clean user state is missing. A shared transaction marks
mutation attempts before calling native helpers, so partial failures still
trigger cleanup. Cleanup steps are independent; final state/readback failures
are fatal, not warnings followed by success. A foreign COM path detected before
cleanup is preserved and reported for manual review (not an atomic concurrent
registry transaction). Sixteen in-memory policy scenarios run without changing
Windows input state. See `docs/phase-0/REGISTERED-TEST.md`; this system key-route
probe still uses a controlled text store, not real Notepad or a browser.

The registrar is a deliberately separate mutation helper. In development it
can write the x64 and x86 `InprocServer32` values to their explicit HKCU COM
views. Its explicitly named machine-profile commands require elevation to
register the TSF profile and keyboard category; current-user enable/disable is
a separate operation that dynamically loads `InstallLayoutOrTip` from the
system `input.dll`. Registration rejects relative or missing binaries and
conflicting existing paths. `status` is read-only;
`test-registrar.ps1` writes and removes only an isolated test CLSID and never
registers or enables Mo. The production installer transaction and current-user
finalization remain separate work.

Still unverified: sink activation by a registered TSF host, formal composition
behavior and candidate UI across the real Notepad/WinUI host matrix, mixed DPI,
TSF UIElement cooperation, candidate metadata, AppContainer
hosts, secure desktop behavior, signing, upgrade, repair, and uninstall. The
controlled text store proves the callback/cache/edit-session mechanics but is
not a substitute for registered end-to-end host testing. See ADR 0018 for the
first native view's staged Rust-first boundary and remaining release gates.

ADR 0022 adds QPC-backed, completion-checked request deadlines; the 50 ms key
budget is unchanged. `build-probe.ps1 -LatencyTrace` explicitly enables a
read-only, content-free diagnostics interface (default builds expose none).
Real smoke accepts the same switch and `-OpenccDataDir <verified-pack>`;
build-time resource instructions are in `tools/opencc-build/README.md`.
The Broker keeps one private input-free resource session to avoid unloading
shared dictionaries when frontend sessions close; it never warms or replays
user input. Pressure and ordinary-host acceptance remain separate gates.

ADR 0023 adds owned context/range references and a saturating candidate epoch,
revalidating after host geometry calls and before/after Win32 popup updates.
A deterministic GetTextExt reentrancy probe invalidates the measured layout,
checks that the old popup stays hidden, then verifies a fresh read recovers it.
TIP reconnect and key exchange now share one absolute 50 ms transport deadline,
with no additional CloseSession wait on key error/context-switch paths. This
does not bound synchronous host COM/edit/render work or kernel cancellation.
Opt-in metadata separately records reset cause/count and key-edit HRESULTs;
it does not turn the existing rare-disappearance or cold-tail risks into passes.
