//! Disposable real-librime probe for Mo's per-session learning gate.

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io;
    use std::path::PathBuf;

    use mo_rime::{Engine, EngineConfig};

    let arguments = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let [dll, shared, user, disable] = arguments.as_slice() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: learning_smoke <absolute-rime.dll> <absolute-shared> <absolute-disposable-user> <true|false>",
        )
        .into());
    };
    if [dll, shared, user].iter().any(|path| !path.is_absolute()) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "paths must be absolute").into());
    }
    let disable = match disable.to_str() {
        Some("true") => true,
        Some("false") => false,
        _ => {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "expected true or false").into(),
            );
        }
    };
    let mut config = EngineConfig::new(shared.to_str().unwrap(), user.to_str().unwrap());
    config.prebuilt_data_dir = Some(shared.join("build").to_str().unwrap().to_owned());
    config.staging_dir = config.prebuilt_data_dir.clone();
    let engine = Engine::load(config, dll)?;
    let mut session = engine.create_session()?;
    session.select_schema("rime_ice")?;
    session.set_option("mo_disable_learning", disable)?;
    for key in b"nihao" {
        assert!(session.process_key(i32::from(*key), 0));
    }
    let context = session.context()?.ok_or("missing context")?;
    assert!(
        context
            .menu
            .candidates
            .iter()
            .any(|candidate| candidate.text == "你好")
    );
    assert!(session.select_candidate_on_current_page(0)?);
    let commit = session.take_commit()?.ok_or("missing commit")?;
    assert_eq!(commit.text, "你好");
    let _ = session.process_key(i32::from(b' '), 0);
    session.close()?;
    drop(engine);
    println!("mo_disable_learning={disable}: 你好 committed");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("learning_smoke currently requires Windows");
    std::process::exit(1);
}
