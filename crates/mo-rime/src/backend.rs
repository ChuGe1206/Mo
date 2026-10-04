use core::fmt;
use std::ffi::c_int;

use mo_domain::{
    Candidate, Composition, CompositionError, EngineCommand, EngineOutput, EngineStatus,
    KeyModifiers, KeyState, SessionOptions, TextRange, Utf8ByteOffset,
};
use mo_engine::EngineBackend;

use crate::{ContextSnapshot, Engine, Error, StatusSnapshot};

const RIME_SHIFT_MASK: u32 = 1 << 0;
const RIME_LOCK_MASK: u32 = 1 << 1;
const RIME_CONTROL_MASK: u32 = 1 << 2;
const RIME_ALT_MASK: u32 = 1 << 3;
const RIME_SUPER_MASK: u32 = 1 << 26;
const RIME_RELEASE_MASK: u32 = 1 << 30;

/// Adapter that makes the safe librime owner usable by `EngineActor`.
pub struct RimeBackend {
    engine: Engine,
    resource_anchor: Option<mo_rime_sys::RimeSessionId>,
}

impl RimeBackend {
    /// Wraps an initialized, thread-affine librime engine.
    #[must_use]
    pub const fn new(engine: Engine) -> Self {
        Self {
            engine,
            resource_anchor: None,
        }
    }

    /// Holds one private, input-free session until backend teardown.
    ///
    /// The pinned engine shares dictionary/OpenCC owners weakly across sessions.
    /// Keeping an owner avoids unloading/reloading them when all frontend sessions
    /// disappear. This session is never dispatched, committed, cleared, exposed,
    /// or recycled as a frontend session. It does not warm the first translation.
    /// The anchor remains on `rime_ice`; selected double-pinyin sessions use
    /// the same pinned rime-ice dictionary pack and their own schema state.
    pub fn with_resource_anchor(engine: Engine) -> Result<Self, Error> {
        let id = engine.create_session_id()?;
        Ok(Self {
            engine,
            resource_anchor: Some(id),
        })
    }

    /// Prepares the private anchor's OpenCC owners and requires a healthy main user dictionary.
    ///
    /// Requires Mo's versioned native extension. Absence or any preparation
    /// failure is fatal to startup; no lazy fallback, frontend session or wire
    /// token is created. The normal engine-startup watchdog covers this work.
    pub fn with_prepared_resources(engine: Engine) -> Result<Self, Error> {
        if engine.prepare_resources.is_none() {
            return Err(Error::MissingFunction("mo_rime_prepare_resources_v3"));
        }
        let backend = Self::with_resource_anchor(engine)?;
        backend.engine.prepare_resources_id(
            backend
                .resource_anchor
                .expect("resource anchor was created"),
        )?;
        Ok(backend)
    }
}

impl Drop for RimeBackend {
    fn drop(&mut self) {
        if let Some(id) = self.resource_anchor.take() {
            // No fallible work/logging in Drop. Engine's subsequent cleanup-all
            // covers an unsuccessful explicit destroy before native finalize.
            let _ = self.engine.destroy_session_id(id);
        }
    }
}

/// Backend-private librime session identity.
///
/// The raw value is never exposed or constructible outside this crate. It is
/// valid only while its owning [`RimeBackend`] remains alive.
pub struct RimeBackendSession {
    id: mo_rime_sys::RimeSessionId,
}

/// Failure while adapting a domain command or owned librime snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RimeBackendError {
    /// The safe librime boundary rejected a native operation.
    Native(Error),
    /// Only the pinned rime-ice schemas and supported per-session options are allowed.
    UnsupportedSessionOptions,
    /// The current minimal librime API prefix cannot represent this command.
    UnsupportedCommand(&'static str),
    /// A logical key symbol cannot be represented by librime's C `int` ABI.
    KeyCodeOutOfRange(u32),
    /// A newer frontend supplied modifier bits this adapter cannot preserve.
    UnsupportedModifiers(u32),
    /// librime returned a negative or otherwise invalid byte offset.
    InvalidOffset {
        /// Native field name.
        field: &'static str,
        /// Native signed value.
        value: c_int,
    },
    /// librime's reported byte length differs from the owned UTF-8 preedit.
    InvalidCompositionLength {
        /// Native byte length.
        reported: c_int,
        /// Owned UTF-8 byte length after copying.
        actual: usize,
    },
    /// librime returned cursor or selection offsets that violate the domain contract.
    InvalidComposition(CompositionError),
}

