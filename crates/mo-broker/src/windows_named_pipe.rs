//! Authenticated Windows named-pipe adapter for the broker state machine.
//!
//! Every connection is validated before dispatching even its `Hello` frame.
//! Successive connections keep independent protocol/session-token state while
//! one process-wide engine service preserves backend state and ordering.

use std::fmt;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mo_engine::{EngineBackend, FakeBackend};
use mo_ipc::{Frame, FrameError, read_frame, write_frame};
use mo_windows_pipe::{AuthenticatedPipe, PipeAddress, PipeListener};

use crate::BrokerConnection;
use crate::engine_service::{EngineClient, EngineService};

pub const DEFAULT_ENDPOINT: &str = "Broker.v1";
pub const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(2);

static GENERATION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn bind_default() -> io::Result<PipeListener> {
    PipeListener::bind(PipeAddress::new(DEFAULT_ENDPOINT)?)
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
/// The listener remains intentionally single-instance until the TIP can
/// authenticate the Broker server. This loop nevertheless keeps librime alive
/// and safely re-arms the protected pipe after each disconnect.
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
        let request = match read_frame(stream) {
            Ok(frame) => frame,
            Err(FrameError::Io(error))
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
            Err(error) => return Err(io::Error::new(io::ErrorKind::InvalidData, error)),
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
    write_frame(stream, &response)
        .map_err(|error| io::Error::new(io::ErrorKind::BrokenPipe, error))?;
    stream.flush()
}

fn next_generation() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let time_bits = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
    let counter = GENERATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    (time_bits ^ counter.rotate_left(17) ^ u64::from(std::process::id())).max(1)
}
