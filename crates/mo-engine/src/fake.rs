use std::collections::BTreeMap;
use std::fmt;

use mo_domain::{
    Candidate, Composition, EngineCommand, EngineOutput, EngineStatus, KeyModifiers, KeyState,
    SessionOptions,
};

use crate::EngineBackend;

/// Observable operation recorded by [`FakeBackend`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FakeEvent {
    /// A backend session was created.
    SessionCreated {
        /// Deterministic fake backend identifier.
        backend_session: u64,
        /// Options supplied by the actor.
        options: SessionOptions,
    },
    /// A command reached a backend session.
    CommandApplied {
        /// Deterministic fake backend identifier.
        backend_session: u64,
        /// Command in exact application order.
        command: EngineCommand,
    },
    /// A backend session was destroyed.
    SessionDestroyed {
        /// Deterministic fake backend identifier.
        backend_session: u64,
    },
}

/// Exhaustion failure from the deterministic fake backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FakeError {
    /// No unique fake backend session identifier remains.
    SessionIdExhausted,
}

impl fmt::Display for FakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionIdExhausted => formatter.write_str("fake session identifier exhausted"),
        }
    }
}

impl std::error::Error for FakeError {}

/// A deterministic, dependency-free backend for actor and frontend tests.
///
/// Printable `KeyEvent::text` presses append to the preedit. ASCII-compatible
/// Backspace, Escape, and Space key symbols remove, clear, and commit it;
/// explicit `Commit` and `Clear` commands provide the same lifecycle controls.
/// Candidate zero mirrors the preedit. The model is intentionally simple; its
/// purpose is contract testing, not language prediction.
#[derive(Debug, Default)]
pub struct FakeBackend {
    next_session: u64,
    events: Vec<FakeEvent>,
}

#[derive(Debug)]
pub struct FakeSession {
    id: u64,
    preedit: String,
    page: u32,
    schema_id: String,
    options: BTreeMap<String, bool>,
}

impl FakeBackend {
    /// Creates an empty fake backend.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next_session: 0,
            events: Vec::new(),
        }
    }

    /// Returns the complete backend operation log in application order.
    #[must_use]
    pub fn events(&self) -> &[FakeEvent] {
        &self.events
    }

    fn candidate_texts(session: &FakeSession) -> Vec<String> {
        if session.preedit.is_empty() {
            return Vec::new();
        }

        let primary = if session.page == 0 {
            session.preedit.clone()
        } else {
            format!("{}#{}", session.preedit, session.page + 1)
        };
        let alternate = primary.to_uppercase();

        if primary == alternate {
            vec![primary]
        } else {
            vec![primary, alternate]
        }
    }

    fn output(session: &FakeSession, handled: bool, commit: Option<String>) -> EngineOutput {
        let candidates = Self::candidate_texts(session)
            .into_iter()
            .enumerate()
            .map(|(index, text)| Candidate::new(text).with_label((index + 1).to_string()))
            .collect();

        EngineOutput {
            handled,
            commit,
            composition: (!session.preedit.is_empty())
                .then(|| Composition::from_preedit(session.preedit.clone())),
            candidates,
            status: EngineStatus {
                schema_id: session.schema_id.clone(),
                disabled: false,
                composing: !session.preedit.is_empty(),
                ascii_mode: session.options.get("ascii_mode").copied().unwrap_or(false),
            },
        }
    }
}

impl EngineBackend for FakeBackend {
    type Session = FakeSession;
    type Error = FakeError;

    fn create_session(&mut self, options: SessionOptions) -> Result<Self::Session, Self::Error> {
        let id = self
            .next_session
            .checked_add(1)
            .ok_or(FakeError::SessionIdExhausted)?;
        self.next_session = id;
        self.events.push(FakeEvent::SessionCreated {
            backend_session: id,
            options: options.clone(),
        });

        Ok(FakeSession {
            id,
            preedit: String::new(),
            page: 0,
            schema_id: options.schema_id.unwrap_or_else(|| "mo.fake".to_owned()),
            options: options.options,
        })
    }

    fn apply(
        &mut self,
        session: &mut Self::Session,
        command: &EngineCommand,
    ) -> Result<EngineOutput, Self::Error> {
        self.events.push(FakeEvent::CommandApplied {
            backend_session: session.id,
            command: command.clone(),
        });

        let mut handled = false;
        let mut commit = None;

        match command {
            EngineCommand::Key(event) => {
                let command_modifiers =
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER;
                if event.state == KeyState::Pressed
                    && !event.modifiers.intersects(command_modifiers)
                {
                    match event.keycode {
                        0xff08 if !session.preedit.is_empty() => {
                            session.preedit.pop();
                            handled = true;
                        }
                        0xff1b if !session.preedit.is_empty() => {
                            session.preedit.clear();
                            handled = true;
                        }
                        0x20 if !session.preedit.is_empty() => {
                            commit = Some(std::mem::take(&mut session.preedit));
                            handled = true;
                        }
                        _ => {
                            if let Some(character) = event.text {
                                session.preedit.push(character);
                                handled = true;
                            }
                        }
                    }
                    if handled {
                        session.page = 0;
                    }
                }
            }
            EngineCommand::SelectCandidate { index } => {
                if let Some(selected) = Self::candidate_texts(session).get(*index as usize) {
                    commit = Some(selected.clone());
                    session.preedit.clear();
                    session.page = 0;
                    handled = true;
                }
            }
            EngineCommand::ChangePage { backward } => {
                if !session.preedit.is_empty() {
                    session.page = if *backward {
                        session.page.saturating_sub(1)
                    } else {
                        session.page.saturating_add(1)
                    };
                    handled = true;
                }
            }
            EngineCommand::SetOption { name, value } => {
                session.options.insert(name.clone(), *value);
                handled = true;
            }
            EngineCommand::Commit => {
                if !session.preedit.is_empty() {
                    commit = Some(std::mem::take(&mut session.preedit));
                    session.page = 0;
                    handled = true;
                }
            }
            EngineCommand::Clear => {
                if !session.preedit.is_empty() {
                    session.preedit.clear();
                    session.page = 0;
                    handled = true;
                }
            }
        }

        Ok(Self::output(session, handled, commit))
    }

    fn destroy_session(&mut self, session: Self::Session) -> Result<(), Self::Error> {
        self.events.push(FakeEvent::SessionDestroyed {
            backend_session: session.id,
        });
        Ok(())
    }
}
