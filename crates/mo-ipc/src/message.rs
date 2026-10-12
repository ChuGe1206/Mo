use crate::{
    CodecError, Decoder, Encoder, MAX_PAYLOAD_LEN, PayloadCodec, ProtocolVersion, VersionRange,
};

pub const FEATURE_KEY_EVENTS: u64 = 1 << 0;
pub const FEATURE_CANDIDATE_ACTIONS: u64 = 1 << 1;
pub const FEATURE_SETTINGS_SNAPSHOT: u64 = 1 << 2;
pub const FEATURE_SESSION_SETTINGS_ACK: u64 = 1 << 3;
pub const FEATURE_CANDIDATE_DETAILS: u64 = 1 << 4;
pub const FEATURE_CANDIDATE_PRESENTATION: u64 = 1 << 5;
pub const FEATURE_SYMBOL_ACTIONS: u64 = 1 << 6;
/// Page presence, u32 page number, previous/next flags, and u8 highlight ordinal.
pub const PRESENTATION_PAGE_HEADER_LEN: usize = 8;
/// Revision plus ten one-byte settings fields in wire format v1.
pub const SETTINGS_SNAPSHOT_PAYLOAD_LEN: usize = 18;
/// Wire format v1 sentinel for an absent highlighted candidate.
const NO_HIGHLIGHTED_CANDIDATE: u8 = u8::MAX;
// These are wire contracts, kept independent of the settings crate. Broker tests
// compare their acceptance boundaries with persisted settings and UI limits.
pub const MIN_CANDIDATE_PAGE_SIZE: u8 = 3;
/// Inclusive maximum candidates per configured page.
pub const MAX_CANDIDATE_PAGE_SIZE: u8 = 9;
pub const MIN_CANDIDATE_RADIUS_PX: u8 = 8;
/// Inclusive maximum corner radius in logical pixels.
pub const MAX_CANDIDATE_RADIUS_PX: u8 = 20;
pub const MIN_CANDIDATE_FONT_SIZE_PX: u8 = 12;
/// Inclusive maximum candidate font size in logical pixels.
pub const MAX_CANDIDATE_FONT_SIZE_PX: u8 = 24;
pub const MAX_RGB_COLOR: u32 = 0x00ff_ffff;
pub const MAX_CANDIDATE_SKIN_ID: u8 = 6;
pub const MIN_PRESENTATION_SNAPSHOT_PAYLOAD_LEN: usize =
    MIN_DETAILED_SNAPSHOT_PAYLOAD_LEN + PRESENTATION_PAGE_HEADER_LEN;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettingsOrigin {
    Defaults = 0,
    Stored = 1,
}
impl TryFrom<u8> for SettingsOrigin {
    type Error = CodecError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            tag if tag == Self::Defaults as u8 => Ok(Self::Defaults),
            tag if tag == Self::Stored as u8 => Ok(Self::Stored),
            _ => Err(CodecError::InvalidValue("unknown settings origin")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum InputScheme {
    FullPinyin = 0,
    DoublePinyinNatural = 1,
    DoublePinyinFlypy = 2,
    DoublePinyinMicrosoft = 3,
    DoublePinyinSogou = 4,
}
impl TryFrom<u8> for InputScheme {
    type Error = CodecError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            tag if tag == Self::FullPinyin as u8 => Ok(Self::FullPinyin),
            tag if tag == Self::DoublePinyinNatural as u8 => Ok(Self::DoublePinyinNatural),
            tag if tag == Self::DoublePinyinFlypy as u8 => Ok(Self::DoublePinyinFlypy),
            tag if tag == Self::DoublePinyinMicrosoft as u8 => Ok(Self::DoublePinyinMicrosoft),
            tag if tag == Self::DoublePinyinSogou as u8 => Ok(Self::DoublePinyinSogou),
            _ => Err(CodecError::InvalidValue("unknown input scheme")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CharacterSet {
    Simplified = 0,
    Traditional = 1,
}
impl TryFrom<u8> for CharacterSet {
    type Error = CodecError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            tag if tag == Self::Simplified as u8 => Ok(Self::Simplified),
            tag if tag == Self::Traditional as u8 => Ok(Self::Traditional),
            _ => Err(CodecError::InvalidValue("unknown character set")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Theme {
    System = 0,
    Light = 1,
    Dark = 2,
}
impl TryFrom<u8> for Theme {
    type Error = CodecError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            tag if tag == Self::System as u8 => Ok(Self::System),
            tag if tag == Self::Light as u8 => Ok(Self::Light),
            tag if tag == Self::Dark as u8 => Ok(Self::Dark),
            _ => Err(CodecError::InvalidValue("unknown settings theme")),
        }
    }
}

/// Fixed, bounded projection of Mo settings. Engine preferences are included
/// as desired state; this payload does not claim that librime applied them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsSnapshot {
    pub revision: u64,
    pub origin: SettingsOrigin,
    pub input_scheme: InputScheme,
    pub character_set: CharacterSet,
    pub candidate_page_size: u8,
    pub theme: Theme,
    pub show_comments: bool,
    pub emoji: bool,
    pub local_learning: bool,
    pub privacy_mode: bool,
    pub effective_learning: bool,
}

impl PayloadCodec for SettingsSnapshot {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        self.validate()?;
        let mut encoder = Encoder::new();
        encoder.put_u64(self.revision);
        encoder.put_u8(self.origin as u8);
        encoder.put_u8(self.input_scheme as u8);
        encoder.put_u8(self.character_set as u8);
        encoder.put_u8(self.candidate_page_size);
        encoder.put_u8(self.theme as u8);
        encoder.put_bool(self.show_comments);
        encoder.put_bool(self.emoji);
        encoder.put_bool(self.local_learning);
        encoder.put_bool(self.privacy_mode);
        encoder.put_bool(self.effective_learning);
        Ok(encoder.finish())
    }

    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let mut decoder = Decoder::new(payload);
        let value = Self {
            revision: decoder.get_u64()?,
            origin: SettingsOrigin::try_from(decoder.get_u8()?)?,
            input_scheme: InputScheme::try_from(decoder.get_u8()?)?,
            character_set: CharacterSet::try_from(decoder.get_u8()?)?,
            candidate_page_size: decoder.get_u8()?,
            theme: Theme::try_from(decoder.get_u8()?)?,
            show_comments: decoder.get_bool()?,
            emoji: decoder.get_bool()?,
            local_learning: decoder.get_bool()?,
            privacy_mode: decoder.get_bool()?,
            effective_learning: decoder.get_bool()?,
        };
        decoder.finish()?;
        value.validate()?;
        Ok(value)
    }
}

impl SettingsSnapshot {
    fn validate(&self) -> Result<(), CodecError> {
        if self.revision == 0 {
            return Err(CodecError::InvalidValue(
                "settings revision must be non-zero",
            ));
        }
        if !(MIN_CANDIDATE_PAGE_SIZE..=MAX_CANDIDATE_PAGE_SIZE).contains(&self.candidate_page_size)
        {
            return Err(CodecError::InvalidValue(
                "candidate page size is outside the product range",
            ));
        }
        if self.effective_learning != (self.local_learning && !self.privacy_mode) {
            return Err(CodecError::InvalidValue(
                "effective learning does not match the privacy policy",
            ));
        }
        Ok(())
    }
}

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
#[repr(u8)]
pub enum CandidateActionKind {
    Select = 0,
    PreviousPage = 1,
    NextPage = 2,
}
impl TryFrom<u8> for CandidateActionKind {
    type Error = CodecError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            tag if tag == Self::Select as u8 => Ok(Self::Select),
            tag if tag == Self::PreviousPage as u8 => Ok(Self::PreviousPage),
            tag if tag == Self::NextPage as u8 => Ok(Self::NextPage),
            _ => Err(CodecError::InvalidValue("unknown candidate action")),
        }
    }
}

