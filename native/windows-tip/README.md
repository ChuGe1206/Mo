# Mo Windows TIP — Phase 0 shell

This directory contains a Mo-owned, C++17 COM/TSF ABI shell. It implements
`IClassFactory`, `ITfTextInputProcessorEx`, and the `ITfKeyEventSink`
advise/unadvise lifecycle. It also contains a small native Broker client that
uses the versioned MOIP protocol over the protected Windows Named Pipe. TIP
activation performs a 50 ms best-effort handshake and opens an isolated Broker
session; failure leaves the host usable. Key callbacks still fail open and do
not dispatch keys until edit-session and duplicate-event handling are ready.

`build-probe.ps1` builds x64 and Win32 variants and loads each DLL into a probe
of matching bitness. The probe checks exports, class creation, the
`ITfTextInputProcessorEx`/`ITfKeyEventSink` interfaces, deterministic sink
lifecycle against a controlled manager, fail-open key behavior, and unload
accounting. It never registers or enables the TIP.

`tools/tip-broker-smoke.ps1` starts the x64 Rust Broker and exercises the x64
and Win32 native clients through a real
`Hello -> OpenSession -> KeyEvent -> Snapshot -> CloseSession` exchange. Every
native pipe operation is overlapped and has a hard deadline; ambiguous or
invalid responses reset the connection.

The registrar is a deliberately separate mutation helper. It demonstrates
`ITfInputProcessorProfileMgr::RegisterProfile`, keyboard-category registration,
and dynamically loading `InstallLayoutOrTip` from the system `input.dll`. Its
installer transaction, rollback behavior, elevation boundary, and current-user
finalization are not validated in Phase 0.

Still unverified: sink activation by a registered TSF host, key callback to
Broker dispatch, edit sessions, real text input, composition/candidate UI,
AppContainer hosts, secure desktop behavior, signing, upgrade, repair, and
uninstall. The standalone IPC probe verifies framed TIP-client transport, but
does not yet prove Broker identity to the client or a real TSF host path.
