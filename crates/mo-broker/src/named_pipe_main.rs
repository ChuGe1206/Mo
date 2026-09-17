#[cfg(windows)]
fn main() -> std::io::Result<()> {
    use mo_broker::windows_runtime::{StartupMode, parse_startup};
    #[cfg(debug_assertions)]
    use std::io;

    let mode = parse_startup(std::env::args_os().skip(1).collect())?;
    let pool = mo_broker::windows_named_pipe::bind_default_pool()?;
    match mode {
        #[cfg(debug_assertions)]
        StartupMode::Fake => mo_broker::windows_named_pipe::serve_pool_with_backend_factory(
            pool,
            move || Ok::<_, io::Error>(mo_engine::FakeBackend::new()),
            None,
        ),
        StartupMode::Rime(startup) => {
            mo_broker::windows_named_pipe::serve_pool_with_backend_factory(
                pool,
                move || {
                    let engine = mo_rime::Engine::load(startup.engine_config, startup.dll_path)?;
                    if startup.require_prepared_resources {
                        mo_rime::RimeBackend::with_prepared_resources(engine)
                    } else {
                        mo_rime::RimeBackend::with_resource_anchor(engine)
                    }
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
