//! Build-only golden checks using managed prebuilt data and an empty staging dir.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use mo_rime::{Engine, EngineConfig};
    use std::{io, path::PathBuf};

    fn select_golden(
        engine: &Engine,
        input: &str,
        expected: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut session = engine.create_session()?;
        for key in input.bytes() {
            assert!(
                session.process_key(i32::from(key), 0),
                "rejected golden key"
            );
            assert!(session.take_commit()?.is_none(), "early golden commit");
            let _ = session.context()?;
        }
        let mut found = false;
        for _ in 0..12 {
            let context = session.context()?.expect("golden candidate context");
            if let Some(index) = context
                .menu
                .candidates
                .iter()
                .position(|candidate| candidate.text == expected)
            {
                assert!(session.select_candidate_on_current_page(index)?);
                assert_eq!(
                    session.take_commit()?.expect("golden commit").text,
                    expected
                );
                assert!(session.take_commit()?.is_none(), "duplicate golden commit");
                found = true;
                break;
            }
            if context.menu.is_last_page {
                break;
            }
            assert!(session.change_page(false)?);
        }
        assert!(found, "missing golden candidate for {input}: {expected}");
        session.close()?;
        println!("Resource pack golden passed: {input} -> {expected}");
        Ok(())
    }

    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let [dll, shared, user, date] = arguments.as_slice() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: resource_pack_smoke <dll> <shared> <marked-empty-user> <local-date>",
        )
        .into());
    };
    let paths = [dll, shared, user].map(PathBuf::from);
    if paths.iter().any(|path| !path.is_absolute())
        || !paths[2].join("mo-resource-pack-fixture").is_file()
    {
        return Err(io::Error::other("absolute paths and disposable marker required").into());
    }
    let staging = paths[2].join("build");
    if std::fs::read_dir(&staging)?.next().is_some() {
        return Err(io::Error::other("golden staging must start empty").into());
    }
    let unicode = |path: &std::path::Path| {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| io::Error::other("Unicode paths required"))
    };
    let mut config = EngineConfig::new(unicode(&paths[1])?, unicode(&paths[2])?);
    config.prebuilt_data_dir = Some(unicode(&paths[1].join("build"))?);
    config.staging_dir = Some(unicode(&staging)?);
    {
        let engine = Engine::load(config, &paths[0])?;
        let mut anchor = engine.create_session()?;
        anchor.prepare_resources()?;
        let date = date
            .to_str()
            .ok_or_else(|| io::Error::other("Unicode date required"))?;
        for (input, expected) in [
            ("nihao", "你好"),
            ("nihao", "👋"),
            ("hello", "hello"),
            ("rq", date),
            ("U4e2d", "中"),
            ("R123", "一百二十三"),
            ("cC1+2", "3"),
        ] {
            select_golden(&engine, input, expected)?;
        }
        anchor.close()?;
    }
    assert!(
        std::fs::read_dir(&staging)?.next().is_none(),
        "input must use the prebuilt pack, not generate schema/dictionary outputs"
    );
    println!("Managed prebuilt pack passed seven golden selections without staging compilation.");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("resource_pack_smoke currently requires Windows");
    std::process::exit(1);
}
