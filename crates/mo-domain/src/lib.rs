//! Platform-neutral domain types shared by Mo's engine and frontends.
//!
//! This crate deliberately has no operating-system or third-party dependencies.

#![forbid(unsafe_code)]

mod engine;
mod input;
mod text;

pub use engine::{
    Candidate, EngineCommand, EngineOutput, EngineSnapshot, EngineStatus, Generation, Revision,
    SessionOptions, SessionToken,
};
pub use input::{KeyEvent, KeyModifiers, KeyState};
pub use text::{
    Composition, CompositionError, GraphemeClusterIndex, TextRange, UnicodeScalarIndex,
    Utf8ByteOffset, Utf16CodeUnitOffset,
};
