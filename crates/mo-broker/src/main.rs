use std::env;
use std::io;
use std::net::SocketAddr;

use mo_broker::tcp_loopback_spike;

fn main() -> io::Result<()> {
    let mut address: SocketAddr = "127.0.0.1:0"
        .parse()
        .expect("built-in loopback address is valid");
    let mut accept_once = false;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--listen" => {
                let value = arguments.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--listen requires ADDRESS:PORT",
                    )
                })?;
                address = value.parse().map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("invalid --listen address: {error}"),
                    )
                })?;
            }
            "--once" => accept_once = true,
            "--help" | "-h" => {
                println!(
                    "mo-broker-tcp-spike [--listen 127.0.0.1:PORT] [--once]\n\
                     Diagnostic only: this is not the production Windows named-pipe broker."
                );
                return Ok(());
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument: {argument}"),
                ));
            }
        }
    }

    let listener = tcp_loopback_spike::bind(address)?;
    tcp_loopback_spike::serve_listener(listener, accept_once)
}
