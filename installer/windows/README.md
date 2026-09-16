# Windows installer — Phase 0 authoring probe

`Package.wxs` and `Bundle.wxs` record the intended WiX v4 split: an elevated,
per-machine MSI owns x64/x86 TIP files, the x64 user broker, the registrar, and
both COM registry views; Burn will eventually coordinate prerequisites and a
non-elevated current-user finalizer.

This is deliberately not a deployable installer. It does **not** call
`ITfInputProcessorProfileMgr::RegisterProfile`, enable the profile with
`InstallLayoutOrTip`, start a per-user broker, stop TSF hosts, implement rollback
for TSF state, or sign either package. The intended broker artifact is now the
default release named-pipe broker, which accepts no arguments and requires the
fixed Known Folder layout recorded by ADR 0016. The placeholder builder still
accepts arbitrary input artifacts and does not verify their build provenance;
it must not be treated as a production packaging path.

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
in CI. Package the allowlisted self-built librime at `runtime/librime/rime.dll`
and precompiled rime-ice at `data/rime-ice/build`; prepare the current user's
`LocalAppData/Mo/Rime` data and staging directories without running deployment
on the input hot path. Reject diagnostic/debug-assertions artifacts in the
release packaging pipeline. Never run an updater or broker as LocalSystem.
