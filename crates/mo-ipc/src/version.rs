use crate::CodecError;

pub const CURRENT_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VersionRange {
    pub minimum: ProtocolVersion,
    pub maximum: ProtocolVersion,
}

impl VersionRange {
    pub fn new(minimum: ProtocolVersion, maximum: ProtocolVersion) -> Result<Self, CodecError> {
        if minimum.major != maximum.major {
            return Err(CodecError::InvalidValue(
                "a version range may not span protocol major versions",
            ));
        }
        if minimum > maximum {
            return Err(CodecError::InvalidValue(
                "minimum protocol version exceeds maximum",
            ));
        }
        Ok(Self { minimum, maximum })
    }
}

pub fn negotiate_version(local: VersionRange, peer: VersionRange) -> Option<ProtocolVersion> {
    if local.minimum.major != peer.minimum.major {
        return None;
    }
    let minimum = local.minimum.max(peer.minimum);
    let maximum = local.maximum.min(peer.maximum);
    (minimum <= maximum).then_some(maximum)
}
