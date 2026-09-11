//! Deterministic backend contract and synchronous Engine Actor state machine.
//!
//! `EngineActor` is intentionally synchronous. The broker will own it on one
//! dedicated thread and communicate over channels; keeping threading outside
//! this crate makes command ordering directly testable.

#![forbid(unsafe_code)]

mod actor;
mod backend;
mod fake;

pub use actor::{EngineActor, EngineError};
pub use backend::EngineBackend;
pub use fake::{FakeBackend, FakeError, FakeEvent};
