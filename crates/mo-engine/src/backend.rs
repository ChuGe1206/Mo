use mo_domain::{EngineCommand, EngineOutput, SessionOptions};

/// Backend operations serialized by [`crate::EngineActor`].
///
/// Backend implementations do not create public session tokens or revisions;
/// those are actor-owned metadata. A future librime backend can therefore keep
/// its raw session handle entirely inside `Self::Session`.
pub trait EngineBackend {
    /// Backend-private state for one active session.
    type Session;

    /// Backend-specific failure.
    type Error;

    /// Creates backend-private state for a new logical session.
    fn create_session(&mut self, options: SessionOptions) -> Result<Self::Session, Self::Error>;

    /// Applies exactly one command to one backend session.
    ///
    /// The returned value must be fully owned. In particular, an FFI backend
    /// must copy data out of foreign buffers before returning.
    fn apply(
        &mut self,
        session: &mut Self::Session,
        command: &EngineCommand,
    ) -> Result<EngineOutput, Self::Error>;

    /// Destroys backend-private state after its public token has been retired.
    fn destroy_session(&mut self, _session: Self::Session) -> Result<(), Self::Error> {
        Ok(())
    }
}
