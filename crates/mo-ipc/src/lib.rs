//! Bounded, versioned wire primitives for Mo's local broker protocol.
//!
//! This crate deliberately uses only the Rust standard library. It defines a
//! transport-neutral frame and payload codec; it does not claim that any
//! particular Windows transport or AppContainer ACL has been validated.

#![forbid(unsafe_code)]

mod codec;
mod composition_snapshot;
mod frame;
mod message;
mod version;

pub use codec::{CodecError, Decoder, Encoder, PayloadCodec};
pub use composition_snapshot::{
    COMPOSITION_POSITION_HEADER_BYTES, CompositionPositions, CompositionSnapshot,
    FEATURE_COMPOSITION_POSITIONS, MIN_COMPOSITION_SNAPSHOT_PAYLOAD_LEN,
};
pub use frame::{
    FLAG_ERROR, FLAG_RESPONSE, Frame, FrameError, FrameHeader, HEADER_LEN, MAGIC, MAX_FRAME_LEN,
    MAX_PAYLOAD_LEN, MessageKind, read_frame, write_frame,
};
pub use message::{
    CandidateAction, CandidateActionKind, CandidatePage, CharacterSet, DetailedCandidate,
    DetailedSnapshot, ErrorMessage, FEATURE_CANDIDATE_ACTIONS, FEATURE_CANDIDATE_DETAILS,
    FEATURE_CANDIDATE_PRESENTATION, FEATURE_KEY_EVENTS, FEATURE_PRESENTATION_SETTINGS,
    FEATURE_QUICK_SETTINGS, FEATURE_SESSION_SETTINGS_ACK, FEATURE_SETTINGS_SNAPSHOT,
    FEATURE_SYMBOL_ACTIONS, Hello, HelloAck, InputScheme, KeyEvent, MAX_CANDIDATE_BYTES,
    MAX_CANDIDATE_COMMENT_BYTES, MAX_CANDIDATE_LABEL_BYTES, MAX_CANDIDATES, MAX_COMMIT_BYTES,
    MAX_COMPOSITION_BYTES, MAX_ERROR_MESSAGE_BYTES, MIN_DETAILED_SNAPSHOT_PAYLOAD_LEN,
    MIN_NEGOTIATED_PAYLOAD_LEN, MIN_PRESENTATION_SNAPSHOT_PAYLOAD_LEN, PresentationSettings,
    PresentationSnapshot, SetEmoji, SettingsOrigin, SettingsSnapshot, Snapshot, SymbolAction,
    Theme,
};
pub use message::{
    MAX_CANDIDATE_FONT_SIZE_PX, MAX_CANDIDATE_PAGE_SIZE, MAX_CANDIDATE_RADIUS_PX,
    MAX_CANDIDATE_SKIN_ID, MAX_RGB_COLOR, MIN_CANDIDATE_FONT_SIZE_PX, MIN_CANDIDATE_PAGE_SIZE,
    MIN_CANDIDATE_RADIUS_PX, PRESENTATION_PAGE_HEADER_LEN, SETTINGS_SNAPSHOT_PAYLOAD_LEN,
};
pub use version::{CURRENT_VERSION, ProtocolVersion, VersionRange, negotiate_version};
