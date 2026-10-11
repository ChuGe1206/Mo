use mo_domain::{EngineCommand, EngineOutput, KeyEvent, Revision, SessionOptions, SessionToken};
use mo_engine::{EngineActor, EngineBackend, EngineError, FakeBackend, FakeEvent};

fn type_character(character: char) -> EngineCommand {
    EngineCommand::Key(KeyEvent::text(character))
}

#[test]
fn commands_receive_one_strict_global_order_across_sessions() {
    let mut actor = EngineActor::new(FakeBackend::new());
    let first = actor
        .create_session(SessionOptions::new().with_schema("first"))
        .unwrap();
    let second = actor
        .create_session(SessionOptions::new().with_schema("second"))
        .unwrap();

    let first_key = actor.dispatch(first, type_character('m')).unwrap();
    let second_key = actor.dispatch(second, type_character('o')).unwrap();
    let first_commit = actor.dispatch(first, EngineCommand::Commit).unwrap();

    assert_eq!(first_key.revision, Revision::new(1));
    assert_eq!(second_key.revision, Revision::new(2));
    assert_eq!(first_commit.revision, Revision::new(3));
    assert_eq!(first_key.session, first);
    assert_eq!(second_key.session, second);
    assert_eq!(
        first_key.composition.as_ref().unwrap().preedit(),
        "m",
        "sessions must not share composition state"
    );
    assert_eq!(second_key.composition.as_ref().unwrap().preedit(), "o");
    assert_eq!(first_commit.commit.as_deref(), Some("m"));
    assert!(first_commit.composition.is_none());

    let applied_sessions = actor
        .backend()
        .events()
        .iter()
        .filter_map(|event| match event {
            FakeEvent::CommandApplied {
                backend_session, ..
            } => Some(*backend_session),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(applied_sessions, vec![1, 2, 1]);
}

#[test]
fn retired_generation_is_rejected_before_backend_and_slot_reuse() {
    let mut actor = EngineActor::new(FakeBackend::new());
    let old = actor.create_session(SessionOptions::new()).unwrap();
    actor.dispatch(old, type_character('a')).unwrap();
    actor.destroy_session(old).unwrap();

    let fresh = actor.create_session(SessionOptions::new()).unwrap();
    assert_eq!(fresh.slot(), old.slot(), "vacant slots should be reused");
    assert_eq!(fresh.generation().get(), old.generation().get() + 1);

    let event_count = actor.backend().events().len();
    let error = actor.dispatch(old, type_character('x')).unwrap_err();
    assert_eq!(
        error,
        EngineError::StaleSession {
            token: old,
            current_generation: fresh.generation(),
        }
    );
    assert_eq!(actor.backend().events().len(), event_count);
    assert_eq!(actor.current_revision(), Revision::new(1));

    let snapshot = actor.dispatch(fresh, type_character('b')).unwrap();
    assert_eq!(snapshot.revision, Revision::new(2));
    assert_eq!(snapshot.composition.unwrap().preedit(), "b");
}

#[test]
fn unknown_and_inactive_tokens_do_not_consume_revisions() {
    let mut actor = EngineActor::new(FakeBackend::new());
    let active = actor.create_session(SessionOptions::new()).unwrap();

    let unknown = SessionToken::from_parts(99, active.generation()).unwrap();
    assert!(matches!(
        actor.dispatch(unknown, EngineCommand::Clear),
        Err(EngineError::UnknownSession { token }) if token == unknown
    ));

    actor.destroy_session(active).unwrap();
    let current_generation = active.generation().checked_next().unwrap();
    let inactive = SessionToken::from_parts(active.slot(), current_generation).unwrap();
    assert!(matches!(
        actor.dispatch(inactive, EngineCommand::Clear),
        Err(EngineError::InactiveSession { token }) if token == inactive
    ));
    assert_eq!(actor.current_revision(), Revision::ZERO);
}

#[derive(Default)]
struct FailOnceBackend {
    failed: bool,
}

impl EngineBackend for FailOnceBackend {
    type Session = ();
    type Error = &'static str;

    fn create_session(&mut self, _options: SessionOptions) -> Result<Self::Session, Self::Error> {
        Ok(())
    }

    fn apply(
        &mut self,
        _session: &mut Self::Session,
        _command: &EngineCommand,
    ) -> Result<EngineOutput, Self::Error> {
        if self.failed {
            Ok(EngineOutput::default())
        } else {
            self.failed = true;
            Err("injected failure")
        }
    }
}

#[test]
fn backend_failure_consumes_its_attempt_revision() {
    let mut actor = EngineActor::new(FailOnceBackend::default());
    let session = actor.create_session(SessionOptions::new()).unwrap();

    assert_eq!(
        actor.dispatch(session, EngineCommand::Clear),
        Err(EngineError::Backend("injected failure"))
    );
    assert_eq!(actor.current_revision(), Revision::new(1));

    let next = actor.dispatch(session, EngineCommand::Clear).unwrap();
    assert_eq!(next.revision, Revision::new(2));
}

#[test]
fn catalog_insertion_clears_preedit_and_commits_exactly_once() {
    let mut actor = EngineActor::new(FakeBackend::new());
    let token = actor.create_session(SessionOptions::new()).unwrap();
    for (index, symbol) in mo_domain::SYMBOLS_V1.iter().enumerate() {
        actor.dispatch(token, type_character('a')).unwrap();
        let snapshot = actor
            .dispatch(
                token,
                EngineCommand::InsertSymbol {
                    index: index as u32,
                },
            )
            .unwrap();
        assert!(snapshot.handled);
        assert_eq!(snapshot.commit.as_deref(), Some(symbol.text));
        assert!(
            snapshot.composition.is_none()
                && snapshot.candidates.is_empty()
                && snapshot.page.is_none()
        );
        assert!(!snapshot.status.composing);
        assert!(matches!(
            actor.backend().events().last(),
            Some(FakeEvent::CommandApplied {
                command: EngineCommand::Clear,
                ..
            })
        ));
        let next = actor.dispatch(token, type_character('b')).unwrap();
        assert!(next.commit.is_none());
        assert_eq!(next.composition.unwrap().preedit(), "b");
        actor.dispatch(token, EngineCommand::Clear).unwrap();
    }
    let revision = actor.current_revision();
    let count = actor.backend().events().len();
    assert_eq!(
        actor.dispatch(token, EngineCommand::InsertSymbol { index: u32::MAX }),
        Err(EngineError::InvalidSymbol)
    );
    assert_eq!(actor.current_revision(), revision);
    assert_eq!(actor.backend().events().len(), count);
}

struct RetainingClearBackend;
impl EngineBackend for RetainingClearBackend {
    type Session = ();
    type Error = &'static str;
    fn create_session(&mut self, _: SessionOptions) -> Result<(), Self::Error> {
        Ok(())
    }
    fn apply(&mut self, _: &mut (), _: &EngineCommand) -> Result<EngineOutput, Self::Error> {
        Ok(EngineOutput {
            composition: Some(mo_domain::Composition::from_preedit("a")),
            ..EngineOutput::default()
        })
    }
}
#[test]
fn symbol_never_commits_when_backend_retains_unconfirmed_input() {
    let mut actor = EngineActor::new(RetainingClearBackend);
    let token = actor.create_session(SessionOptions::new()).unwrap();
    assert_eq!(
        actor.dispatch(token, EngineCommand::InsertSymbol { index: 0 }),
        Err(EngineError::SymbolClearIncomplete)
    );
    assert_eq!(actor.current_revision(), Revision::new(1));
}
