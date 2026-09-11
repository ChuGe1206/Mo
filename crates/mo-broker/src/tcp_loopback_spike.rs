//! Explicitly non-production TCP loopback transport for Phase 0 diagnostics.
//!
//! This module does not implement, emulate, or validate the planned Windows
//! named pipe, its DACL, peer identity checks, or AppContainer access.

use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mo_ipc::{FrameError, read_frame, write_frame};

use crate::BrokerConnection;

static GENERATION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn bind(address: SocketAddr) -> io::Result<TcpListener> {
    if !address.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the Phase 0 TCP spike may bind only to a loopback address",
        ));
    }
    TcpListener::bind(address)
}

pub fn serve_listener(listener: TcpListener, accept_once: bool) -> io::Result<()> {
    let address = listener.local_addr()?;
    eprintln!(
        "Mo Phase 0 diagnostic TCP spike listening on {address}; this is NOT the production named-pipe transport"
    );
    for accepted in listener.incoming() {
        let stream = accepted?;
        if !stream.peer_addr()?.ip().is_loopback() {
            continue;
        }
        if let Err(error) = serve_stream(stream) {
            eprintln!("diagnostic connection ended with error: {error}");
        }
        if accept_once {
            break;
        }
    }
    Ok(())
}

pub fn serve_stream(mut stream: TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.set_nodelay(true)?;
    let mut broker = BrokerConnection::new(next_generation());
    loop {
        let request = match read_frame(&mut stream) {
            Ok(frame) => frame,
            Err(FrameError::Io(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::UnexpectedEof
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::ConnectionAborted
                ) =>
            {
                return Ok(());
            }
            Err(error) => return Err(io::Error::new(io::ErrorKind::InvalidData, error)),
        };
        let response = broker.handle(request).map_err(io::Error::other)?;
        write_frame(&mut stream, &response)
            .map_err(|error| io::Error::new(io::ErrorKind::BrokenPipe, error))?;
    }
}

fn next_generation() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let time_bits = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
    let counter = GENERATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    (time_bits ^ counter.rotate_left(17) ^ u64::from(std::process::id())).max(1)
}
