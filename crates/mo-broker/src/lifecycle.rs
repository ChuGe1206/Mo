//! Broker-only fail-stop boundary. No host DLL links this Rust module.

/// Continuing after a timed-out native operation or failed worker could apply
/// a delayed commit. Do not unwind across librime, retry, detach an unsafe
/// worker, or block on stderr while trying to terminate the broken process.
pub(crate) fn fail_stop() -> ! {
    #[cfg(windows)]
    {
        mo_windows_platform::fail_stop_current_process()
    }
    #[cfg(not(windows))]
    {
        std::process::abort()
    }
}