impl fmt::Display for RimeBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native(error) => write!(formatter, "librime operation failed: {error}"),
            Self::UnsupportedSessionOptions => formatter.write_str(
                "session requests a schema or option outside Mo's pinned rime-ice allowlist",
            ),
            Self::UnsupportedCommand(command) => {
                write!(
                    formatter,
                    "librime command `{command}` is not enabled in Phase 0"
                )
            }
            Self::KeyCodeOutOfRange(keycode) => {
                write!(
                    formatter,
                    "logical key symbol {keycode:#x} exceeds librime's C int"
                )
            }
            Self::UnsupportedModifiers(bits) => {
                write!(formatter, "unsupported Mo modifier bits {bits:#x}")
            }
            Self::InvalidOffset { field, value } => {
                write!(
                    formatter,
                    "librime returned invalid `{field}` offset {value}"
                )
            }
            Self::InvalidCompositionLength { reported, actual } => write!(
                formatter,
                "librime reported composition length {reported}, owned UTF-8 length is {actual}"
            ),
            Self::InvalidComposition(error) => {
                write!(formatter, "invalid librime preedit: {error}")
            }
        }
    }
}

impl std::error::Error for RimeBackendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Native(error) => Some(error),
            Self::InvalidComposition(error) => Some(error),
            _ => None,
        }
    }
}

impl From<Error> for RimeBackendError {
    fn from(value: Error) -> Self {
        Self::Native(value)
    }
}

impl From<CompositionError> for RimeBackendError {
    fn from(value: CompositionError) -> Self {
        Self::InvalidComposition(value)
    }
}

impl EngineBackend for RimeBackend {
    type Session = RimeBackendSession;
    type Error = RimeBackendError;

    fn create_session(&mut self, options: SessionOptions) -> Result<Self::Session, Self::Error> {
        if options.schema_id.as_deref().is_some_and(|schema| {
            !matches!(
                schema,
                "rime_ice"
                    | "double_pinyin"
                    | "double_pinyin_flypy"
                    | "double_pinyin_mspy"
                    | "double_pinyin_sogou"
            )
        }) || options.options.keys().any(|name| {
            name != "traditionalization" && name != "emoji" && name != "mo_disable_learning"
        }) {
            return Err(RimeBackendError::UnsupportedSessionOptions);
        }
        let id = self.engine.create_session_id()?;
        let configured = (|| -> Result<(), Error> {
            if let Some(schema) = options.schema_id.as_deref() {
                self.engine.select_schema_id(id, schema)?;
            }
            for (name, value) in &options.options {
                self.engine.set_option_id(id, name, *value)?;
            }
            Ok(())
        })();
        if let Err(error) = configured {
            let _ = self.engine.destroy_session_id(id);
            return Err(error.into());
        }
        Ok(RimeBackendSession { id })
    }

    fn apply(
        &mut self,
        session: &mut Self::Session,
        command: &EngineCommand,
    ) -> Result<EngineOutput, Self::Error> {
        let handled = match command {
            EngineCommand::Key(event) => {
                let keycode = rime_keycode(*event)?;
                self.engine
                    .process_key_id(session.id, keycode, rime_modifier_mask(*event)?)
            }
            EngineCommand::Commit => self.engine.commit_composition_id(session.id),
            EngineCommand::Clear => {
                let was_composing = self
                    .engine
                    .status_id(session.id)?
                    .is_some_and(|status| status.is_composing);
                self.engine.clear_composition_id(session.id);
                was_composing
            }
            EngineCommand::SelectCandidate { index } => self
                .engine
                .select_candidate_id(session.id, *index as usize)?,
            EngineCommand::ChangePage { backward } => {
                self.engine.change_page_id(session.id, *backward)?
            }
            EngineCommand::SetOption { .. } => {
                return Err(RimeBackendError::UnsupportedCommand("set_option"));
            }
        };

        let commit = self
            .engine
            .take_commit_id(session.id)?
            .map(|value| value.text);
        let context = self.engine.context_id(session.id)?;
        let status = self
            .engine
            .status_id(session.id)?
            .unwrap_or(StatusSnapshot {
                schema_id: None,
                schema_name: None,
                is_disabled: false,
                is_composing: false,
                is_ascii_mode: false,
                is_full_shape: false,
                is_simplified: false,
                is_traditional: false,
                is_ascii_punct: false,
            });

        Ok(EngineOutput {
            handled,
            commit,
            composition: context
                .as_ref()
                .map(context_composition)
                .transpose()?
                .flatten(),
            candidates: context.map_or_else(Vec::new, context_candidates),
            status: EngineStatus {
                schema_id: status.schema_id.unwrap_or_default(),
                disabled: status.is_disabled,
                composing: status.is_composing,
                ascii_mode: status.is_ascii_mode,
            },
        })
    }

    fn destroy_session(&mut self, session: Self::Session) -> Result<(), Self::Error> {
        self.engine.destroy_session_id(session.id)?;
        Ok(())
    }
}

