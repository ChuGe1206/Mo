//! Test-only preparation success/failure checks in a marked disposable fixture.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use mo_rime::{Engine, EngineConfig, Error};
    use std::{io, path::PathBuf};
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let [dll, shared, user, outcome] = args.as_slice() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: preparation_probe <dll> <shared> <disposable-user> success|failure|missing",
        )
        .into());
    };
    let paths = [dll, shared, user].map(PathBuf::from);
    if paths.iter().any(|p| !p.is_absolute()) || !paths[2].join("mo-preparation-fixture").is_file()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "absolute paths and test marker required",
        )
        .into());
    }
    let unicode = |p: &std::path::Path| {
        p.to_str()
            .map(str::to_owned)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Unicode paths required"))
    };
    let engine = Engine::load(
        EngineConfig::new(unicode(&paths[1])?, unicode(&paths[2])?),
        &paths[0],
    )?;
    let mut anchor = engine.create_session()?;
    let before = anchor.context()?;
    assert!(anchor.take_commit()?.is_none());
    let result = anchor.prepare_resources();
    match outcome.to_str() {
        Some("success") => result?,
        Some("failure") => assert_eq!(
            result,
            Err(Error::NativeCallFailed("mo_rime_prepare_resources_v2"))
        ),
        Some("missing") => assert_eq!(
            result,
            Err(Error::MissingFunction("mo_rime_prepare_resources_v2"))
        ),
        _ => {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "invalid expected outcome").into(),
            );
        }
    }
    assert_eq!(
        anchor.context()?,
        before,
        "preparation must not change input"
    );
    assert!(anchor.take_commit()?.is_none());
    assert!(anchor.status()?.is_some_and(|s| !s.is_composing));
    if outcome == "success" {
        let mut session = engine.create_session()?;
        for key in b"nihao" {
            assert!(session.process_key(i32::from(*key), 0));
        }
        let before = session.context()?;
        assert_eq!(
            session.prepare_resources(),
            Err(Error::NativeCallFailed("mo_rime_prepare_resources_v2"))
        );
        assert_eq!(session.context()?, before);
        assert!(session.take_commit()?.is_none());
        let mut found = false;
        for _ in 0..10 {
            let context = session.context()?.expect("candidate page");
            found |= context.menu.candidates.iter().any(|c| c.text == "👋");
            if context.menu.is_last_page {
                break;
            }
            assert!(session.change_page(false)?);
        }
        assert!(found, "preparation must preserve Emoji candidates");
        session.clear_composition();
        assert!(session.take_commit()?.is_none());
        for key in b"nihao" {
            assert!(session.process_key(i32::from(*key), 0));
        }
        assert!(session.select_candidate_on_current_page(0)?);
        assert_eq!(
            session.take_commit()?.expect("selected commit").text,
            "你好"
        );
        assert!(session.take_commit()?.is_none());
        session.close()?;
    }
    anchor.close()?;
    println!("Mo input-free preparation boundary probe passed.");
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("preparation_probe requires Windows");
    std::process::exit(1);
}
