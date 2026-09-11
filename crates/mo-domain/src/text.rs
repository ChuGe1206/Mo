use core::fmt;

macro_rules! offset_type {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(usize);

        impl $name {
            /// Constructs an offset from its scalar representation.
            #[must_use]
            pub const fn new(value: usize) -> Self {
                Self(value)
            }

            /// Returns the scalar representation of this offset.
            #[must_use]
            pub const fn get(self) -> usize {
                self.0
            }
        }

        impl From<usize> for $name {
            fn from(value: usize) -> Self {
                Self(value)
            }
        }

        impl From<$name> for usize {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

offset_type!(Utf8ByteOffset, "An offset measured in UTF-8 bytes.");
offset_type!(
    Utf16CodeUnitOffset,
    "An offset measured in UTF-16 code units."
);
offset_type!(
    UnicodeScalarIndex,
    "An index measured in Unicode scalar values."
);
offset_type!(
    GraphemeClusterIndex,
    "An index measured in user-perceived grapheme clusters."
);

/// A half-open range whose unit is carried by the offset type.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextRange<T> {
    /// Inclusive start offset.
    pub start: T,
    /// Exclusive end offset.
    pub end: T,
}

impl<T> TextRange<T> {
    /// Constructs a half-open text range.
    #[must_use]
    pub const fn new(start: T, end: T) -> Self {
        Self { start, end }
    }
}

/// Invalid UTF-8 byte offsets supplied for a composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositionError {
    /// Selection start is after selection end.
    InvertedSelection {
        /// Selection start in UTF-8 bytes.
        start: Utf8ByteOffset,
        /// Selection end in UTF-8 bytes.
        end: Utf8ByteOffset,
    },
    /// An offset is beyond the end of the preedit string.
    OutOfBounds {
        /// Invalid UTF-8 byte offset.
        offset: Utf8ByteOffset,
        /// Preedit length in UTF-8 bytes.
        len: Utf8ByteOffset,
    },
    /// An offset falls in the middle of a UTF-8 code point.
    NotCharBoundary {
        /// Invalid UTF-8 byte offset.
        offset: Utf8ByteOffset,
    },
}

impl fmt::Display for CompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::InvertedSelection { start, end } => write!(
                formatter,
                "composition selection is inverted: {}..{}",
                start.get(),
                end.get()
            ),
            Self::OutOfBounds { offset, len } => write!(
                formatter,
                "composition offset {} exceeds UTF-8 length {}",
                offset.get(),
                len.get()
            ),
            Self::NotCharBoundary { offset } => write!(
                formatter,
                "composition offset {} is not a UTF-8 character boundary",
                offset.get()
            ),
        }
    }
}

impl std::error::Error for CompositionError {}

/// The engine's preedit text and its UTF-8-based cursor/selection metadata.
///
/// Platform adapters must perform checked conversion to native units such as
/// UTF-16 code units before calling an operating-system text API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Composition {
    preedit: String,
    cursor: Utf8ByteOffset,
    selection: TextRange<Utf8ByteOffset>,
}

impl Composition {
    /// Builds a composition after validating every UTF-8 byte offset.
    pub fn try_new(
        preedit: impl Into<String>,
        cursor: Utf8ByteOffset,
        selection: TextRange<Utf8ByteOffset>,
    ) -> Result<Self, CompositionError> {
        let preedit = preedit.into();
        let len = Utf8ByteOffset::new(preedit.len());

        if selection.start > selection.end {
            return Err(CompositionError::InvertedSelection {
                start: selection.start,
                end: selection.end,
            });
        }

        for offset in [cursor, selection.start, selection.end] {
            if offset > len {
                return Err(CompositionError::OutOfBounds { offset, len });
            }
            if !preedit.is_char_boundary(offset.get()) {
                return Err(CompositionError::NotCharBoundary { offset });
            }
        }

        Ok(Self {
            preedit,
            cursor,
            selection,
        })
    }

    /// Builds a composition with its cursor and empty selection at the end.
    #[must_use]
    pub fn from_preedit(preedit: impl Into<String>) -> Self {
        let preedit = preedit.into();
        let end = Utf8ByteOffset::new(preedit.len());
        Self {
            preedit,
            cursor: end,
            selection: TextRange::new(end, end),
        }
    }

    /// Returns the preedit string.
    #[must_use]
    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    /// Returns the cursor offset measured in UTF-8 bytes.
    #[must_use]
    pub const fn cursor(&self) -> Utf8ByteOffset {
        self.cursor
    }

    /// Returns the half-open selection measured in UTF-8 bytes.
    #[must_use]
    pub const fn selection(&self) -> TextRange<Utf8ByteOffset> {
        self.selection
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_rejects_untyped_mid_codepoint_offsets() {
        let error = Composition::try_new(
            "墨",
            Utf8ByteOffset::new(1),
            TextRange::new(Utf8ByteOffset::new(0), Utf8ByteOffset::new(3)),
        )
        .unwrap_err();

        assert_eq!(
            error,
            CompositionError::NotCharBoundary {
                offset: Utf8ByteOffset::new(1)
            }
        );
    }

    #[test]
    fn composition_end_offsets_are_utf8_bytes() {
        let composition = Composition::from_preedit("Mo·墨");
        let expected = Utf8ByteOffset::new("Mo·墨".len());

        assert_eq!(composition.cursor(), expected);
        assert_eq!(composition.selection(), TextRange::new(expected, expected));
    }
}
