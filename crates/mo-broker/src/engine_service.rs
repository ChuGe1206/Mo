use std::fmt;
use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use mo_domain::{EngineCommand, EngineSnapshot, SessionOptions, SessionToken};
use mo_engine::{EngineActor, EngineBackend};

#[derive(Clone, Copy)]
struct EngineBudgets {
    startup: Duration,
    request: Duration,
    shutdown: Duration,
}

impl Default for EngineBudgets {
    fn default() -> Self {
        Self {
            startup: Duration::from_secs(30),
            request: Duration::from_secs(5),
            shutdown: Duration::from_secs(5),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EngineServiceError {
    OperationFailed,
}

impl fmt::Display for EngineServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OperationFailed => formatter.write_str("engine operation failed"),
        }
    }
}

#[derive(Clone)]
pub(crate) struct EngineClient {
    sender: Sender<Envelope>,
    request_budget: Duration,
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
        let operation = match &request {
            EngineRequest::Create { .. } => crate::latency::Operation::Create,
            EngineRequest::Dispatch { .. } => crate::latency::Operation::Dispatch,
            EngineRequest::Destroy { .. } | EngineRequest::Shutdown => {
                crate::latency::Operation::Destroy
            }
        };
        if self
            .sender
            .send(Envelope {
                request,
                queued: crate::latency::Queued::new(operation),
            })
            .is_err()
        {
            crate::lifecycle::fail_stop();
        }
        // Queue time and backend execution share one budget. A late result is
        // not safe to ignore: the native command may already have committed.
        match receiver.recv_timeout(self.request_budget) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                crate::lifecycle::fail_stop()
            }
        }
    }
}

enum EngineRequest {
    Shutdown,
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

struct Envelope {
    request: EngineRequest,
    queued: crate::latency::Queued,
}

pub(crate) struct EngineService {
    client: Option<EngineClient>,
    thread: Option<JoinHandle<()>>,
    shutdown_budget: Duration,
}

impl EngineService {
    pub(crate) fn start<F, B, E>(factory: F) -> io::Result<Self>
    where
        F: FnOnce() -> Result<B, E> + Send + 'static,
        B: EngineBackend + 'static,
        E: fmt::Display,
    {
        Self::start_with_budgets(factory, EngineBudgets::default())
    }

    fn start_with_budgets<F, B, E>(factory: F, budgets: EngineBudgets) -> io::Result<Self>
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

        match startup_receiver.recv_timeout(budgets.startup) {
            Ok(Ok(())) => Ok(Self {
                client: Some(EngineClient {
                    sender,
                    request_budget: budgets.request,
                }),
                thread: Some(thread),
                shutdown_budget: budgets.shutdown,
            }),
            Ok(Err(message)) => {
                let _ = join_with_budget(thread, budgets.shutdown);
                Err(io::Error::other(message))
            }
            Err(RecvTimeoutError::Disconnected) => {
                let _ = join_with_budget(thread, budgets.shutdown);
                Err(io::Error::other("engine service exited during startup"))
            }
            Err(RecvTimeoutError::Timeout) => crate::lifecycle::fail_stop(),
        }
    }

    pub(crate) fn client(&self) -> EngineClient {
        self.client
            .as_ref()
            .expect("running engine service owns a client")
            .clone()
    }

    pub(crate) fn shutdown(mut self) -> io::Result<()> {
        if let Some(client) = self.client.take() {
            // Stop even if another idle client clone still exists. Production
            // joins connection workers first, so no new operation is admitted.
            let _ = client.sender.send(Envelope {
                request: EngineRequest::Shutdown,
                queued: crate::latency::Queued::new(crate::latency::Operation::Destroy),
            });
        }
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        join_with_budget(thread, self.shutdown_budget)
    }
}

fn join_with_budget(thread: JoinHandle<()>, budget: Duration) -> io::Result<()> {
    let deadline = Instant::now()
        .checked_add(budget)
        .unwrap_or_else(Instant::now);
    while !thread.is_finished() {
        if Instant::now() >= deadline {
            crate::lifecycle::fail_stop();
        }
        thread::sleep(Duration::from_millis(1));
    }
    thread
        .join()
        .map_err(|_| io::Error::other("engine service thread panicked"))
}

