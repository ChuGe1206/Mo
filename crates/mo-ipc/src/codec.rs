use std::fmt;

pub trait PayloadCodec: Sized {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError>;
    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError>;
}

#[derive(Debug, Default)]
pub struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn put_bool(&mut self, value: bool) {
        self.put_u8(u8::from(value));
    }

    pub fn put_u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn put_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn put_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn put_string(&mut self, value: &str, maximum: usize) -> Result<(), CodecError> {
        let bytes = value.as_bytes();
        if bytes.len() > maximum {
            return Err(CodecError::StringTooLong {
                declared: bytes.len(),
                maximum,
            });
        }
        let length = u32::try_from(bytes.len()).map_err(|_| CodecError::IntegerOverflow)?;
        self.put_u32(length);
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    pub fn put_optional_string(
        &mut self,
        value: Option<&str>,
        maximum: usize,
    ) -> Result<(), CodecError> {
        self.put_bool(value.is_some());
        if let Some(value) = value {
            self.put_string(value, maximum)?;
        }
        Ok(())
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Debug)]
pub struct Decoder<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    pub fn get_u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.take(1)?[0])
    }

    pub fn get_bool(&mut self) -> Result<bool, CodecError> {
        match self.get_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(CodecError::InvalidBoolean(other)),
        }
    }

    pub fn get_u16(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("fixed slice"),
        ))
    }

    pub fn get_u32(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("fixed slice"),
        ))
    }

    pub fn get_u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("fixed slice"),
        ))
    }

    pub fn get_string(&mut self, maximum: usize) -> Result<String, CodecError> {
        let declared = self.get_u32()? as usize;
        if declared > maximum {
            return Err(CodecError::StringTooLong { declared, maximum });
        }
        let bytes = self.take(declared)?;
        let value = std::str::from_utf8(bytes).map_err(CodecError::InvalidUtf8)?;
        Ok(value.to_owned())
    }

    pub fn get_optional_string(&mut self, maximum: usize) -> Result<Option<String>, CodecError> {
        if self.get_bool()? {
            self.get_string(maximum).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn finish(self) -> Result<(), CodecError> {
        let remaining = self.input.len() - self.offset;
        if remaining == 0 {
            Ok(())
        } else {
            Err(CodecError::TrailingBytes(remaining))
        }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], CodecError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(CodecError::IntegerOverflow)?;
        if end > self.input.len() {
            return Err(CodecError::UnexpectedEnd {
                offset: self.offset,
                needed: count,
                remaining: self.input.len().saturating_sub(self.offset),
            });
        }
        let bytes = &self.input[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }
}

#[derive(Debug)]
pub enum CodecError {
    UnexpectedEnd {
        offset: usize,
        needed: usize,
        remaining: usize,
    },
    InvalidBoolean(u8),
    InvalidUtf8(std::str::Utf8Error),
    StringTooLong {
        declared: usize,
        maximum: usize,
    },
    TooManyItems {
        declared: usize,
        maximum: usize,
    },
    TrailingBytes(usize),
    InvalidValue(&'static str),
    IntegerOverflow,
}

impl fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd {
                offset,
                needed,
                remaining,
            } => write!(
                formatter,
                "payload ends at offset {offset}: need {needed} bytes, only {remaining} remain"
            ),
            Self::InvalidBoolean(value) => write!(formatter, "invalid boolean value {value}"),
            Self::InvalidUtf8(error) => write!(formatter, "invalid UTF-8 payload: {error}"),
            Self::StringTooLong { declared, maximum } => {
                write!(
                    formatter,
                    "string length {declared} exceeds maximum {maximum}"
                )
            }
            Self::TooManyItems { declared, maximum } => {
                write!(formatter, "item count {declared} exceeds maximum {maximum}")
            }
            Self::TrailingBytes(count) => write!(formatter, "payload has {count} trailing bytes"),
            Self::InvalidValue(message) => formatter.write_str(message),
            Self::IntegerOverflow => formatter.write_str("integer overflow while coding payload"),
        }
    }
}

impl std::error::Error for CodecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidUtf8(error) => Some(error),
            _ => None,
        }
    }
}
