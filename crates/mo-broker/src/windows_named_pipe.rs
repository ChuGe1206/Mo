//! Authenticated Windows named-pipe adapter for the broker state machine.
//!
//! Every connection is validated before dispatching even its `Hello` frame.
//! Concurrent and successive connections keep independent protocol/session-token state while
//! one process-wide engine service preserves backend state and ordering.

use std::fmt;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, mpsc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mo_engine::{EngineBackend, FakeBackend};
use mo_ipc::Frame;
use mo_windows_pipe::{
    AuthenticatedPipe, MAX_PIPE_SLOTS, PipeAddress, PipeCancellation, PipeListener, PipePool,
};

use crate::BrokerConnection;
use crate::engine_service::{EngineClient, EngineService};

pub const DEFAULT_ENDPOINT: &str = "Broker.v1";
pub const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(2);
pub const FRAME_ASSEMBLY_TIMEOUT: Duration = Duration::from_secs(2);
pub const RESPONSE_WRITE_TIMEOUT: Duration = Duration::from_secs(2);
pub const POOL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);

static GENERATION_COUNTER: AtomicU64 = AtomicU64::new(1);
static GENERATION_SEED: OnceLock<u64> = OnceLock::new();

pub fn bind_default() -> io::Result<PipeListener> {
    PipeListener::bind(PipeAddress::new(DEFAULT_ENDPOINT)?)
}

pub fn bind_default_pool() -> io::Result<PipePool> {
    PipePool::bind(PipeAddress::new(DEFAULT_ENDPOINT)?, MAX_PIPE_SLOTS)
}

/// Bounded connection workers, one per independently secured slot, sharing one
/// thread-affine engine. None runs until every worker has been created. The
/// optional per-slot limit is for deterministic tests, not installed startup.
pub fn serve_pool_with_backend_factory<F, B, E>(
    pool: PipePool,
    factory: F,
    per_slot_limit: Option<usize>,
) -> io::Result<()>
where
    F: FnOnce() -> Result<B, E> + Send + 'static,
    B: EngineBackend + 'static,
    E: fmt::Display,
{
    serve_pool_with_shutdown(pool, factory, PipeCancellation::new()?, per_slot_limit)
}

/// Coordinator-owned, process-local stop signal; never driven by an IPC peer.
pub fn serve_pool_with_shutdown<F, B, E>(
    mut pool: PipePool,
    factory: F,
    shutdown: PipeCancellation,
    per_slot_limit: Option<usize>,
) -> io::Result<()>
where
    F: FnOnce() -> Result<B, E> + Send + 'static,
    B: EngineBackend + 'static,
    E: fmt::Display,
{
    if per_slot_limit == Some(0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "per-slot limit must be non-zero",
        ));
    }
    if shutdown.is_cancelled() {
        return Ok(());
    }
    // A per-destroy limit alone permits 64 slow destroys per connection to
    // multiply shutdown latency. This guard covers ALL workers and finalize.
    let _stop_watchdog = PoolStopWatchdog::start(&shutdown, POOL_SHUTDOWN_TIMEOUT)?;
    pool.set_cancellation(&shutdown);
    let engine = EngineService::start(factory)?;
    let slot_count = pool.listeners().len();
    let outcome = thread::scope(|scope| {
        let mut gates = Vec::new();
        let mut workers: Vec<thread::ScopedJoinHandle<'_, io::Result<()>>> = Vec::new();
        for (slot, mut listener) in pool.into_listeners().into_iter().enumerate() {
            let client = engine.client();
            let stop = shutdown.clone();
            let (gate, start) = mpsc::sync_channel::<()>(1);
            match thread::Builder::new()
                .name(format!("mo-pipe-{slot:02}"))
                .spawn_scoped(scope, move || {
                    supervise_pipe_worker(&stop, || {
                        if start.recv().is_err() {
                            return Ok(());
                        }
                        let mut served = 0usize;
                        loop {
                            if stop.is_cancelled() {
                                return Ok(());
                            }
                            match listener.accept_reusable_first_frame(FIRST_FRAME_TIMEOUT) {
                                Ok((mut stream, hello)) => {
                                    let result = serve_authenticated_with_engine(
                                        &mut stream,
                                        hello,
                                        client.clone(),
                                    );
                                    drop(stream);
                                    if stop.is_cancelled() {
                                        return Ok(());
                                    }
                                    served += 1;
                                    if per_slot_limit.is_some() {
                                        result?;
                                    }
                                    // Expected peer failures are local to this
                                    // slot. Never block a worker on stderr (or
                                    // amplify untrusted traffic into logs).
                                    if per_slot_limit == Some(served) {
                                        return Ok(());
                                    }
                                }
                                Err(_error) => {
                                    if stop.is_cancelled() {
                                        return Ok(());
                                    }
                                    // A rejected/slow client consumes only its own slot.
                                    // The retained listener prevents a namespace gap.
                                    thread::sleep(Duration::from_millis(5));
                                }
                            }
                        }
                    })
                }) {
                Ok(worker) => {
                    gates.push(gate);
                    workers.push(worker);
                }
                Err(error) => {
                    drop(gates);
                    return Err(error);
                }
            }
        }
        for gate in gates {
            gate.send(())
                .map_err(|_| io::Error::other("pipe worker exited before startup"))?;
        }
        eprintln!("Mo broker listening on {slot_count} protected pipe slots");
        for worker in workers {
            let result = worker
                .join()
                .unwrap_or_else(|_| crate::lifecycle::fail_stop());
            if let Err(error) = result {
                shutdown.cancel()?;
                return Err(error);
            }
        }
        Ok(())
    });
    outcome.and(engine.shutdown())
}

