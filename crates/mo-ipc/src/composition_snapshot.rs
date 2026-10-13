//! Negotiated preedit positions; older snapshot encodings remain unchanged.
use crate::{
    CodecError, Decoder, Encoder, MIN_PRESENTATION_SNAPSHOT_PAYLOAD_LEN, PayloadCodec,
    PresentationSnapshot,
};

pub const FEATURE_COMPOSITION_POSITIONS: u64 = 1 << 9;
/// Presence byte plus three little-endian UTF-8 byte offsets.
pub const COMPOSITION_POSITION_HEADER_BYTES: usize = 1 + 3 * size_of::<u32>();
pub const MIN_COMPOSITION_SNAPSHOT_PAYLOAD_LEN: usize =
    MIN_PRESENTATION_SNAPSHOT_PAYLOAD_LEN + COMPOSITION_POSITION_HEADER_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionPositions {
    pub cursor_utf8_bytes: u32,
    pub selection_start_utf8_bytes: u32,
    pub selection_end_utf8_bytes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionSnapshot {
    pub presentation: PresentationSnapshot,
    pub positions: Option<CompositionPositions>,
}
impl CompositionSnapshot {
    fn validate(&self) -> Result<(), CodecError> {
        let text = &self.presentation.snapshot.composition;
        if text.is_empty() != self.positions.is_none() {
            return Err(CodecError::InvalidValue(
                "preedit position presence mismatch",
            ));
        }
        if let Some(p) = self.positions {
            if p.selection_start_utf8_bytes > p.selection_end_utf8_bytes
                || [
                    p.cursor_utf8_bytes,
                    p.selection_start_utf8_bytes,
                    p.selection_end_utf8_bytes,
                ]
                .iter()
                .any(|offset| !text.is_char_boundary(*offset as usize))
            {
                return Err(CodecError::InvalidValue("invalid UTF-8 preedit positions"));
            }
        }
        Ok(())
    }
}
impl PayloadCodec for CompositionSnapshot {
    fn encode_payload(&self) -> Result<Vec<u8>, CodecError> {
        self.validate()?;
        let body = self.presentation.encode_payload()?;
        let mut encoder = Encoder::new();
        encoder.put_bool(self.positions.is_some());
        let p = self.positions.unwrap_or(CompositionPositions {
            cursor_utf8_bytes: 0,
            selection_start_utf8_bytes: 0,
            selection_end_utf8_bytes: 0,
        });
        encoder.put_u32(p.cursor_utf8_bytes);
        encoder.put_u32(p.selection_start_utf8_bytes);
        encoder.put_u32(p.selection_end_utf8_bytes);
        let mut bytes = encoder.finish();
        bytes.extend(body);
        Ok(bytes)
    }
    fn decode_payload(payload: &[u8]) -> Result<Self, CodecError> {
        let header = payload
            .get(..COMPOSITION_POSITION_HEADER_BYTES)
            .ok_or(CodecError::InvalidValue("missing preedit position header"))?;
        let mut decoder = Decoder::new(header);
        let present = decoder.get_bool()?;
        let p = CompositionPositions {
            cursor_utf8_bytes: decoder.get_u32()?,
            selection_start_utf8_bytes: decoder.get_u32()?,
            selection_end_utf8_bytes: decoder.get_u32()?,
        };
        decoder.finish()?;
        if !present
            && p != (CompositionPositions {
                cursor_utf8_bytes: 0,
                selection_start_utf8_bytes: 0,
                selection_end_utf8_bytes: 0,
            })
        {
            return Err(CodecError::InvalidValue(
                "noncanonical absent preedit positions",
            ));
        }
        let value = Self {
            positions: present.then_some(p),
            presentation: PresentationSnapshot::decode_payload(
                &payload[COMPOSITION_POSITION_HEADER_BYTES..],
            )?,
        };
        value.validate()?;
        Ok(value)
    }
}