fn run_actor<B>(mut actor: EngineActor<B>, receiver: Receiver<Envelope>)
where
    B: EngineBackend,
{
    while let Ok(Envelope { request, queued }) = receiver.recv() {
        let running = queued.begin();
        match request {
            EngineRequest::Shutdown => break,
            EngineRequest::Create { options, reply } => {
                let result = actor
                    .create_session(options)
                    .map_err(|_| EngineServiceError::OperationFailed);
                running.finish();
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
                running.finish();
                let _ = reply.send(result);
            }
            EngineRequest::Destroy { token, reply } => {
                let result = actor
                    .destroy_session(token)
                    .map_err(|_| EngineServiceError::OperationFailed);
                running.finish();
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

    #[test]
    fn shutdown_does_not_wait_for_an_unused_client_clone_to_drop() {
        let service = EngineService::start(|| Ok::<_, io::Error>(FakeBackend::new())).unwrap();
        let unused_client = service.client();
        let started = Instant::now();
        service.shutdown().unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        drop(unused_client);
    }

    #[cfg(windows)]
    fn assert_fault_subprocess(phase: &str) {
        use std::process::{Command, Stdio};
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "engine_service::tests::subprocess_fault_fixture",
                "--nocapture",
            ])
            .env("MO_TEST_ENGINE_FAULT", phase)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if started.elapsed() > Duration::from_secs(3) {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("watchdog did not terminate {phase} fixture within 3 seconds");
            }
            thread::sleep(Duration::from_millis(5));
        };
        // Rust/MSVC abort uses STATUS_STACK_BUFFER_OVERRUN (fast-fail), not a
        // normal test failure (101/1). Do not accept arbitrary nonzero status.
        assert_eq!(
            status.code(),
            Some(0xc000_0409u32 as i32),
            "wrong exit status for {phase}: {status}"
        );
        use std::io::Read;
        let mut output = String::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut output)
            .unwrap();
        assert!(
            output.contains(&format!("MO_TEST_FAULT:{phase}")),
            "fixture failed before reaching its intended fault: {phase}"
        );
    }

    #[test]
    #[cfg(windows)]
    fn watchdog_fail_stops_a_stuck_startup() {
        assert_fault_subprocess("startup");
    }
    #[test]
    #[cfg(windows)]
    fn watchdog_fail_stops_a_stuck_create() {
        assert_fault_subprocess("create");
    }
    #[test]
    #[cfg(windows)]
    fn watchdog_fail_stops_a_stuck_dispatch() {
        assert_fault_subprocess("apply");
    }
    #[test]
    #[cfg(windows)]
    fn watchdog_fail_stops_a_stuck_session_destroy() {
        assert_fault_subprocess("destroy");
    }
    #[test]
    #[cfg(windows)]
    fn watchdog_fail_stops_a_stuck_finalize() {
        assert_fault_subprocess("finalize");
    }
    #[test]
    #[cfg(windows)]
    fn engine_panic_fail_stops_the_process_instead_of_serving_dead_sessions() {
        assert_fault_subprocess("panic");
    }
    #[test]
    #[cfg(windows)]
    fn connection_worker_panic_fail_stops_instead_of_hanging_scoped_joins() {
        assert_fault_subprocess("worker-panic");
    }
    #[test]
    #[cfg(windows)]
    fn pool_shutdown_watchdog_caps_total_cleanup_not_just_each_operation() {
        assert_fault_subprocess("pool-shutdown");
    }

    /// Child-only fault injection. The environment variable is read exclusively
    /// by this test, never by production Broker startup or engine code.
    #[test]
    #[cfg(windows)]
    fn subprocess_fault_fixture() {
        let Ok(phase) = std::env::var("MO_TEST_ENGINE_FAULT") else {
            return;
        };
        if phase == "pool-shutdown" {
            let stop = mo_windows_pipe::PipeCancellation::new().unwrap();
            let _guard = crate::windows_named_pipe::PoolStopWatchdog::start(
                &stop,
                Duration::from_millis(80),
            )
            .unwrap();
            println!("MO_TEST_FAULT:pool-shutdown");
            stop.cancel().unwrap();
            thread::sleep(Duration::from_secs(5));
            panic!("pool shutdown watchdog unexpectedly survived");
        }
        if phase == "worker-panic" {
            println!("MO_TEST_FAULT:worker-panic");
            crate::windows_named_pipe::supervise_pipe_worker(
                &mo_windows_pipe::PipeCancellation::new().unwrap(),
                || panic!("injected connection worker panic"),
            )
            .unwrap();
            panic!("worker panic unexpectedly survived");
        }
        struct FaultBackend {
            inner: FakeBackend,
            phase: String,
        }
        impl EngineBackend for FaultBackend {
            type Session = <FakeBackend as EngineBackend>::Session;
            type Error = <FakeBackend as EngineBackend>::Error;
            fn create_session(
                &mut self,
                options: SessionOptions,
            ) -> Result<Self::Session, Self::Error> {
                if self.phase == "create" {
                    println!("MO_TEST_FAULT:create");
                    thread::sleep(Duration::from_secs(5));
                }
                self.inner.create_session(options)
            }
            fn apply(
                &mut self,
                session: &mut Self::Session,
                command: &EngineCommand,
            ) -> Result<mo_domain::EngineOutput, Self::Error> {
                if self.phase == "apply" {
                    println!("MO_TEST_FAULT:apply");
                    thread::sleep(Duration::from_secs(5));
                }
                if self.phase == "panic" {
                    println!("MO_TEST_FAULT:panic");
                }
                assert_ne!(self.phase, "panic", "injected engine panic");
                self.inner.apply(session, command)
            }
            fn destroy_session(&mut self, session: Self::Session) -> Result<(), Self::Error> {
                if self.phase == "destroy" {
                    println!("MO_TEST_FAULT:destroy");
                    thread::sleep(Duration::from_secs(5));
                }
                self.inner.destroy_session(session)
            }
        }
        impl Drop for FaultBackend {
            fn drop(&mut self) {
                if self.phase == "finalize" {
                    println!("MO_TEST_FAULT:finalize");
                    thread::sleep(Duration::from_secs(5));
                }
            }
        }
        let budgets = EngineBudgets {
            startup: Duration::from_millis(if phase == "startup" { 80 } else { 500 }),
            request: Duration::from_millis(80),
            shutdown: Duration::from_millis(80),
        };
        let service = EngineService::start_with_budgets(
            move || {
                if phase == "startup" {
                    println!("MO_TEST_FAULT:startup");
                    thread::sleep(Duration::from_secs(5));
                }
                Ok::<_, io::Error>(FaultBackend {
                    inner: FakeBackend::new(),
                    phase,
                })
            },
            budgets,
        )
        .unwrap();
        let client = service.client();
        let token = client.create_session(SessionOptions::new()).unwrap();
        client
            .dispatch(token, EngineCommand::Key(KeyEvent::text('m')))
            .unwrap();
        client.destroy_session(token).unwrap();
        drop(client);
        service.shutdown().unwrap();
        panic!("fault fixture unexpectedly survived");
    }
}
