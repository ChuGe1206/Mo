#[cfg(windows)]
fn main() -> std::io::Result<()> {
    let listener = mo_broker::windows_named_pipe::bind_default()?;
    eprintln!("Mo broker listening on {}", listener.address().as_str());
    mo_broker::windows_named_pipe::serve_listener(listener)
}

#[cfg(not(windows))]
fn main() {
    eprintln!("mo-broker currently supports Windows only");
    std::process::exit(1);
}