impl PayloadCodec for CandidateAction {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        self.validate()?;
        let mut encoder = Encoder::new();
        encoder.put_u64(self.expected_revision);
        encoder.put_u8(self.action as u8);
        encoder.put_u32(self.index);
        Ok(encoder.finish())
    }

    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let mut decoder = Decoder::new(payload);
        let expected_revision = decoder.get_u64()?;
        let action = CandidateActionKind::try_from(decoder.get_u8()?)?;
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
pub const MAX_CANDIDATE_COMMENT_BYTES: usize = 256;
pub const MAX_CANDIDATE_LABEL_BYTES: usize = 32;
pub const MAX_ERROR_MESSAGE_BYTES: usize = 512;

// Largest valid Snapshot payload:
// revision + handled + composition + optional commit + candidate count +
// every length-prefixed candidate. A peer advertising less cannot safely
// receive every response permitted by this protocol version.
pub const MIN_NEGOTIATED_PAYLOAD_LEN: usize = std::mem::size_of::<u64>()
    + std::mem::size_of::<u8>()
    + (std::mem::size_of::<u32>() + MAX_COMPOSITION_BYTES)
    + (std::mem::size_of::<u8>() + std::mem::size_of::<u32>() + MAX_COMMIT_BYTES)
    + std::mem::size_of::<u16>()
    + MAX_CANDIDATES * (std::mem::size_of::<u32>() + MAX_CANDIDATE_BYTES);