pub(crate) struct PoolStopWatchdog {
    completed: PipeCancellation,
    thread: Option<thread::JoinHandle<()>>,
}

impl PoolStopWatchdog {
    pub(crate) fn start(shutdown: &PipeCancellation, budget: Duration) -> io::Result<Self> {
        let completed = PipeCancellation::new()?;
        let observed_completion = completed.clone();
        let observed_shutdown = shutdown.clone();
        let thread = thread::Builder::new()
            .name("mo-stop-watchdog".to_owned())
            .spawn(move || {
                if observed_shutdown
                    .wait_until_or_completed(&observed_completion)
                    .unwrap_or_else(|_| crate::lifecycle::fail_stop())
                    && !observed_completion
                        .wait_with_timeout(budget)
                        .unwrap_or_else(|_| crate::lifecycle::fail_stop())
                {
                    crate::lifecycle::fail_stop();
                }
            })?;
        Ok(Self {
            completed,
            thread: Some(thread),
        })
    }
}

impl Drop for PoolStopWatchdog {
    fn drop(&mut self) {
        self.completed
            .cancel()
            .unwrap_or_else(|_| crate::lifecycle::fail_stop());
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .unwrap_or_else(|_| crate::lifecycle::fail_stop());
        }
    }
}

pub(crate) fn supervise_pipe_worker<F>(shutdown: &PipeCancellation, worker: F) -> io::Result<()>
where
    F: FnOnce() -> io::Result<()>,
{
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(worker))
        .unwrap_or_else(|_| crate::lifecycle::fail_stop());
    if result.is_err() {
        shutdown
            .cancel()
            .unwrap_or_else(|_| crate::lifecycle::fail_stop());
    }
    result
}

pub fn serve_listener(listener: PipeListener) -> io::Result<()> {
    serve_listener_with_backend(listener, mo_engine::FakeBackend::new())
}

pub fn serve_listener_with_backend<B>(listener: PipeListener, backend: B) -> io::Result<()>
where
    B: EngineBackend,
{
    let (mut stream, first_frame) = listener.accept_first_frame(FIRST_FRAME_TIMEOUT)?;
    serve_authenticated_with_backend(&mut stream, first_frame, backend)
}

