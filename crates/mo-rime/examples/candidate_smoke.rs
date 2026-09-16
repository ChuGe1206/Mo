//! Diagnostic proof of page-local candidate commands against real librime.

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
            "usage: candidate_smoke <absolute-rime.dll> <absolute-shared> <absolute-disposable-user>").into());
    };
    if arguments.iter().any(|path| !path.is_absolute()) {
        return Err(
            io::Error::new(io::ErrorKind::InvalidInput, "all paths must be absolute").into(),
        );
    }
    let config = EngineConfig::new(
        shared.to_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "shared path must be Unicode")
        })?,
        user.to_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "user path must be Unicode")
        })?,
    );
    let mut actor = EngineActor::new(RimeBackend::new(Engine::load(config, dll)?));
    let token = actor.create_session(SessionOptions::new())?;
    actor.dispatch(token, EngineCommand::Key(KeyEvent::text('n')))?;
    let first = actor.dispatch(token, EngineCommand::Key(KeyEvent::text('i')))?;
    assert!(
        first.handled && first.candidates.len() >= 2,
        "ni must have at least two candidates"
    );
    let expected_selection = first.candidates[1].text.clone();
    let first_texts = first
        .candidates
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect::<Vec<_>>();
    let next = actor.dispatch(token, EngineCommand::ChangePage { backward: false })?;
    assert!(
        next.handled && next.commit.is_none(),
        "page next must not commit text"
    );
    let next_texts = next
        .candidates
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect::<Vec<_>>();
    assert!(
        !next_texts.is_empty() && next_texts != first_texts,
        "page next must change the candidate page"
    );
    let previous = actor.dispatch(token, EngineCommand::ChangePage { backward: true })?;
    assert!(previous.handled && previous.commit.is_none());
    assert_eq!(
        previous
            .candidates
            .iter()
            .map(|candidate| candidate.text.clone())
            .collect::<Vec<_>>(),
        first_texts,
        "page previous must restore the original page"
    );
    let selected = actor.dispatch(token, EngineCommand::SelectCandidate { index: 1 })?;
    assert!(selected.handled);
    assert_eq!(
        selected.commit.as_deref(),
        Some(expected_selection.as_str()),
        "selection must use a zero-based current-page index"
    );
    assert!(
        selected.composition.is_none(),
        "selection must end the ni composition"
    );
    actor.destroy_session(token)?;
    println!("Real librime/rime-ice Actor candidate paging and page-local selection passed.");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("candidate_smoke currently requires Windows");
    std::process::exit(1);
}