pub const MIN_DETAILED_SNAPSHOT_PAYLOAD_LEN: usize = MIN_NEGOTIATED_PAYLOAD_LEN
    + MAX_CANDIDATES
        * ((std::mem::size_of::<u8>() + std::mem::size_of::<u32>() + MAX_CANDIDATE_COMMENT_BYTES)
            + (std::mem::size_of::<u8>() + std::mem::size_of::<u32>() + MAX_CANDIDATE_LABEL_BYTES));

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

/// Rich candidate page, sent only when FEATURE_CANDIDATE_DETAILS was negotiated.
/// The legacy Snapshot kind and payload remain byte-for-byte unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetailedCandidate {
    pub text: String,
    pub comment: Option<String>,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetailedSnapshot {
    pub revision: u64,
    pub handled: bool,
    pub composition: String,
    pub commit: Option<String>,
    pub candidates: Vec<DetailedCandidate>,
}

impl PayloadCodec for DetailedSnapshot {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        if self.candidates.len() > MAX_CANDIDATES {
            return Err(CodecError::TooManyItems {
                declared: self.candidates.len(),
                maximum: MAX_CANDIDATES,
            });
        }
        let mut encoder = Encoder::new();
        encoder.put_u64(self.revision);
        encoder.put_bool(self.handled);
        encoder.put_string(&self.composition, MAX_COMPOSITION_BYTES)?;
        encoder.put_optional_string(self.commit.as_deref(), MAX_COMMIT_BYTES)?;
        encoder.put_u16(self.candidates.len() as u16);
        for candidate in &self.candidates {
            encoder.put_string(&candidate.text, MAX_CANDIDATE_BYTES)?;
            encoder
                .put_optional_string(candidate.comment.as_deref(), MAX_CANDIDATE_COMMENT_BYTES)?;
            encoder.put_optional_string(candidate.label.as_deref(), MAX_CANDIDATE_LABEL_BYTES)?;
        }
        Ok(encoder.finish())
    }

    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let mut decoder = Decoder::new(payload);
        let revision = decoder.get_u64()?;
        let handled = decoder.get_bool()?;
        let composition = decoder.get_string(MAX_COMPOSITION_BYTES)?;
        let commit = decoder.get_optional_string(MAX_COMMIT_BYTES)?;
        let count = usize::from(decoder.get_u16()?);
        if count > MAX_CANDIDATES {
            return Err(CodecError::TooManyItems {
                declared: count,
                maximum: MAX_CANDIDATES,
            });
        }
        let mut candidates = Vec::with_capacity(count);
        for _ in 0..count {
            candidates.push(DetailedCandidate {
                text: decoder.get_string(MAX_CANDIDATE_BYTES)?,
                comment: decoder.get_optional_string(MAX_CANDIDATE_COMMENT_BYTES)?,
                label: decoder.get_optional_string(MAX_CANDIDATE_LABEL_BYTES)?,
            });
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

/// Exact paging facts alongside an unchanged detailed snapshot body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidatePage {
    pub number: u32,
    pub has_previous: bool,
    pub has_next: bool,
    pub highlighted: Option<u8>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationSnapshot {
    pub snapshot: DetailedSnapshot,
    pub page: Option<CandidatePage>,
}
impl PresentationSnapshot {
    fn validate(&self) -> Result<(), CodecError> {
        if self.snapshot.revision == 0 {
            return Err(CodecError::InvalidValue(
                "presentation revision must be non-zero",
            ));
        }
        if let Some(page) = self.page {
            if self.snapshot.composition.is_empty()
                || self.snapshot.candidates.is_empty()
                || page.has_previous != (page.number > 0)
                || page
                    .highlighted
                    .is_some_and(|i| i as usize >= self.snapshot.candidates.len())
            {
                return Err(CodecError::InvalidValue(
                    "inconsistent candidate presentation",
                ));
            }
        }
        Ok(())
    }
}
impl PayloadCodec for PresentationSnapshot {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        self.validate()?;
        let mut encoder = Encoder::new();
        encoder.put_bool(self.page.is_some());
        let page = self.page.unwrap_or(CandidatePage {
            number: 0,
            has_previous: false,
            has_next: false,
            highlighted: None,
        });
        encoder.put_u32(page.number);
        encoder.put_bool(page.has_previous);
        encoder.put_bool(page.has_next);
        encoder.put_u8(page.highlighted.unwrap_or(NO_HIGHLIGHTED_CANDIDATE));
        let mut bytes = encoder.finish();
        bytes.extend(self.snapshot.encode_payload()?);
        Ok(bytes)
    }
    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let mut decoder = Decoder::new(
            payload
                .get(..PRESENTATION_PAGE_HEADER_LEN)
                .ok_or(CodecError::InvalidValue("missing page header"))?,
        );
        let present = decoder.get_bool()?;
        let number = decoder.get_u32()?;
        let has_previous = decoder.get_bool()?;
        let has_next = decoder.get_bool()?;
        let index = decoder.get_u8()?;
        decoder.finish()?;
        if !present
            && (number != 0 || has_previous || has_next || index != NO_HIGHLIGHTED_CANDIDATE)
        {
            return Err(CodecError::InvalidValue("noncanonical absent page"));
        }
        let value = Self {
            snapshot: DetailedSnapshot::decode_payload(&payload[PRESENTATION_PAGE_HEADER_LEN..])?,
            page: present.then_some(CandidatePage {
                number,
                has_previous,
                has_next,
                highlighted: (index != NO_HIGHLIGHTED_CANDIDATE).then_some(index),
            }),
        };
        value.validate()?;
        Ok(value)
    }
}
/// Catalog v1 insertion. The broker validates the catalog ordinal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SymbolAction {
    pub expected_revision: u64,
    pub index: u32,
}
impl PayloadCodec for SymbolAction {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        if self.expected_revision == 0 {
            return Err(CodecError::InvalidValue("symbol revision must be non-zero"));
        }
        let mut encoder = Encoder::new();
        encoder.put_u64(self.expected_revision);
        encoder.put_u32(self.index);
        Ok(encoder.finish())
    }
    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let mut decoder = Decoder::new(payload);
        let value = Self {
            expected_revision: decoder.get_u64()?,
            index: decoder.get_u32()?,
        };
        decoder.finish()?;
        value.encode_payload()?;
        Ok(value)
    }
}

