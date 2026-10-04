# ADR 0056: Direct process termination at the Broker fail-stop boundary

Status: Accepted for development, 2026-10-04. Supersedes the Windows exit mechanism and status check in ADR 0021; budgets and recovery semantics stay the same.

## Evidence

The previous workspace runs sometimes failed the three-second subprocess exit assertion. A test-only marker placed immediately before Rust abort separates receipt of the injected fault from entry to fail_stop. All eight old-process samples reached that boundary, and a WerFault command line identifying the owned PID was observed for every sample. Five exited after three seconds; total observed exit time was 2,183–7,430 ms. Thus at least these observed failures occur after entry to termination, rather than requiring a longer native-operation budget.

The marker and WER observation exist only in an archived diagnostic build/harness. Parent receipt/poll timestamps are approximate; WMI observation can delay the parent, and they do not measure CPU time or precisely partition WER overhead. Other historical scheduling failures are not all proven to have this cause.

## Decision

On Windows, Broker lifecycle::fail_stop calls the safe mo-windows-platform primitive that invokes TerminateProcess with GetCurrentProcess's pseudo-handle and fixed application exit code 0xE04D4F01. It cannot choose another PID. It allocates nothing, logs nothing, runs no Rust destructor or DLL detach hook, and does not wait for a user-mode crash report. If the API unexpectedly returns, abort is the last fallback. Non-Windows remains abort.

The eight subprocess tests continue to require their intended synthetic fault marker and actual process exit within three seconds. They now require the dedicated application status, rejecting a generic panic/crash code, a test failure, or arbitrary nonzero exit. No engine/TIP deadline was extended; no native operation or commit is retried. Normal coordinated shutdown still closes sessions and finalizes normally.

## Verification and boundaries

Twenty-four new diagnostic children (eight phases, three trials) exited with the dedicated status in 36–215 ms and without parent termination. Default and latency-trace workspace tests and Clippy pass; x64/Win32 nonregistered fake TIP IPC/pool/UI/edit/fault-reconnect smoke passes. Release-optimized Broker library tests include all eight fault phases and pass; Rust test harnesses use unwind, so this does not establish the behavior of every production panic=abort path.

Only the Broker lifecycle boundary is redirected. Existing low-level pipe failure aborts and Rust's implicit aborts are separate paths. Microsoft specifies that pending kernel I/O must complete or cancel before process teardown; this is not a guarantee against a permanently stuck kernel/driver. Forced termination also does not establish user dictionary durability or exactly-once commits. There is no WER registry/policy change, new VM installation, or Phase 0 gate promotion.

API contracts: [TerminateProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess), [fastfail](https://learn.microsoft.com/en-us/cpp/intrinsics/fastfail?view=msvc-170). Actual local evidence and identity: [Win10 watchdog exit](../phase-0/WIN10-WATCHDOG-EXIT-EVIDENCE.md).
