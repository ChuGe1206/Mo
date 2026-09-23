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
            let startup = *startup;
            let settings_path = startup.settings_path;
            let factory = move || {
                let engine = mo_rime::Engine::load(startup.engine_config, startup.dll_path)?;
                if startup.require_prepared_resources {
                    mo_rime::RimeBackend::with_prepared_resources(engine)
                } else {
                    mo_rime::RimeBackend::with_resource_anchor(engine)
                }
            };
            if let Some(settings_path) = settings_path {
                mo_broker::windows_named_pipe::serve_pool_with_backend_factory_and_settings(
                    pool,
                    factory,
                    settings_path,
                    None,
                )
            } else {
                mo_broker::windows_named_pipe::serve_pool_with_backend_factory(pool, factory, None)
            }
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("mo-broker currently supports Windows only");
    std::process::exit(1);
}
