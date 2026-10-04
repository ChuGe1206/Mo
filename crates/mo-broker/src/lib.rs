//! Phase 0 broker state machine and process-boundary adapters.
//!
//! The TCP module remains diagnostic-only. On Windows, `windows_named_pipe`
//! connects the state machine to Mo's authenticated local transport.

mod engine_service;
mod latency;
mod lifecycle;
mod settings_service;
pub mod startup_latency;
mod state;
pub mod tcp_loopback_spike;
#[cfg(windows)]
pub mod windows_named_pipe;
#[cfg(windows)]
pub mod windows_runtime;

pub use state::{
    BrokerConnection, BrokerError, ERROR_BAD_HANDSHAKE, ERROR_BAD_REQUEST, ERROR_ENGINE_FAILURE,
    ERROR_INCOMPATIBLE_VERSION, ERROR_NO_SUCH_SESSION, ERROR_OUT_OF_ORDER, ERROR_SESSION_LIMIT,
    ERROR_SETTINGS_UNAVAILABLE, ERROR_STALE_CANDIDATES,
};
