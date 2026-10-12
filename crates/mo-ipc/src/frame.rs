use std::fmt;
use std::io::{self, Read, Write};

use crate::ProtocolVersion;

// Field ranges of the fixed little-endian v1 header. Both directions use these
// names; independent wire fixtures continue to assert the literal byte layout.
mod header_fields {
    use std::ops::Range;
    pub const MAGIC: Range<usize> = 0..4;
    pub const HEADER_LENGTH: Range<usize> = 4..6;
    pub const VERSION_MAJOR: Range<usize> = 6..8;
    pub const VERSION_MINOR: Range<usize> = 8..10;
    pub const KIND: Range<usize> = 10..12;
    pub const FLAGS: Range<usize> = 12..16;
    pub const PAYLOAD_LENGTH: Range<usize> = 16..20;
    pub const PAYLOAD_CRC32: Range<usize> = 20..24;
    pub const CONNECTION_GENERATION: Range<usize> = 24..32;
    pub const SESSION_TOKEN: Range<usize> = 32..40;
    pub const REQUEST_ID: Range<usize> = 40..48;
}

pub const MAGIC: [u8; 4] = *b"MOIP";
pub const HEADER_LEN: usize = 48;
pub const MAX_PAYLOAD_LEN: usize = 64 * 1024;
pub const MAX_FRAME_LEN: usize = HEADER_LEN + MAX_PAYLOAD_LEN;

pub const FLAG_RESPONSE: u32 = 1 << 0;
pub const FLAG_ERROR: u32 = 1 << 1;
const KNOWN_FLAGS: u32 = FLAG_RESPONSE | FLAG_ERROR;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum MessageKind {
    Hello = 1,
    HelloAck = 2,
    OpenSession = 3,
    OpenSessionAck = 4,
    KeyEvent = 5,
    Snapshot = 6,
    CloseSession = 7,
    CloseSessionAck = 8,
    Error = 9,
    Ping = 10,
    Pong = 11,
    CandidateAction = 12,
    GetSettings = 13,
    SettingsSnapshot = 14,
    DetailedSnapshot = 15,
    PresentationSnapshot = 16,
    SymbolAction = 17,
    PresentationSettings = 18,
    SetEmoji = 19,
}

impl TryFrom<u16> for MessageKind {
    type Error = FrameError;

