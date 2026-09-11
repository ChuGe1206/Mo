# Mo Windows TIP — Phase 0 shell

This directory contains a Mo-owned, C++17 COM/TSF ABI shell. It implements
`IClassFactory`, `ITfTextInputProcessorEx`, and the `ITfKeyEventSink`
advise/unadvise lifecycle. Key callbacks currently fail open and never consume
input. It does not yet request edit sessions, render candidates, or connect to
the Rust broker.

`build-probe.ps1` builds x64 and Win32 variants and loads each DLL into a probe
of matching bitness. The probe checks exports, class creation, the
`ITfTextInputProcessorEx`/`ITfKeyEventSink` interfaces, deterministic sink
lifecycle against a controlled manager, fail-open key behavior, and unload
accounting. It never registers or enables the TIP.

The registrar is a deliberately separate mutation helper. It demonstrates
`ITfInputProcessorProfileMgr::RegisterProfile`, keyboard-category registration,
and dynamically loading `InstallLayoutOrTip` from the system `input.dll`. Its
installer transaction, rollback behavior, elevation boundary, and current-user
finalization are not validated in Phase 0.

Still unverified: sink activation by a registered TSF host, real text input,
x86 host coverage beyond the standalone ABI probe, AppContainer hosts, secure
desktop behavior, signing, upgrade, repair, and uninstall. Named-pipe security
is verified separately by the Rust workspace tests; TIP-to-pipe I/O is not.