/// Serves successive authenticated clients while constructing and owning the
/// thread-affine backend on one dedicated engine thread.
///
/// Legacy serial compatibility helper, retained for focused tests. Installed
/// startup uses the independently secured retained-handle pool instead. This
/// helper re-arms its protected singleton after each disconnect.
pub fn serve_listener_loop_with_backend_factory<F, B, E>(
    mut listener: PipeListener,
    backend_factory: F,
    connection_limit: Option<usize>,
) -> io::Result<()>
where
    F: FnOnce() -> Result<B, E> + Send + 'static,
    B: EngineBackend + 'static,
    E: fmt::Display,
{
    if connection_limit == Some(0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "connection limit must be non-zero",
        ));
    }
    let engine = EngineService::start(backend_factory)?;
    let mut accepted = 0_usize;

    let outcome = loop {
        let accepted_pipe = listener.accept_next_first_frame(FIRST_FRAME_TIMEOUT);
        let (mut stream, first_frame) = match accepted_pipe {
            Ok(connection) => connection,
            Err(error) => {
                if let Err(rearm_error) = listener.rearm() {
                    break Err(rearm_error);
                }
                if connection_limit.is_none() {
                    eprintln!("rejected named-pipe connection: {error}");
                    continue;
                }
                break Err(error);
            }
        };
        accepted += 1;
        let engine_client = engine.client();
        let connection_result =
            serve_authenticated_with_engine(&mut stream, first_frame, engine_client);
        drop(stream);
        if let Err(error) = listener.rearm() {
            break Err(error);
        }
        if let Err(error) = connection_result {
            if connection_limit.is_some() {
                break Err(error);
            }
            eprintln!("named-pipe connection ended with error: {error}");
        }
        if connection_limit.is_some_and(|limit| accepted == limit) {
            break Ok(());
        }
    };
    let shutdown = engine.shutdown();
    outcome.and(shutdown)
}

pub fn serve_authenticated(stream: &mut AuthenticatedPipe, first_frame: Frame) -> io::Result<()> {
    serve_authenticated_with_backend(stream, first_frame, mo_engine::FakeBackend::new())
}

pub fn serve_authenticated_with_backend<B>(
    stream: &mut AuthenticatedPipe,
    first_frame: Frame,
    backend: B,
) -> io::Result<()>
where
    B: EngineBackend,
{
    let mut broker = BrokerConnection::with_backend(next_generation(), backend);
    serve_connection(stream, first_frame, &mut broker)
}

fn serve_authenticated_with_engine(
    stream: &mut AuthenticatedPipe,
    first_frame: Frame,
    engine: EngineClient,
) -> io::Result<()> {
    let mut broker = BrokerConnection::<FakeBackend>::with_engine_client(next_generation(), engine);
    serve_connection(stream, first_frame, &mut broker)
}

