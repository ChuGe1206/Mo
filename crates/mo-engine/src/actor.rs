use core::fmt;

use mo_domain::{
    EngineCommand, EngineSnapshot, Generation, Revision, SessionOptions, SessionToken,
};

use crate::EngineBackend;

struct SessionSlot<S> {
    generation: Generation,
    session: Option<S>,
    retired: bool,
}

/// Failure produced while routing an operation through [`EngineActor`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineError<E> {
    /// The token refers to a slot this actor has never allocated.
    UnknownSession {
        /// Rejected token.
        token: SessionToken,
    },
    /// The slot exists but now belongs to a different generation.
    StaleSession {
        /// Rejected token.
        token: SessionToken,
        /// Current generation of the token's slot.
        current_generation: Generation,
    },
    /// The token has the current generation but its slot is not active.
    InactiveSession {
        /// Rejected token.
        token: SessionToken,
    },
    /// The actor cannot represent another session slot.
    SessionCapacityExhausted,
    /// The actor cannot assign another globally ordered revision.
    RevisionExhausted,
    /// The backend rejected an operation.
    Backend(E),
}

impl<E: fmt::Display> fmt::Display for EngineError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSession { token } => write!(formatter, "unknown engine session {token}"),
            Self::StaleSession {
                token,
                current_generation,
            } => write!(
                formatter,
                "stale engine session {token}; current generation is {}",
                current_generation.get()
            ),
            Self::InactiveSession { token } => {
                write!(formatter, "engine session {token} is not active")
            }
            Self::SessionCapacityExhausted => {
                formatter.write_str("engine session capacity exhausted")
            }
            Self::RevisionExhausted => formatter.write_str("engine revision exhausted"),
            Self::Backend(source) => write!(formatter, "engine backend failed: {source}"),
        }
    }
}

impl<E> std::error::Error for EngineError<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Backend(source) => Some(source),
            _ => None,
        }
    }
}

/// Synchronous owner of one backend and every backend session.
///
/// The exclusive `&mut self` command API is the ordering boundary: callers may
/// put the actor on a dedicated thread, but cannot concurrently enter the
/// backend through this type. Revisions are global to the actor so independently
/// arriving clients can discard late snapshots deterministically.
pub struct EngineActor<B>
where
    B: EngineBackend,
{
    backend: B,
    slots: Vec<SessionSlot<B::Session>>,
    revision: Revision,
    active_sessions: usize,
}

impl<B> EngineActor<B>
where
    B: EngineBackend,
{
    /// Creates an actor whose first successful dispatch attempt gets revision 1.
    #[must_use]
    pub fn new(backend: B) -> Self {
        Self::with_initial_revision(backend, Revision::ZERO)
    }

    /// Creates an actor starting after a persisted or negotiated revision.
    #[must_use]
    pub fn with_initial_revision(backend: B, revision: Revision) -> Self {
        Self {
            backend,
            slots: Vec::new(),
            revision,
            active_sessions: 0,
        }
    }

    /// Returns a shared backend reference for diagnostics and test inspection.
    ///
    /// Mutation is intentionally unavailable so callers cannot bypass actor
    /// ordering.
    #[must_use]
    pub const fn backend(&self) -> &B {
        &self.backend
    }

    /// Returns the last revision consumed by a backend command attempt.
    #[must_use]
    pub const fn current_revision(&self) -> Revision {
        self.revision
    }

    /// Returns the number of active logical sessions.
    #[must_use]
    pub const fn active_session_count(&self) -> usize {
        self.active_sessions
    }

    /// Creates a session, reusing a vacant slot when possible.
    pub fn create_session(
        &mut self,
        options: SessionOptions,
    ) -> Result<SessionToken, EngineError<B::Error>> {
        let reusable_index = self
            .slots
            .iter()
            .position(|slot| !slot.retired && slot.session.is_none());

        let (index, generation) = match reusable_index {
            Some(index) => (index, self.slots[index].generation),
            None => (self.slots.len(), Generation::FIRST),
        };

        let one_based_slot = u64::try_from(index)
            .ok()
            .and_then(|index| index.checked_add(1))
            .ok_or(EngineError::SessionCapacityExhausted)?;
        let token = SessionToken::from_parts(one_based_slot, generation)
            .ok_or(EngineError::SessionCapacityExhausted)?;

        let session = self
            .backend
            .create_session(options)
            .map_err(EngineError::Backend)?;

        if index == self.slots.len() {
            self.slots.push(SessionSlot {
                generation,
                session: Some(session),
                retired: false,
            });
        } else {
            self.slots[index].session = Some(session);
        }
        self.active_sessions += 1;

        Ok(token)
    }

    /// Applies one command after validating the complete session token.
    ///
    /// Invalid and stale tokens never reach the backend and do not consume a
    /// revision. Once a valid command reaches the backend its revision is
    /// consumed even if the backend returns an error; this prevents retry paths
    /// from assigning the same revision to two possibly-mutating attempts.
    pub fn dispatch(
        &mut self,
        token: SessionToken,
        command: EngineCommand,
    ) -> Result<EngineSnapshot, EngineError<B::Error>> {
        let index = self.resolve_active_index(token)?;
        let revision = self
            .revision
            .checked_next()
            .ok_or(EngineError::RevisionExhausted)?;
        self.revision = revision;

        let session = self.slots[index]
            .session
            .as_mut()
            .expect("validated active session must be present");
        let output = self
            .backend
            .apply(session, &command)
            .map_err(EngineError::Backend)?;

        Ok(EngineSnapshot::from_output(token, revision, output))
    }

    /// Retires a token before asking the backend to destroy its private state.
    ///
    /// Even when backend destruction fails, delayed commands cannot re-enter the
    /// removed session. A slot at generation `u64::MAX` is permanently retired.
    pub fn destroy_session(&mut self, token: SessionToken) -> Result<(), EngineError<B::Error>> {
        let index = self.resolve_active_index(token)?;
        let slot = &mut self.slots[index];
        let session = slot
            .session
            .take()
            .expect("validated active session must be present");

        match slot.generation.checked_next() {
            Some(next) => slot.generation = next,
            None => slot.retired = true,
        }
        self.active_sessions -= 1;

        self.backend
            .destroy_session(session)
            .map_err(EngineError::Backend)
    }

    fn resolve_active_index(&self, token: SessionToken) -> Result<usize, EngineError<B::Error>> {
        let zero_based = token.slot() - 1;
        let index = usize::try_from(zero_based)
            .ok()
            .filter(|index| *index < self.slots.len())
            .ok_or(EngineError::UnknownSession { token })?;
        let slot = &self.slots[index];

        if token.generation() != slot.generation {
            return Err(EngineError::StaleSession {
                token,
                current_generation: slot.generation,
            });
        }
        if slot.session.is_none() {
            return Err(EngineError::InactiveSession { token });
        }

        Ok(index)
    }
}