fn rime_keycode(event: mo_domain::KeyEvent) -> Result<c_int, RimeBackendError> {
    let keycode = event.text.map_or(event.keycode, |character| {
        let scalar = character as u32;
        if scalar <= 0xff {
            scalar
        } else {
            0x0100_0000 | scalar
        }
    });
    c_int::try_from(keycode).map_err(|_| RimeBackendError::KeyCodeOutOfRange(keycode))
}

fn rime_modifier_mask(event: mo_domain::KeyEvent) -> Result<c_int, RimeBackendError> {
    const KNOWN_MO_MODIFIERS: u32 = (1 << 5) - 1;
    let unknown = event.modifiers.bits() & !KNOWN_MO_MODIFIERS;
    if unknown != 0 {
        return Err(RimeBackendError::UnsupportedModifiers(unknown));
    }
    let mut mask = 0_u32;
    if event.modifiers.contains(KeyModifiers::SHIFT) {
        mask |= RIME_SHIFT_MASK;
    }
    if event.modifiers.contains(KeyModifiers::CAPS_LOCK) {
        mask |= RIME_LOCK_MASK;
    }
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        mask |= RIME_CONTROL_MASK;
    }
    if event.modifiers.contains(KeyModifiers::ALT) {
        mask |= RIME_ALT_MASK;
    }
    if event.modifiers.contains(KeyModifiers::SUPER) {
        mask |= RIME_SUPER_MASK;
    }
    if event.state == KeyState::Released {
        mask |= RIME_RELEASE_MASK;
    }
    Ok(c_int::try_from(mask).expect("known librime modifier bits fit C int"))
}

fn context_composition(context: &ContextSnapshot) -> Result<Option<Composition>, RimeBackendError> {
    let native = &context.composition;
    if native.preedit.is_empty() {
        return Ok(None);
    }
    if usize::try_from(native.byte_length).ok() != Some(native.preedit.len()) {
        return Err(RimeBackendError::InvalidCompositionLength {
            reported: native.byte_length,
            actual: native.preedit.len(),
        });
    }
    let cursor = checked_offset("composition.cursor_byte_pos", native.cursor_byte_pos)?;
    let selection_start = checked_offset(
        "composition.selection_start_byte",
        native.selection_start_byte,
    )?;
    let selection_end =
        checked_offset("composition.selection_end_byte", native.selection_end_byte)?;
    Ok(Some(Composition::try_new(
        native.preedit.clone(),
        cursor,
        TextRange::new(selection_start, selection_end),
    )?))
}

fn checked_offset(field: &'static str, value: c_int) -> Result<Utf8ByteOffset, RimeBackendError> {
    usize::try_from(value)
        .map(Utf8ByteOffset::new)
        .map_err(|_| RimeBackendError::InvalidOffset { field, value })
}

fn context_candidates(context: ContextSnapshot) -> Vec<Candidate> {
    let fallback_labels = context
        .menu
        .select_keys
        .as_deref()
        .map(|keys| keys.chars().map(|key| key.to_string()).collect::<Vec<_>>())
        .unwrap_or_default();
    context
        .menu
        .candidates
        .into_iter()
        .enumerate()
        .map(|(index, native)| {
            let mut candidate = Candidate::new(native.text);
            if let Some(comment) = native.comment {
                candidate = candidate.with_comment(comment);
            }
            if let Some(label) = context
                .select_labels
                .get(index)
                .or_else(|| fallback_labels.get(index))
            {
                candidate = candidate.with_label(label.clone());
            }
            candidate
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mo_domain::KeyEvent;

    #[test]
    fn modifiers_match_librime_key_table_masks() {
        let event = KeyEvent::released(0xffe1).with_modifiers(
            KeyModifiers::SHIFT
                | KeyModifiers::CAPS_LOCK
                | KeyModifiers::CONTROL
                | KeyModifiers::ALT
                | KeyModifiers::SUPER,
        );
        assert_eq!(
            rime_modifier_mask(event).unwrap() as u32,
            RIME_SHIFT_MASK
                | RIME_LOCK_MASK
                | RIME_CONTROL_MASK
                | RIME_ALT_MASK
                | RIME_SUPER_MASK
                | RIME_RELEASE_MASK
        );
    }

    #[test]
    fn unicode_text_uses_x11_unicode_keysym_encoding() {
        assert_eq!(
            rime_keycode(KeyEvent::text('墨')).unwrap() as u32,
            0x0100_0000 | '墨' as u32
        );
        assert_eq!(
            rime_keycode(KeyEvent::text('é')).unwrap(),
            c_int::from(b'\xe9')
        );
    }

    #[test]
    fn unknown_mo_modifier_bits_are_not_silently_dropped() {
        let event = KeyEvent::pressed(0x61).with_modifiers(KeyModifiers::from_bits(1 << 31));
        assert_eq!(
            rime_modifier_mask(event),
            Err(RimeBackendError::UnsupportedModifiers(1 << 31))
        );
    }
}
