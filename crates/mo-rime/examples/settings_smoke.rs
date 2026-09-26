//! Explicit, disposable real-librime check for Mo's selectable schemas.

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io;
    use std::path::PathBuf;

    use mo_domain::{EngineCommand, KeyEvent, SessionOptions};
    use mo_engine::EngineActor;
    use mo_rime::{Engine, EngineConfig, RimeBackend};

    let arguments = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let [dll, shared, user] = arguments.as_slice() else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,
            "usage: settings_smoke <absolute-rime.dll> <absolute-shared> <absolute-disposable-user>").into());
    };
    if arguments.iter().any(|path| !path.is_absolute()) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "paths must be absolute").into());
    }
    let mut config = EngineConfig::new(shared.to_str().unwrap(), user.to_str().unwrap());
    config.prebuilt_data_dir = Some(shared.join("build").to_str().unwrap().to_owned());
    config.staging_dir = config.prebuilt_data_dir.clone();
    let engine = Engine::load(config.clone(), dll)?;
    let mut actor = EngineActor::new(RimeBackend::with_prepared_resources(engine)?);
    for schema in [
        "rime_ice",
        "double_pinyin",
        "double_pinyin_flypy",
        "double_pinyin_mspy",
        "double_pinyin_sogou",
    ] {
        for traditional in [false, true] {
            let options = SessionOptions::new()
                .with_schema(schema)
                .with_option("traditionalization", traditional);
            let token = actor.create_session(options)?;
            let snapshot = actor.dispatch(token, EngineCommand::Key(KeyEvent::text('n')))?;
            assert_eq!(snapshot.status.schema_id, schema);
            assert!(
                snapshot.handled,
                "{schema} must accept a first composition key"
            );
            actor.destroy_session(token)?;
        }
    }
    drop(actor);
    let engine = Engine::load(config, dll)?;
    for (traditional, expected) in [(false, "中国"), (true, "中國")] {
        let mut session = engine.create_session()?;
        session.select_schema("rime_ice")?;
        session.set_option("traditionalization", traditional)?;
        for key in "zhongguo".bytes() {
            assert!(session.process_key(i32::from(key), 0));
        }
        let context = session.context()?.ok_or("missing librime context")?;
        assert!(
            context
                .menu
                .candidates
                .iter()
                .any(|candidate| candidate.text == expected),
            "{expected} must appear in the selected character mode"
        );
        session.close()?;
    }
    println!("Five schemas x two modes compose; 中国/中國 conversion verified.");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("settings_smoke currently requires Windows");
    std::process::exit(1);
}
