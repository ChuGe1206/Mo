use crate::{
    CodecError, Decoder, Encoder, MAX_PAYLOAD_LEN, PayloadCodec, ProtocolVersion, VersionRange,
};

pub const FEATURE_KEY_EVENTS: u64 = 1 << 0;
pub const FEATURE_CANDIDATE_ACTIONS: u64 = 1 << 1;

/// A UI action against exactly the candidate page displayed by the caller.
/// Existing Snapshot encoding is unchanged; this additive message requires
/// FEATURE_CANDIDATE_ACTIONS in the negotiated HelloAck.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateAction {
    pub expected_revision: u64,
    pub action: CandidateActionKind,
    /// Zero-based page-local ordinal. Must be zero for page navigation.
    pub index: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateActionKind {
    Select,
    PreviousPage,
    NextPage,
}

impl PayloadCodec for CandidateAction {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        self.validate()?;
        let mut encoder = Encoder::new();
        encoder.put_u64(self.expected_revision);
        encoder.put_u8(match self.action {
            CandidateActionKind::Select => 0,
            CandidateActionKind::PreviousPage => 1,
            CandidateActionKind::NextPage => 2,
        });
        encoder.put_u32(self.index);
        Ok(encoder.finish())
    }

    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let mut decoder = Decoder::new(payload);
        let expected_revision = decoder.get_u64()?;
        let action = match decoder.get_u8()? {
            0 => CandidateActionKind::Select,
            1 => CandidateActionKind::PreviousPage,
            2 => CandidateActionKind::NextPage,
            _ => return Err(CodecError::InvalidValue("unknown candidate action")),
        };
        let index = decoder.get_u32()?;
        decoder.finish()?;
        let value = Self {
            expected_revision,
            action,
            index,
        };
        value.validate()?;
        Ok(value)
    }
}

impl CandidateAction {
    fn validate(&self) -> Result<(), CodecError> {
        if self.expected_revision == 0 {
            return Err(CodecError::InvalidValue(
                "candidate revision must be non-zero",
            ));
        }
        if self.action != CandidateActionKind::Select && self.index != 0 {
            return Err(CodecError::InvalidValue("page action index must be zero"));
        }
        if self.action == CandidateActionKind::Select && self.index as usize >= MAX_CANDIDATES {
            return Err(CodecError::InvalidValue(
                "candidate ordinal exceeds wire limit",
            ));
        }
        Ok(())
    }
}

pub const MAX_COMPOSITION_BYTES: usize = 4 * 1024;
pub const MAX_COMMIT_BYTES: usize = 4 * 1024;
pub const MAX_CANDIDATES: usize = 32;
pub const MAX_CANDIDATE_BYTES: usize = 512;
pub const MAX_ERROR_MESSAGE_BYTES: usize = 512;

// Largest valid Snapshot payload:
// revision + handled + composition + optional commit + candidate count +
// every length-prefixed candidate. A peer advertising less cannot safely
// receive every response permitted by this protocol version.
pub const MIN_NEGOTIATED_PAYLOAD_LEN: usize = 8
    + 1
    + (4 + MAX_COMPOSITION_BYTES)
    + (1 + 4 + MAX_COMMIT_BYTES)
    + 2
    + MAX_CANDIDATES * (4 + MAX_CANDIDATE_BYTES);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Hello {
    pub supported: VersionRange,
    pub features: u64,
    pub max_payload_len: u32,
}

