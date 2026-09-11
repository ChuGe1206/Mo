# Windows installer — Phase 0 authoring probe

`Package.wxs` and `Bundle.wxs` record the intended WiX v4 split: an elevated,
per-machine MSI owns x64/x86 TIP files, the x64 user broker, the registrar, and
both COM registry views; Burn will eventually coordinate prerequisites and a
non-elevated current-user finalizer.

This is deliberately not a deployable installer. It does **not** call
`ITfInputProcessorProfileMgr::RegisterProfile`, enable the profile with
`InstallLayoutOrTip`, start a per-user broker, stop TSF hosts, implement rollback
for TSF state, or sign either package. The installed broker input is currently
the TCP diagnostic spike, not a production named-pipe broker.

The build script has two safety gates:

1. If the WiX v4 `wix` command is absent it fails immediately and never downloads
   anything.
2. Even with WiX installed, `-AllowPlaceholderBuild` is required and the outputs
   are named as placeholders.

Before any release, replace the placeholder flow with transactional machine
registration plus a user-context finalizer; define repair/uninstall recovery;
use versioned binaries to tolerate loaded TIP DLLs; validate clean install,
upgrade, rollback, repair, and uninstall in disposable VMs; sign the x64/x86
DLLs, broker, MSI, and Burn EXE with timestamping; and enforce signature checks
in CI. Never run an updater or broker as LocalSystem.

