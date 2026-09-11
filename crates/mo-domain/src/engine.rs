use std::collections::BTreeMap;
use std::fmt;

use crate::{Composition, KeyEvent};

/// A monotonically increasing generation used to prevent session-token ABA.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Generation(u64);

impl Generation {
    /// The first valid generation.
    pub const FIRST: Self = Self(1);

    /// Constructs a non-zero generation.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    /// Returns the integer representation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns the next generation, or `None` at exhaustion.
    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// A global Engine Actor revision assigned to an accepted command attempt.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Revision(u64);

impl Revision {
    /// Revision before the first command is dispatched.
    pub const ZERO: Self = Self(0);

    /// Constructs a revision.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the integer representation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns the next revision, or `None` at exhaustion.
    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Opaque logical identity of an Engine Actor session.
///
/// Reusing a vacant slot always changes its generation, so delayed IPC from a
/// former owner cannot affect the new session.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionToken {
    slot: u64,
    generation: Generation,
}

impl SessionToken {
    /// Constructs a token from validated wire values.
    #[must_use]
    pub const fn from_parts(slot: u64, generation: Generation) -> Option<Self> {
        if slot == 0 {
            None
        } else {
            Some(Self { slot, generation })
        }
    }

    /// Returns the one-based session slot.
    #[must_use]
    pub const fn slot(self) -> u64 {
        self.slot
    }

    /// Returns the slot generation.
    #[must_use]
    pub const fn generation(self) -> Generation {
        self.generation
    }
}

impl fmt::Display for SessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.slot, self.generation.get())
    }
}

/// Optional values used when creating an engine session.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionOptions {
    /// Requested schema identifier. A backend may choose its configured default
    /// when this is `None`.
    pub schema_id: Option<String>,
    /// Initial named boolean options in deterministic key order.
    pub options: BTreeMap<String, bool>,
}

impl SessionOptions {
    /// Creates empty session options.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            schema_id: None,
            options: BTreeMap::new(),
        }
    }

    /// Sets the requested schema identifier.
    #[must_use]
    pub fn with_schema(mut self, schema_id: impl Into<String>) -> Self {
        self.schema_id = Some(schema_id.into());
        self
    }

    /// Sets an initial named boolean option.
    #[must_use]
    pub fn with_option(mut self, name: impl Into<String>, value: bool) -> Self {
        self.options.insert(name.into(), value);
        self
    }
}

/// Commands accepted by the platform-neutral engine contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineCommand {
    /// Dispatches a normalized keyboard event.
    Key(KeyEvent),
    /// Selects a candidate by zero-based index on the current page.
    SelectCandidate { index: u32 },
    /// Moves to the previous or next candidate page.
    ChangePage { backward: bool },
    /// Changes a named boolean runtime option.
    SetOption { name: String, value: bool },
    /// Commits the active composition.
    Commit,
    /// Clears the active composition without committing it.
    Clear,
}

/// A candidate displayed for the active composition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    /// Candidate text that will be committed.
    pub text: String,
    /// Optional annotation displayed next to the candidate.
    pub comment: Option<String>,
    /// Optional frontend label, such as a selection key.
    pub label: Option<String>,
}

impl Candidate {
    /// Constructs an unannotated candidate.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            comment: None,
            label: None,
        }
    }

    /// Attaches a display-only annotation.
    #[must_use]
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Attaches a frontend selection label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Session status required by platform frontends.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EngineStatus {
    /// Active schema identifier.
    pub schema_id: String,
    /// Whether the backend is disabled.
    pub disabled: bool,
    /// Whether a non-empty composition is active.
    pub composing: bool,
    /// Whether direct ASCII mode is active.
    pub ascii_mode: bool,
}

/// Owned backend output before the Engine Actor attaches ordering metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EngineOutput {
    /// Whether the command was consumed by the engine.
    pub handled: bool,
    /// Text to commit exactly once for this command, if any.
    pub commit: Option<String>,
    /// Active composition after the command, if any.
    pub composition: Option<Composition>,
    /// Candidate page after the command.
    pub candidates: Vec<Candidate>,
    /// Session status after the command.
    pub status: EngineStatus,
}

/// Immutable, fully owned result of one ordered engine command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineSnapshot {
    /// Session that produced this result.
    pub session: SessionToken,
    /// Global actor revision of this command attempt.
    pub revision: Revision,
    /// Whether the command was consumed by the engine.
    pub handled: bool,
    /// Text to commit exactly once for this command, if any.
    pub commit: Option<String>,
    /// Active composition after the command, if any.
    pub composition: Option<Composition>,
    /// Candidate page after the command.
    pub candidates: Vec<Candidate>,
    /// Session status after the command.
    pub status: EngineStatus,
}

impl EngineSnapshot {
    /// Attaches actor-owned identity and ordering metadata to backend output.
    #[must_use]
    pub fn from_output(session: SessionToken, revision: Revision, output: EngineOutput) -> Self {
        Self {
            session,
            revision,
            handled: output.handled,
            commit: output.commit,
            composition: output.composition,
            candidates: output.candidates,
            status: output.status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_reject_invalid_zero_values_and_advance_checked() {
        assert!(Generation::new(0).is_none());
        assert!(SessionToken::from_parts(0, Generation::FIRST).is_none());

        let token = SessionToken::from_parts(7, Generation::FIRST).unwrap();
        assert_eq!(token.to_string(), "7:1");
        assert_eq!(token.generation().checked_next().unwrap().get(), 2);
        assert_eq!(Revision::ZERO.checked_next().unwrap(), Revision::new(1));
    }

    #[test]
    fn session_options_are_deterministic_and_last_write_wins() {
        let options = SessionOptions::new()
            .with_option("z", false)
            .with_option("a", true)
            .with_option("z", true);

        assert_eq!(
            options
                .options
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["a", "z"]
        );
        assert!(options.options["z"]);
    }
}
