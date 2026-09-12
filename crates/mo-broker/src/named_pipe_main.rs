#[cfg(windows)]
fn main() -> std::io::Result<()> {
    use mo_broker::windows_runtime::{StartupMode, parse_startup};
    use std::io;

    let mode = parse_startup(std::env::args_os().skip(1).collect())?;
    let listener = mo_broker::windows_named_pipe::bind_default()?;
    match mode {
        StartupMode::Fake => {
            eprintln!(
                "Mo broker diagnostic backend listening on {}",
                listener.address().as_str()
            );
            mo_broker::windows_named_pipe::serve_listener(listener)
        }
        StartupMode::Rime(startup) => {
            let engine = mo_rime::Engine::load(startup.engine_config, startup.dll_path)
                .map_err(io::Error::other)?;
            let backend = mo_rime::RimeBackend::new(engine);
            eprintln!(
                "Mo broker librime backend listening on {}",
                listener.address().as_str()
            );
            mo_broker::windows_named_pipe::serve_listener_with_backend(listener, backend)
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("mo-broker currently supports Windows only");
    std::process::exit(1);
}
