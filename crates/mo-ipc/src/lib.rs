//! Bounded, versioned wire primitives for Mo's local broker protocol.
//!
//! This crate deliberately uses only the Rust standard library. It defines a
//! transport-neutral frame and payload codec; it does not claim that any
//! particular Windows transport or AppContainer ACL has been validated.

mod codec;
mod frame;
mod message;
mod version;

pub use codec::{CodecError, Decoder, Encoder, PayloadCodec};
pub use frame::{
    FLAG_ERROR, FLAG_RESPONSE, Frame, FrameError, FrameHeader, HEADER_LEN, MAGIC, MAX_FRAME_LEN,
    MAX_PAYLOAD_LEN, MessageKind, read_frame, write_frame,
};
pub use message::{
    CandidateAction, CandidateActionKind, CharacterSet, DetailedCandidate, DetailedSnapshot,
    ErrorMessage, FEATURE_CANDIDATE_ACTIONS, FEATURE_CANDIDATE_DETAILS, FEATURE_KEY_EVENTS,
    FEATURE_SESSION_SETTINGS_ACK, FEATURE_SETTINGS_SNAPSHOT, Hello, HelloAck, InputScheme,
    KeyEvent, MAX_CANDIDATE_BYTES, MAX_CANDIDATE_COMMENT_BYTES, MAX_CANDIDATE_LABEL_BYTES,
    MAX_CANDIDATES, MAX_COMMIT_BYTES, MAX_COMPOSITION_BYTES, MAX_ERROR_MESSAGE_BYTES,
    MIN_DETAILED_SNAPSHOT_PAYLOAD_LEN, MIN_NEGOTIATED_PAYLOAD_LEN, SettingsOrigin,
    SettingsSnapshot, Snapshot, Theme,
};
pub use version::{CURRENT_VERSION, ProtocolVersion, VersionRange, negotiate_version};
