//! Windows named-pipe transport and its process-boundary security checks.
//!
//! The crate keeps Win32 and `unsafe` code out of Mo's domain and engine
//! crates. A server connection is exposed only after one bounded Mo IPC frame
//! has been read and the client's logon SID has been authenticated.

#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{AuthenticatedPipe, PipeAddress, PipeClient, PipeListener};