    fn try_from(value: u16) -> Result<Self, FrameError> {
        match value {
            tag if tag == Self::Hello as u16 => Ok(Self::Hello),
            tag if tag == Self::HelloAck as u16 => Ok(Self::HelloAck),
            tag if tag == Self::OpenSession as u16 => Ok(Self::OpenSession),
            tag if tag == Self::OpenSessionAck as u16 => Ok(Self::OpenSessionAck),
            tag if tag == Self::KeyEvent as u16 => Ok(Self::KeyEvent),
            tag if tag == Self::Snapshot as u16 => Ok(Self::Snapshot),
            tag if tag == Self::CloseSession as u16 => Ok(Self::CloseSession),
            tag if tag == Self::CloseSessionAck as u16 => Ok(Self::CloseSessionAck),
            tag if tag == Self::Error as u16 => Ok(Self::Error),
            tag if tag == Self::Ping as u16 => Ok(Self::Ping),
            tag if tag == Self::Pong as u16 => Ok(Self::Pong),
            tag if tag == Self::CandidateAction as u16 => Ok(Self::CandidateAction),
            tag if tag == Self::GetSettings as u16 => Ok(Self::GetSettings),
            tag if tag == Self::SettingsSnapshot as u16 => Ok(Self::SettingsSnapshot),
            tag if tag == Self::DetailedSnapshot as u16 => Ok(Self::DetailedSnapshot),
            tag if tag == Self::PresentationSnapshot as u16 => Ok(Self::PresentationSnapshot),
            tag if tag == Self::SymbolAction as u16 => Ok(Self::SymbolAction),
            tag if tag == Self::PresentationSettings as u16 => Ok(Self::PresentationSettings),
            tag if tag == Self::SetEmoji as u16 => Ok(Self::SetEmoji),
            other => Err(FrameError::UnknownMessageKind(other)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    pub version: ProtocolVersion,
    pub kind: MessageKind,
    pub flags: u32,
    pub payload_len: u32,
    pub payload_crc32: u32,
    pub connection_generation: u64,
    pub session_token: u64,
    pub request_id: u64,
}

impl FrameHeader {
    fn validate(&self) -> Result<(), FrameError> {
        let payload_len = self.payload_len as usize;
        if payload_len > MAX_PAYLOAD_LEN {
            return Err(FrameError::PayloadTooLarge {
                declared: payload_len,
                maximum: MAX_PAYLOAD_LEN,
            });
        }
        if self.flags & !KNOWN_FLAGS != 0 {
            return Err(FrameError::UnknownFlags(self.flags & !KNOWN_FLAGS));
        }
        Ok(())
    }

    fn encode_into(&self, output: &mut [u8; HEADER_LEN]) {
        output[header_fields::MAGIC].copy_from_slice(&MAGIC);
        output[header_fields::HEADER_LENGTH].copy_from_slice(&(HEADER_LEN as u16).to_le_bytes());
        output[header_fields::VERSION_MAJOR].copy_from_slice(&self.version.major.to_le_bytes());
        output[header_fields::VERSION_MINOR].copy_from_slice(&self.version.minor.to_le_bytes());
        output[header_fields::KIND].copy_from_slice(&(self.kind as u16).to_le_bytes());
        output[header_fields::FLAGS].copy_from_slice(&self.flags.to_le_bytes());
        output[header_fields::PAYLOAD_LENGTH].copy_from_slice(&self.payload_len.to_le_bytes());
        output[header_fields::PAYLOAD_CRC32].copy_from_slice(&self.payload_crc32.to_le_bytes());
        output[header_fields::CONNECTION_GENERATION]
            .copy_from_slice(&self.connection_generation.to_le_bytes());
        output[header_fields::SESSION_TOKEN].copy_from_slice(&self.session_token.to_le_bytes());
        output[header_fields::REQUEST_ID].copy_from_slice(&self.request_id.to_le_bytes());
    }

    fn decode(input: &[u8; HEADER_LEN]) -> Result<Self, FrameError> {
        let mut magic = [0_u8; 4];
        magic.copy_from_slice(&input[header_fields::MAGIC]);
        if magic != MAGIC {
            return Err(FrameError::InvalidMagic(magic));
        }

        let declared_header_len = u16::from_le_bytes(
            input[header_fields::HEADER_LENGTH]
                .try_into()
                .expect("fixed slice"),
        );
        if usize::from(declared_header_len) != HEADER_LEN {
            return Err(FrameError::InvalidHeaderLength(declared_header_len));
        }

        let header = Self {
            version: ProtocolVersion {
                major: u16::from_le_bytes(
                    input[header_fields::VERSION_MAJOR]
                        .try_into()
                        .expect("fixed slice"),
                ),
                minor: u16::from_le_bytes(
                    input[header_fields::VERSION_MINOR]
                        .try_into()
                        .expect("fixed slice"),
                ),
            },
            kind: MessageKind::try_from(u16::from_le_bytes(
                input[header_fields::KIND].try_into().expect("fixed slice"),
            ))?,
            flags: u32::from_le_bytes(input[header_fields::FLAGS].try_into().expect("fixed slice")),
            payload_len: u32::from_le_bytes(
                input[header_fields::PAYLOAD_LENGTH]
                    .try_into()
                    .expect("fixed slice"),
            ),
            payload_crc32: u32::from_le_bytes(
                input[header_fields::PAYLOAD_CRC32]
                    .try_into()
                    .expect("fixed slice"),
            ),
            connection_generation: u64::from_le_bytes(
                input[header_fields::CONNECTION_GENERATION]
                    .try_into()
                    .expect("fixed slice"),
            ),
            session_token: u64::from_le_bytes(
                input[header_fields::SESSION_TOKEN]
                    .try_into()
                    .expect("fixed slice"),
            ),
            request_id: u64::from_le_bytes(
                input[header_fields::REQUEST_ID]
                    .try_into()
                    .expect("fixed slice"),
            ),
        };
        header.validate()?;
        Ok(header)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

impl Frame {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        version: ProtocolVersion,
        kind: MessageKind,
        flags: u32,
        connection_generation: u64,
        session_token: u64,
        request_id: u64,
        payload: Vec<u8>,
    ) -> Result<Self, FrameError> {
        if payload.len() > MAX_PAYLOAD_LEN {
            return Err(FrameError::PayloadTooLarge {
                declared: payload.len(),
                maximum: MAX_PAYLOAD_LEN,
            });
        }
        let payload_len =
            u32::try_from(payload.len()).map_err(|_| FrameError::PayloadTooLarge {
                declared: payload.len(),
                maximum: MAX_PAYLOAD_LEN,
            })?;
        let header = FrameHeader {
            version,
            kind,
            flags,
            payload_len,
            payload_crc32: crc32(&payload),
            connection_generation,
            session_token,
            request_id,
        };
        header.validate()?;
        Ok(Self { header, payload })
    }

    pub fn encode(&self) -> Result<Vec<u8>, FrameError> {
        validate_frame(self)?;
        let mut encoded = Vec::with_capacity(HEADER_LEN + self.payload.len());
        let mut header = [0_u8; HEADER_LEN];
        self.header.encode_into(&mut header);
        encoded.extend_from_slice(&header);
        encoded.extend_from_slice(&self.payload);
        Ok(encoded)
    }

    pub fn decode(input: &[u8]) -> Result<Self, FrameError> {
        if input.len() < HEADER_LEN {
            return Err(FrameError::Truncated {
                expected: HEADER_LEN,
                actual: input.len(),
            });
        }
        let header_bytes: &[u8; HEADER_LEN] =
            input[..HEADER_LEN].try_into().expect("length checked");
        let header = FrameHeader::decode(header_bytes)?;
        let total_len = HEADER_LEN
            .checked_add(header.payload_len as usize)
            .ok_or(FrameError::LengthOverflow)?;
        if input.len() < total_len {
            return Err(FrameError::Truncated {
                expected: total_len,
                actual: input.len(),
            });
        }
        if input.len() > total_len {
            return Err(FrameError::TrailingBytes(input.len() - total_len));
        }
        let payload = input[HEADER_LEN..].to_vec();
        verify_checksum(&header, &payload)?;
        Ok(Self { header, payload })
    }
}

pub fn read_frame<R: Read>(reader: &mut R) -> Result<Frame, FrameError> {
    let mut header_bytes = [0_u8; HEADER_LEN];
    reader
        .read_exact(&mut header_bytes)
        .map_err(FrameError::Io)?;
    let header = FrameHeader::decode(&header_bytes)?;

    // The bound is checked by FrameHeader::decode before this allocation.
    let mut payload = vec![0_u8; header.payload_len as usize];
    reader.read_exact(&mut payload).map_err(FrameError::Io)?;
    verify_checksum(&header, &payload)?;
    Ok(Frame { header, payload })
}

pub fn write_frame<W: Write>(writer: &mut W, frame: &Frame) -> Result<(), FrameError> {
    validate_frame(frame)?;
    let mut header_bytes = [0_u8; HEADER_LEN];
    frame.header.encode_into(&mut header_bytes);
    writer.write_all(&header_bytes).map_err(FrameError::Io)?;
    writer.write_all(&frame.payload).map_err(FrameError::Io)?;
    writer.flush().map_err(FrameError::Io)
}

fn validate_frame(frame: &Frame) -> Result<(), FrameError> {
    frame.header.validate()?;
    let declared = frame.header.payload_len as usize;
    if declared != frame.payload.len() {
        return Err(FrameError::PayloadLengthMismatch {
            declared,
            actual: frame.payload.len(),
        });
    }
    verify_checksum(&frame.header, &frame.payload)
}

fn verify_checksum(header: &FrameHeader, payload: &[u8]) -> Result<(), FrameError> {
    let actual = crc32(payload);
    if header.payload_crc32 != actual {
        return Err(FrameError::ChecksumMismatch {
            expected: header.payload_crc32,
            actual,
        });
    }
    Ok(())
}

fn crc32(bytes: &[u8]) -> u32 {
    const CRC32_IEEE_REFLECTED_POLYNOMIAL: u32 = 0xedb8_8320;
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..u8::BITS {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (CRC32_IEEE_REFLECTED_POLYNOMIAL & mask);
        }
    }
    !crc
}

#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    InvalidMagic([u8; 4]),
    InvalidHeaderLength(u16),
    UnknownMessageKind(u16),
    UnknownFlags(u32),
    PayloadTooLarge { declared: usize, maximum: usize },
    PayloadLengthMismatch { declared: usize, actual: usize },
    ChecksumMismatch { expected: u32, actual: u32 },
    Truncated { expected: usize, actual: usize },
    TrailingBytes(usize),
    LengthOverflow,
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "IPC I/O failed: {error}"),
            Self::InvalidMagic(actual) => write!(formatter, "invalid IPC magic: {actual:02x?}"),
            Self::InvalidHeaderLength(actual) => {
                write!(formatter, "invalid IPC header length {actual}")
            }
            Self::UnknownMessageKind(kind) => write!(formatter, "unknown message kind {kind}"),
            Self::UnknownFlags(flags) => write!(formatter, "unknown frame flags 0x{flags:08x}"),
            Self::PayloadTooLarge { declared, maximum } => {
                write!(
                    formatter,
                    "payload length {declared} exceeds maximum {maximum}"
                )
            }
            Self::PayloadLengthMismatch { declared, actual } => write!(
                formatter,
                "payload length mismatch: header says {declared}, actual is {actual}"
            ),
            Self::ChecksumMismatch { expected, actual } => write!(
                formatter,
                "payload checksum mismatch: expected 0x{expected:08x}, got 0x{actual:08x}"
            ),
            Self::Truncated { expected, actual } => {
                write!(
                    formatter,
                    "truncated frame: expected {expected} bytes, got {actual}"
                )
            }
            Self::TrailingBytes(count) => write!(formatter, "frame has {count} trailing bytes"),
            Self::LengthOverflow => formatter.write_str("frame length overflow"),
        }
    }
}

impl std::error::Error for FrameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}
