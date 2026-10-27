//! Negotiated authoritative input mode; existing snapshot bytes stay unchanged.
use crate::{
    CodecError, CompositionSnapshot, Decoder, Encoder, MIN_COMPOSITION_SNAPSHOT_PAYLOAD_LEN,
    PayloadCodec,
};

pub const FEATURE_INPUT_MODE: u64 = 1 << 10;
/// Two canonical booleans: ASCII mode and engine-disabled state.
pub const INPUT_MODE_HEADER_BYTES: usize = 2;
pub const MIN_MODE_SNAPSHOT_PAYLOAD_LEN: usize =
    MIN_COMPOSITION_SNAPSHOT_PAYLOAD_LEN + INPUT_MODE_HEADER_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModeSnapshot {
    pub ascii_mode: bool,
    pub disabled: bool,
    pub composition: CompositionSnapshot,
}

impl PayloadCodec for ModeSnapshot {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        let mut encoder = Encoder::new();
        encoder.put_bool(self.ascii_mode);
        encoder.put_bool(self.disabled);
        let mut bytes = encoder.finish();
        bytes.extend(self.composition.encode_payload()?);
        Ok(bytes)
    }

    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let header = payload
            .get(..INPUT_MODE_HEADER_BYTES)
            .ok_or(CodecError::InvalidValue("missing input mode header"))?;
        let mut decoder = Decoder::new(header);
        let ascii_mode = decoder.get_bool()?;
        let disabled = decoder.get_bool()?;
        decoder.finish()?;
        Ok(Self {
            ascii_mode,
            disabled,
            composition: CompositionSnapshot::decode_payload(&payload[INPUT_MODE_HEADER_BYTES..])?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum InputModeAction {
    Inspect = 0,
    Chinese = 1,
    English = 2,
}

/// Inspection uses revision zero. Mutations require the last authoritative revision
/// and are rejected while composing, without consuming input or clearing preedit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputModeRequest {
    pub expected_revision: u64,
    pub action: InputModeAction,
}

impl PayloadCodec for InputModeRequest {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        if (self.action == InputModeAction::Inspect) != (self.expected_revision == 0) {
            return Err(CodecError::InvalidValue("invalid input mode revision"));
        }
        let mut encoder = Encoder::new();
        encoder.put_u64(self.expected_revision);
        encoder.put_u8(self.action as u8);
        Ok(encoder.finish())
    }

    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let mut decoder = Decoder::new(payload);
        let expected_revision = decoder.get_u64()?;
        let action = match decoder.get_u8()? {
            0 => InputModeAction::Inspect,
            1 => InputModeAction::Chinese,
            2 => InputModeAction::English,
            _ => return Err(CodecError::InvalidValue("invalid input mode action")),
        };
        decoder.finish()?;
        if (action == InputModeAction::Inspect) != (expected_revision == 0) {
            return Err(CodecError::InvalidValue("invalid input mode revision"));
        }
        Ok(Self {
            expected_revision,
            action,
        })
    }
}
