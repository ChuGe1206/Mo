use std::fmt;
use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use mo_domain::{EngineCommand, EngineSnapshot, SessionOptions, SessionToken};
use mo_engine::{EngineActor, EngineBackend};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EngineServiceError {
    Unavailable,
    OperationFailed,
}

impl fmt::Display for EngineServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("engine service is unavailable"),
            Self::OperationFailed => formatter.write_str("engine operation failed"),
        }
    }
}

#[derive(Clone)]
pub(crate) struct EngineClient {
    sender: Sender<EngineRequest>,
}

impl EngineClient {
    pub(crate) fn create_session(
        &self,
        options: SessionOptions,
    ) -> Result<SessionToken, EngineServiceError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.request(EngineRequest::Create { options, reply }, receiver)
    }

    pub(crate) fn dispatch(
        &self,
        token: SessionToken,
        command: EngineCommand,
    ) -> Result<EngineSnapshot, EngineServiceError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.request(
            EngineRequest::Dispatch {
                token,
                command,
                reply,
            },
            receiver,
        )
    }

    pub(crate) fn destroy_session(&self, token: SessionToken) -> Result<(), EngineServiceError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.request(EngineRequest::Destroy { token, reply }, receiver)
    }

    fn request<T>(
        &self,
        request: EngineRequest,
        receiver: Receiver<Result<T, EngineServiceError>>,
    ) -> Result<T, EngineServiceError> {
        self.sender
            .send(request)
            .map_err(|_| EngineServiceError::Unavailable)?;
        receiver
            .recv()
            .map_err(|_| EngineServiceError::Unavailable)?
    }
}

enum EngineRequest {
    Create {
        options: SessionOptions,
        reply: mpsc::SyncSender<Result<SessionToken, EngineServiceError>>,
    },
    Dispatch {
        token: SessionToken,
        command: EngineCommand,
        reply: mpsc::SyncSender<Result<EngineSnapshot, EngineServiceError>>,
    },
    Destroy {
        token: SessionToken,
        reply: mpsc::SyncSender<Result<(), EngineServiceError>>,
    },
}

pub(crate) struct EngineService {
    client: Option<EngineClient>,
    thread: Option<JoinHandle<()>>,
}

impl EngineService {
    pub(crate) fn start<F, B, E>(factory: F) -> io::Result<Self>
    where
        F: FnOnce() -> Result<B, E> + Send + 'static,
        B: EngineBackend + 'static,
        E: fmt::Display,
    {
        let (sender, receiver) = mpsc::channel();
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("mo-engine".to_owned())
            .spawn(move || match factory() {
                Ok(backend) => {
                    let _ = startup_sender.send(Ok(()));
                    run_actor(EngineActor::new(backend), receiver);
                }
                Err(error) => {
                    let _ = startup_sender.send(Err(error.to_string()));
                }
            })?;

        match startup_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                client: Some(EngineClient { sender }),
                thread: Some(thread),
            }),
            Ok(Err(message)) => {
                let _ = thread.join();
                Err(io::Error::other(message))
            }
            Err(_) => {
                let _ = thread.join();
                Err(io::Error::other("engine service exited during startup"))
            }
        }
    }

    pub(crate) fn client(&self) -> EngineClient {
        self.client
            .as_ref()
            .expect("running engine service owns a client")
            .clone()
    }

    pub(crate) fn shutdown(mut self) -> io::Result<()> {
        self.client.take();
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        thread
            .join()
            .map_err(|_| io::Error::other("engine service thread panicked"))
    }
}

fn run_actor<B>(mut actor: EngineActor<B>, receiver: Receiver<EngineRequest>)
where
    B: EngineBackend,
{
    while let Ok(request) = receiver.recv() {
        match request {
            EngineRequest::Create { options, reply } => {
                let result = actor
                    .create_session(options)
                    .map_err(|_| EngineServiceError::OperationFailed);
                let _ = reply.send(result);
            }
            EngineRequest::Dispatch {
                token,
                command,
                reply,
            } => {
                let result = actor
                    .dispatch(token, command)
                    .map_err(|_| EngineServiceError::OperationFailed);
                let _ = reply.send(result);
            }
            EngineRequest::Destroy { token, reply } => {
                let result = actor
                    .destroy_session(token)
                    .map_err(|_| EngineServiceError::OperationFailed);
                let _ = reply.send(result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use mo_domain::KeyEvent;
    use mo_engine::FakeBackend;

    use super::*;

    #[test]
    fn service_serializes_sessions_on_its_own_thread() {
        let service = EngineService::start(|| Ok::<_, io::Error>(FakeBackend::new())).unwrap();
        let first = service.client();
        let second = service.client();
        let first_token = first.create_session(SessionOptions::new()).unwrap();
        let second_token = second.create_session(SessionOptions::new()).unwrap();

        let first_snapshot = first
            .dispatch(first_token, EngineCommand::Key(KeyEvent::text('a')))
            .unwrap();
        let second_snapshot = second
            .dispatch(second_token, EngineCommand::Key(KeyEvent::text('b')))
            .unwrap();
        assert!(second_snapshot.revision > first_snapshot.revision);

        first.destroy_session(first_token).unwrap();
        second.destroy_session(second_token).unwrap();
        drop(first);
        drop(second);
        service.shutdown().unwrap();
    }

    #[test]
    fn startup_failure_is_returned_to_the_caller() {
        let result = EngineService::start(|| Err::<FakeBackend, _>("backend unavailable"));
        match result {
            Err(error) => assert_eq!(error.to_string(), "backend unavailable"),
            Ok(_) => panic!("failing backend factory unexpectedly started"),
        }
    }
}
