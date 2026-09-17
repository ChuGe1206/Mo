//! Content-free, direct API timing of synthetic input in a disposable fixture.
//! Never run this against a real user's Rime directory.

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use mo_rime::{Engine, EngineConfig};
    use std::{io, path::PathBuf, time::Instant};

    let arguments = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let (paths, keep_resources) = match arguments.as_slice() {
        [dll, shared, user] => ([dll, shared, user], false),
        [dll, shared, user, flag] if flag == std::path::Path::new("--keep-resources") => ([dll, shared, user], true),
        _ => return Err(io::Error::new(io::ErrorKind::InvalidInput,
            "usage: latency_probe <absolute-rime.dll> <absolute-shared> <absolute-disposable-user> [--keep-resources]").into()),
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
    let engine = Engine::load(EngineConfig::new(unicode(shared)?, unicode(user)?), dll)?;
    // No synthetic key or commit is ever sent to this resource owner.
    let _resource_owner = keep_resources
        .then(|| engine.create_session())
        .transpose()?;
    for trial in 0..20 {
        let started = Instant::now();
        let mut session = engine.create_session()?;
        let create_us = started.elapsed().as_micros();
        for pass in 0..2 {
            let started = Instant::now();
            assert!(session.process_key(i32::from(b'n'), 0));
            let process_us = started.elapsed().as_micros();
            let started = Instant::now();
            assert!(
                session.take_commit()?.is_none(),
                "synthetic input must not commit"
            );
            let commit_us = started.elapsed().as_micros();
            let started = Instant::now();
            assert!(session.context()?.is_some());
            let context_us = started.elapsed().as_micros();
            let started = Instant::now();
            assert!(session.status()?.is_some_and(|status| status.is_composing));
            let status_us = started.elapsed().as_micros();
            session.clear_composition();
            assert!(
                session.take_commit()?.is_none(),
                "clear must not produce a commit"
            );
            assert!(
                session
                    .context()?
                    .is_none_or(|context| context.composition.preedit.is_empty())
            );
            println!(
                "MO_NATIVE trial={trial} pass={pass} create_us={create_us} process_us={process_us} commit_us={commit_us} context_us={context_us} status_us={status_us}"
            );
        }
        session.close()?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("latency_probe currently requires Windows");
    std::process::exit(1);
}
