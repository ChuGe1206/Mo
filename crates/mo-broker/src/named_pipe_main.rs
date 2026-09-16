#[cfg(windows)]
fn main() -> std::io::Result<()> {
    use mo_broker::windows_runtime::{StartupMode, parse_startup};
    #[cfg(debug_assertions)]
    use std::io;

    let mode = parse_startup(std::env::args_os().skip(1).collect())?;
    let listener = mo_broker::windows_named_pipe::bind_default()?;
    match mode {
        #[cfg(debug_assertions)]
        StartupMode::Fake => {
            let endpoint = listener.address().as_str().to_owned();
            mo_broker::windows_named_pipe::serve_listener_loop_with_backend_factory(
                listener,
                move || {
                    eprintln!("Mo broker diagnostic backend listening on {endpoint}");
                    Ok::<_, io::Error>(mo_engine::FakeBackend::new())
                },
                None,
            )
        }
        StartupMode::Rime(startup) => {
            let endpoint = listener.address().as_str().to_owned();
            mo_broker::windows_named_pipe::serve_listener_loop_with_backend_factory(
                listener,
                move || {
                    mo_rime::Engine::load(startup.engine_config, startup.dll_path)
                        .map(mo_rime::RimeBackend::new)
                        .inspect(|_| eprintln!("Mo broker librime backend listening on {endpoint}"))
                },
                None,
            )
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("mo-broker currently supports Windows only");
    std::process::exit(1);
}