pub const FEATURE_PRESENTATION_SETTINGS: u64 = 1 << 7;
pub const FEATURE_QUICK_SETTINGS: u64 = 1 << 8;
/// Bounded appearance projection appended to the untouched legacy settings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationSettings {
    pub settings: SettingsSnapshot,
    pub skin: u8,
    pub background: u32,
    pub accent: u32,
    pub text: u32,
    pub radius: u8,
    pub font_size: u8,
    pub vertical: bool,
}
impl PayloadCodec for PresentationSettings {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        if self.skin > MAX_CANDIDATE_SKIN_ID
            || self.background > MAX_RGB_COLOR
            || self.accent > MAX_RGB_COLOR
            || self.text > MAX_RGB_COLOR
            || !(MIN_CANDIDATE_RADIUS_PX..=MAX_CANDIDATE_RADIUS_PX).contains(&self.radius)
            || !(MIN_CANDIDATE_FONT_SIZE_PX..=MAX_CANDIDATE_FONT_SIZE_PX).contains(&self.font_size)
        {
            return Err(CodecError::InvalidValue("invalid appearance settings"));
        }
        let mut bytes = self.settings.encode_payload()?;
        let mut encoder = Encoder::new();
        encoder.put_u8(self.skin);
        encoder.put_u32(self.background);
        encoder.put_u32(self.accent);
        encoder.put_u32(self.text);
        encoder.put_u8(self.radius);
        encoder.put_u8(self.font_size);
        encoder.put_bool(self.vertical);
        bytes.extend(encoder.finish());
        Ok(bytes)
    }
    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let settings = SettingsSnapshot::decode_payload(
            payload
                .get(..SETTINGS_SNAPSHOT_PAYLOAD_LEN)
                .ok_or(CodecError::InvalidValue("missing settings"))?,
        )?;
        let mut decoder = Decoder::new(&payload[SETTINGS_SNAPSHOT_PAYLOAD_LEN..]);
        let value = Self {
            settings,
            skin: decoder.get_u8()?,
            background: decoder.get_u32()?,
            accent: decoder.get_u32()?,
            text: decoder.get_u32()?,
            radius: decoder.get_u8()?,
            font_size: decoder.get_u8()?,
            vertical: decoder.get_bool()?,
        };
        decoder.finish()?;
        value.encode_payload()?;
        Ok(value)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SetEmoji {
    pub expected_revision: u64,
    pub expected_settings_revision: u64,
    pub value: bool,
}
impl PayloadCodec for SetEmoji {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        if self.expected_revision == 0 || self.expected_settings_revision == 0 {
            return Err(CodecError::InvalidValue("zero settings action revision"));
        }
        let mut encoder = Encoder::new();
        encoder.put_u64(self.expected_revision);
        encoder.put_u64(self.expected_settings_revision);
        encoder.put_bool(self.value);
        Ok(encoder.finish())
    }
    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let mut decoder = Decoder::new(payload);
        let value = Self {
            expected_revision: decoder.get_u64()?,
            expected_settings_revision: decoder.get_u64()?,
            value: decoder.get_bool()?,
        };
        decoder.finish()?;
        value.encode_payload()?;
        Ok(value)
    }
}