fn serve_connection<B>(
    stream: &mut AuthenticatedPipe,
    first_frame: Frame,
    broker: &mut BrokerConnection<B>,
) -> io::Result<()>
where
    B: EngineBackend,
{
    dispatch(stream, broker, first_frame)?;

    loop {
        let request = match stream.read_frame_after_activity(FRAME_ASSEMBLY_TIMEOUT) {
            Ok(frame) => frame,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::UnexpectedEof
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::ConnectionAborted
                        | io::ErrorKind::BrokenPipe
                ) =>
            {
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        dispatch(stream, broker, request)?;
    }
}

fn dispatch<B>(
    stream: &mut AuthenticatedPipe,
    broker: &mut BrokerConnection<B>,
    request: Frame,
) -> io::Result<()>
where
    B: EngineBackend,
{
    let response = broker.handle(request).map_err(io::Error::other)?;
    stream.write_frame_with_timeout(&response, RESPONSE_WRITE_TIMEOUT)
}

fn next_generation() -> u64 {
    let seed = *GENERATION_SEED.get_or_init(|| {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX)
            ^ u64::from(std::process::id()).rotate_left(32)
    });
    loop {
        // A fixed seed with unique counters is injective, unlike XORing each
        // counter with a different clock reading. These are correlation IDs,
        // not authentication secrets. Skip zero instead of aliasing it to one.
        let counter = GENERATION_COUNTER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("connection generation space exhausted");
        let generation = seed ^ counter;
        if generation != 0 {
            return generation;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mo_ipc::{
        CURRENT_VERSION, FEATURE_KEY_EVENTS, Hello, MAX_PAYLOAD_LEN, MessageKind, PayloadCodec,
        VersionRange, read_frame, write_frame,
    };
    use mo_windows_pipe::{PipeClient, pool_slot_address};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
    use std::time::Instant;

    fn test_hello() -> Frame {
        Frame::new(
            CURRENT_VERSION,
            MessageKind::Hello,
            0,
            0,
            0,
            1,
            Hello {
                supported: VersionRange::new(CURRENT_VERSION, CURRENT_VERSION).unwrap(),
                features: FEATURE_KEY_EVENTS,
                max_payload_len: MAX_PAYLOAD_LEN as u32,
            }
            .encode_payload()
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn cancelled_pool_does_not_initialize_the_backend() {
        let address = PipeAddress::new(&format!("broker-prestop-{}", std::process::id())).unwrap();
        let pool = PipePool::bind(address.clone(), 3).unwrap();
        let stop = PipeCancellation::new().unwrap();
        stop.cancel().unwrap();
        let initialized = Arc::new(AtomicBool::new(false));
        let observed = initialized.clone();
        serve_pool_with_shutdown(
            pool,
            move || {
                observed.store(true, AtomicOrdering::SeqCst);
                Ok::<_, io::Error>(FakeBackend::new())
            },
            stop,
            None,
        )
        .unwrap();
        assert!(!initialized.load(AtomicOrdering::SeqCst));
        assert!(PipePool::bind(address, 3).is_ok());
    }

    #[test]
    fn coordinated_stop_joins_all_sixteen_waiting_slots_and_releases_the_namespace() {
        let address = PipeAddress::new(&format!("broker-stop-all-{}", std::process::id())).unwrap();
        let pool = PipePool::bind(address.clone(), MAX_PIPE_SLOTS).unwrap();
        let stop = PipeCancellation::new().unwrap();
        let worker_stop = stop.clone();
        let (done, received) = mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            done.send(serve_pool_with_shutdown(
                pool,
                || Ok::<_, io::Error>(FakeBackend::new()),
                worker_stop,
                None,
            ))
            .unwrap();
        });
        thread::sleep(Duration::from_millis(30));
        let started = Instant::now();
        stop.cancel().unwrap();
        received
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        server.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(PipePool::bind(address, MAX_PIPE_SLOTS).is_ok());
    }

    #[test]
    fn coordinated_stop_wakes_idle_authenticated_partial_and_unconnected_slots() {
        use std::io::Write;
        let address =
            PipeAddress::new(&format!("broker-stop-mixed-{}", std::process::id())).unwrap();
        let pool = PipePool::bind(address.clone(), 3).unwrap();
        let stop = PipeCancellation::new().unwrap();
        let worker_stop = stop.clone();
        let (done, received) = mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            done.send(serve_pool_with_shutdown(
                pool,
                || Ok::<_, io::Error>(FakeBackend::new()),
                worker_stop,
                None,
            ))
            .unwrap();
        });
        let mut idle = PipeClient::connect(&address, Duration::from_secs(2)).unwrap();
        write_frame(&mut idle, &test_hello()).unwrap();
        let generation = read_frame(&mut idle).unwrap().header.connection_generation;
        write_frame(
            &mut idle,
            &Frame::new(
                CURRENT_VERSION,
                MessageKind::OpenSession,
                0,
                generation,
                0,
                2,
                vec![],
            )
            .unwrap(),
        )
        .unwrap();
        assert_ne!(read_frame(&mut idle).unwrap().header.session_token, 0);
        let mut partial = PipeClient::connect(
            &pool_slot_address(&address, 1).unwrap(),
            Duration::from_secs(2),
        )
        .unwrap();
        partial.write_all(b"M").unwrap();
        let started = Instant::now();
        stop.cancel().unwrap();
        received
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();
        server.join().unwrap();
        assert!(started.elapsed() < FIRST_FRAME_TIMEOUT);
        // Peers are deliberately still alive. Shutdown must not await EOF.
        drop(idle);
        drop(partial);
        assert!(PipePool::bind(address, 3).is_ok());
    }

    #[test]
    fn later_slot_failure_cancels_earlier_slots_before_coordinator_join() {
        use std::io::Write;
        let address =
            PipeAddress::new(&format!("broker-worker-error-{}", std::process::id())).unwrap();
        let pool = PipePool::bind(address.clone(), 3).unwrap();
        let stop = PipeCancellation::new().unwrap();
        let worker_stop = stop.clone();
        let (done, received) = mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            done.send(serve_pool_with_shutdown(
                pool,
                || Ok::<_, io::Error>(FakeBackend::new()),
                worker_stop,
                Some(1),
            ))
            .unwrap();
        });
        let mut broken = PipeClient::connect(
            &pool_slot_address(&address, 2).unwrap(),
            Duration::from_secs(2),
        )
        .unwrap();
        write_frame(&mut broken, &test_hello()).unwrap();
        read_frame(&mut broken).unwrap();
        // The later diagnostic worker exits on a corrupt frame while earlier
        // workers are still waiting. Protocol error replies alone are not fatal.
        let mut corrupt = Vec::new();
        write_frame(&mut corrupt, &test_hello()).unwrap();
        *corrupt.last_mut().unwrap() ^= 0x80;
        broken.write_all(&corrupt).unwrap();
        assert!(
            received
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .is_err()
        );
        assert!(stop.is_cancelled());
        server.join().unwrap();
        drop(broken);
        assert!(PipePool::bind(address, 3).is_ok());
    }

    #[test]
    fn truncated_and_corrupt_requests_retire_sessions_without_replaying_commits() {
        use super::{EngineService, FIRST_FRAME_TIMEOUT, serve_authenticated_with_engine};
        use mo_domain::{EngineCommand, EngineOutput, SessionOptions};
        use mo_engine::{EngineBackend, FakeBackend, FakeEvent};
        use mo_ipc::{
            CURRENT_VERSION, FEATURE_KEY_EVENTS, Frame, Hello, KeyEvent, MAX_PAYLOAD_LEN,
            MessageKind, PayloadCodec, Snapshot, VersionRange, read_frame, write_frame,
        };
        use mo_windows_pipe::{PipeAddress, PipeClient, PipeListener};
        use std::fs::File;
        use std::io::Write;
        use std::sync::{Arc, Mutex, mpsc};

        struct Traced {
            inner: FakeBackend,
            log: Arc<Mutex<Vec<FakeEvent>>>,
        }
        impl EngineBackend for Traced {
            type Session = <FakeBackend as EngineBackend>::Session;
            type Error = <FakeBackend as EngineBackend>::Error;
            fn create_session(
                &mut self,
                options: SessionOptions,
            ) -> Result<Self::Session, Self::Error> {
                let result = self.inner.create_session(options);
                *self.log.lock().unwrap() = self.inner.events().to_vec();
                result
            }
            fn apply(
                &mut self,
                session: &mut Self::Session,
                command: &EngineCommand,
            ) -> Result<EngineOutput, Self::Error> {
                let result = self.inner.apply(session, command);
                *self.log.lock().unwrap() = self.inner.events().to_vec();
                result
            }
            fn destroy_session(&mut self, session: Self::Session) -> Result<(), Self::Error> {
                let result = self.inner.destroy_session(session);
                *self.log.lock().unwrap() = self.inner.events().to_vec();
                result
            }
        }
        fn request(
            kind: MessageKind,
            generation: u64,
            token: u64,
            id: u64,
            payload: Vec<u8>,
        ) -> Frame {
            Frame::new(CURRENT_VERSION, kind, 0, generation, token, id, payload).unwrap()
        }
        fn open(address: &PipeAddress) -> (File, u64, u64) {
            let mut pipe = PipeClient::connect(address, std::time::Duration::from_secs(2)).unwrap();
            let hello = Hello {
                supported: VersionRange::new(CURRENT_VERSION, CURRENT_VERSION).unwrap(),
                features: FEATURE_KEY_EVENTS,
                max_payload_len: MAX_PAYLOAD_LEN as u32,
            };
            write_frame(
                &mut pipe,
                &request(MessageKind::Hello, 0, 0, 1, hello.encode_payload().unwrap()),
            )
            .unwrap();
            let generation = read_frame(&mut pipe).unwrap().header.connection_generation;
            write_frame(
                &mut pipe,
                &request(MessageKind::OpenSession, generation, 0, 2, vec![]),
            )
            .unwrap();
            let token = read_frame(&mut pipe).unwrap().header.session_token;
            (pipe, generation, token)
        }
        fn key(generation: u64, token: u64, id: u64, virtual_key: u32) -> Frame {
            let key = KeyEvent {
                virtual_key,
                scan_code: 0,
                modifiers: 0,
                key_down: true,
                repeat: false,
            };
            request(
                MessageKind::KeyEvent,
                generation,
                token,
                id,
                key.encode_payload().unwrap(),
            )
        }
        let address =
            PipeAddress::new(&format!("broker-fault-cleanup-{}", std::process::id())).unwrap();
        let mut listener = PipeListener::bind(address.clone()).unwrap();
        let log = Arc::new(Mutex::new(Vec::new()));
        let observed = log.clone();
        let (done, received) = mpsc::sync_channel(1);
        let server = std::thread::spawn(move || {
            let factory_log = log.clone();
            let engine = EngineService::start(move || {
                Ok::<_, std::io::Error>(Traced {
                    inner: FakeBackend::new(),
                    log: factory_log,
                })
            })
            .unwrap();
            for (index, expected) in [
                Some(std::io::ErrorKind::TimedOut),
                Some(std::io::ErrorKind::InvalidData),
                None,
            ]
            .into_iter()
            .enumerate()
            {
                let (mut stream, hello) = listener
                    .accept_reusable_first_frame(FIRST_FRAME_TIMEOUT)
                    .unwrap();
                let result = serve_authenticated_with_engine(&mut stream, hello, engine.client());
                assert_eq!(result.err().map(|error| error.kind()), expected);
                drop(stream);
                assert!(listener.has_expected_dacl().unwrap());
                let events = log.lock().unwrap();
                assert_eq!(
                    events
                        .iter()
                        .filter(|event| matches!(event, FakeEvent::SessionCreated { .. }))
                        .count(),
                    index + 1
                );
                assert_eq!(
                    events
                        .iter()
                        .filter(|event| matches!(event, FakeEvent::SessionDestroyed { .. }))
                        .count(),
                    index + 1
                );
                drop(events);
                done.send(()).unwrap();
            }
            engine.shutdown().unwrap();
        });
        let mut generations = Vec::new();
        for fault in 0..2 {
            let (mut pipe, generation, token) = open(&address);
            generations.push(generation);
            write_frame(&mut pipe, &key(generation, token, 3, u32::from(b'M'))).unwrap();
            assert_eq!(
                Snapshot::decode_payload(&read_frame(&mut pipe).unwrap().payload)
                    .unwrap()
                    .composition,
                "m"
            );
            let mut bytes = Vec::new();
            write_frame(&mut bytes, &key(generation, token, 4, 0x20)).unwrap();
            if fault == 0 {
                pipe.write_all(&bytes[..24]).unwrap();
            } else {
                *bytes.last_mut().unwrap() ^= 0x80;
                pipe.write_all(&bytes).unwrap();
            }
            received
                .recv_timeout(std::time::Duration::from_secs(4))
                .unwrap();
            drop(pipe);
        }
        let (mut pipe, generation, token) = open(&address);
        assert!(!generations.contains(&generation));
        for (id, symbol, commit) in [
            (3, u32::from(b'M'), None),
            (4, 0x20, Some("m")),
            (5, 0x20, None),
        ] {
            write_frame(&mut pipe, &key(generation, token, id, symbol)).unwrap();
            let snapshot =
                Snapshot::decode_payload(&read_frame(&mut pipe).unwrap().payload).unwrap();
            assert_eq!(snapshot.commit.as_deref(), commit);
            if id == 3 {
                assert_eq!(snapshot.composition, "m");
            }
        }
        drop(pipe);
        received
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        server.join().unwrap();
        // Two valid M keys before faults, then M/Space/Space on recovery. The
        // truncated/corrupt Space requests never reached the engine at all.
        assert_eq!(
            observed
                .lock()
                .unwrap()
                .iter()
                .filter(|event| matches!(event, FakeEvent::CommandApplied { .. }))
                .count(),
            5
        );
    }

    #[test]
    fn simultaneous_connection_generations_are_unique_and_nonzero() {
        let values = std::thread::scope(|scope| {
            let threads = (0..16)
                .map(|_| {
                    scope.spawn(|| {
                        (0..64)
                            .map(|_| super::next_generation())
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>();
            threads
                .into_iter()
                .flat_map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(values.len(), 1024);
        assert!(!values.contains(&0));
        assert_eq!(
            values
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            1024
        );
    }
}
