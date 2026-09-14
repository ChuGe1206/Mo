# Mo Windows TIP — Phase 0 shell

This directory contains a Mo-owned, C++17 COM/TSF ABI shell. It implements
`IClassFactory`, `ITfTextInputProcessorEx`, and the `ITfKeyEventSink`
advise/unadvise lifecycle. It also contains a small native Broker client that
uses the versioned MOIP protocol over the protected Windows Named Pipe. TIP
activation performs a 400 ms best-effort handshake and opens an isolated Broker
session; failure leaves the host usable. Key callbacks use a decision cache so
the matching `OnTestKey*` and `OnKey*` pair advances the engine only once, then
apply owned Broker snapshots through synchronous read/write edit sessions.
Missing or failed Broker connections use a bounded, throttled retry and fail
open.

`build-probe.ps1` builds x64 and Win32 variants and loads each DLL into a probe
of matching bitness. The probe checks exports, class creation, the
`ITfTextInputProcessorEx`/`ITfKeyEventSink` interfaces, deterministic sink
lifecycle against a controlled manager, fail-open key behavior, and unload
accounting. Its Broker mode supplies a deterministic `ITextStoreACP` backed by
a real Windows EDIT control and verifies preedit/commit writes through TSF
ranges for both fake and rime-ice inputs. It never registers or enables the TIP.

`tools/tip-broker-smoke.ps1` starts the x64 Rust Broker and exercises the x64
and Win32 native clients through a real
`Hello -> OpenSession -> KeyEvent -> Snapshot -> CloseSession` exchange. Every
native pipe operation is overlapped and has a hard deadline; ambiguous or
invalid responses reset the connection. The smoke also loads the TIP and
verifies that the test/key callback pair commits exactly once through an edit
session for both x64 and Win32.

The registrar is a deliberately separate mutation helper. In development it
can write the x64 and x86 `InprocServer32` values to their explicit HKCU COM
views, register the TSF profile and keyboard category, and dynamically load
`InstallLayoutOrTip` from the system `input.dll`. Registration rejects relative
or missing binaries and conflicting existing paths. `status` is read-only;
`test-registrar.ps1` writes and removes only an isolated test CLSID and never
registers or enables Mo. The production installer transaction, elevation
boundary, and current-user finalization remain separate work.

Still unverified: sink activation by a registered TSF host, formal composition
behavior across the real Notepad/WinUI host matrix, candidate UI, AppContainer
hosts, secure desktop behavior, signing, upgrade, repair, and uninstall. The
controlled text store proves the callback/cache/edit-session mechanics but is
not a substitute for registered end-to-end host testing. Broker identity is
still not authenticated back to the TIP client.
