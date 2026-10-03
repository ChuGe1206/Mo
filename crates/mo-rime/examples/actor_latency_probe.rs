//! Content-free, direct Actor timing with the installed Broker's session options.
//! Run only with a disposable, marker-guarded compiled Rime user directory.

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use mo_domain::{EngineCommand, KeyEvent, SessionOptions};
    use mo_engine::EngineActor;
    use mo_rime::{Engine, EngineConfig, RimeBackend};
    use std::{io, path::PathBuf, time::Instant};

    let args = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let (paths, broker_plan) = match args.as_slice() {
        [dll, shared, user] => ([dll, shared, user], false),
        [dll, shared, user, flag] if flag == std::path::Path::new("--broker-plan") => {
            ([dll, shared, user], true)
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: actor_latency_probe <absolute-rime.dll> <absolute-shared> <absolute-disposable-user> [--broker-plan]",
            )
            .into());
        }
    };
    let [dll, shared, user] = paths;
    if paths.iter().any(|path| !path.is_absolute()) || !user.join("mo-latency-fixture").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "absolute paths and a disposable mo-latency-fixture marker are required",
        )
        .into());
    }
    let unicode = |path: &std::path::Path| -> Result<String, io::Error> {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "paths must be Unicode"))
    };
    let started = Instant::now();
    let engine = Engine::load(EngineConfig::new(unicode(shared)?, unicode(user)?), dll)?;
    let load_us = started.elapsed().as_micros();
    let started = Instant::now();
    let mut actor = EngineActor::new(RimeBackend::with_prepared_resources(engine)?);
    let prepare_us = started.elapsed().as_micros();
    println!("MO_ACTOR_READY load_us={load_us} prepare_us={prepare_us} input_free=true");

    for trial in 0..2 {
        let mut options = SessionOptions::new();
        if broker_plan {
            options = options
                .with_schema("rime_ice")
                .with_option("traditionalization", false)
                .with_option("emoji", true)
                .with_option("mo_disable_learning", true);
        }
        let started = Instant::now();
        let token = actor.create_session(options)?;
        let create_us = started.elapsed().as_micros();
        let started = Instant::now();
        let snapshot = actor.dispatch(token, EngineCommand::Key(KeyEvent::text('n')))?;
        let dispatch_us = started.elapsed().as_micros();
        assert!(snapshot.handled && snapshot.commit.is_none());
        assert_eq!(snapshot.status.schema_id, "rime_ice");
        assert!(snapshot.status.composing);
        println!(
            "MO_ACTOR trial={trial} broker_plan={broker_plan} create_us={create_us} dispatch_us={dispatch_us} candidate_count={}",
            snapshot.candidates.len()
        );
        let cleared = actor.dispatch(token, EngineCommand::Clear)?;
        assert!(cleared.commit.is_none());
        actor.destroy_session(token)?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("actor_latency_probe currently requires Windows");
    std::process::exit(1);
}
