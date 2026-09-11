//! Phase 0 broker state machine.
//!
//! This crate intentionally contains no Windows named-pipe implementation yet.
//! The `tcp_loopback_spike` module is a local diagnostic transport only; it is
//! not a security or AppContainer compatibility claim.

mod state;
pub mod tcp_loopback_spike;

pub use state::{
    BrokerConnection, BrokerError, ERROR_BAD_HANDSHAKE, ERROR_BAD_REQUEST,
    ERROR_INCOMPATIBLE_VERSION, ERROR_NO_SUCH_SESSION, ERROR_OUT_OF_ORDER, ERROR_SESSION_LIMIT,
};