impl PayloadCodec for Hello {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        validate_hello(self.supported, self.max_payload_len)?;
        let mut encoder = Encoder::new();
        put_version(&mut encoder, self.supported.minimum);
        put_version(&mut encoder, self.supported.maximum);
        encoder.put_u64(self.features);
        encoder.put_u32(self.max_payload_len);
        Ok(encoder.finish())
    }

    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let mut decoder = Decoder::new(payload);
        let supported = VersionRange::new(get_version(&mut decoder)?, get_version(&mut decoder)?)?;
        let features = decoder.get_u64()?;
        let max_payload_len = decoder.get_u32()?;
        decoder.finish()?;
        validate_hello(supported, max_payload_len)?;
        Ok(Self {
            supported,
            features,
            max_payload_len,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelloAck {
    pub selected: ProtocolVersion,
    pub features: u64,
    pub max_payload_len: u32,
}

impl PayloadCodec for HelloAck {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        validate_payload_limit(self.max_payload_len)?;
        let mut encoder = Encoder::new();
        put_version(&mut encoder, self.selected);
        encoder.put_u64(self.features);
        encoder.put_u32(self.max_payload_len);
        Ok(encoder.finish())
    }

    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let mut decoder = Decoder::new(payload);
        let selected = get_version(&mut decoder)?;
        let features = decoder.get_u64()?;
        let max_payload_len = decoder.get_u32()?;
        decoder.finish()?;
        validate_payload_limit(max_payload_len)?;
        Ok(Self {
            selected,
            features,
            max_payload_len,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyEvent {
    pub virtual_key: u32,
    pub scan_code: u32,
    pub modifiers: u16,
    pub key_down: bool,
    pub repeat: bool,
}

impl PayloadCodec for KeyEvent {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        let mut encoder = Encoder::new();
        encoder.put_u32(self.virtual_key);
        encoder.put_u32(self.scan_code);
        encoder.put_u16(self.modifiers);
        encoder.put_bool(self.key_down);
        encoder.put_bool(self.repeat);
        Ok(encoder.finish())
    }

    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let mut decoder = Decoder::new(payload);
        let event = Self {
            virtual_key: decoder.get_u32()?,
            scan_code: decoder.get_u32()?,
            modifiers: decoder.get_u16()?,
            key_down: decoder.get_bool()?,
            repeat: decoder.get_bool()?,
        };
        decoder.finish()?;
        Ok(event)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub revision: u64,
    pub handled: bool,
    pub composition: String,
    pub commit: Option<String>,
    pub candidates: Vec<String>,
}

impl PayloadCodec for Snapshot {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        if self.candidates.len() > MAX_CANDIDATES {
            return Err(CodecError::TooManyItems {
                declared: self.candidates.len(),
                maximum: MAX_CANDIDATES,
            });
        }
        let candidate_count =
            u16::try_from(self.candidates.len()).map_err(|_| CodecError::IntegerOverflow)?;
        let mut encoder = Encoder::new();
        encoder.put_u64(self.revision);
        encoder.put_bool(self.handled);
        encoder.put_string(&self.composition, MAX_COMPOSITION_BYTES)?;
        encoder.put_optional_string(self.commit.as_deref(), MAX_COMMIT_BYTES)?;
        encoder.put_u16(candidate_count);
        for candidate in &self.candidates {
            encoder.put_string(candidate, MAX_CANDIDATE_BYTES)?;
        }
        Ok(encoder.finish())
    }

    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let mut decoder = Decoder::new(payload);
        let revision = decoder.get_u64()?;
        let handled = decoder.get_bool()?;
        let composition = decoder.get_string(MAX_COMPOSITION_BYTES)?;
        let commit = decoder.get_optional_string(MAX_COMMIT_BYTES)?;
        let candidate_count = usize::from(decoder.get_u16()?);
        if candidate_count > MAX_CANDIDATES {
            return Err(CodecError::TooManyItems {
                declared: candidate_count,
                maximum: MAX_CANDIDATES,
            });
        }
        let mut candidates = Vec::with_capacity(candidate_count);
        for _ in 0..candidate_count {
            candidates.push(decoder.get_string(MAX_CANDIDATE_BYTES)?);
        }
        decoder.finish()?;
        Ok(Self {
            revision,
            handled,
            composition,
            commit,
            candidates,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorMessage {
    pub code: u32,
    pub message: String,
}

impl PayloadCodec for ErrorMessage {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        let mut encoder = Encoder::new();
        encoder.put_u32(self.code);
        encoder.put_string(&self.message, MAX_ERROR_MESSAGE_BYTES)?;
        Ok(encoder.finish())
    }

    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let mut decoder = Decoder::new(payload);
        let error = Self {
            code: decoder.get_u32()?,
            message: decoder.get_string(MAX_ERROR_MESSAGE_BYTES)?,
        };
        decoder.finish()?;
        Ok(error)
    }
}

fn put_version(encoder: &mut Encoder, version: ProtocolVersion) {
    encoder.put_u16(version.major);
    encoder.put_u16(version.minor);
}

fn get_version(decoder: &mut Decoder<'_>) -> Result<ProtocolVersion, CodecError> {
    Ok(ProtocolVersion {
        major: decoder.get_u16()?,
        minor: decoder.get_u16()?,
    })
}

fn validate_hello(supported: VersionRange, max_payload_len: u32) -> Result<(), CodecError> {
    VersionRange::new(supported.minimum, supported.maximum)?;
    validate_payload_limit(max_payload_len)
}

fn validate_payload_limit(max_payload_len: u32) -> Result<(), CodecError> {
    if (max_payload_len as usize) < MIN_NEGOTIATED_PAYLOAD_LEN
        || max_payload_len as usize > MAX_PAYLOAD_LEN
    {
        return Err(CodecError::InvalidValue(
            "peer payload limit cannot carry every valid response or exceeds Mo's hard maximum",
        ));
    }
    Ok(())
}
