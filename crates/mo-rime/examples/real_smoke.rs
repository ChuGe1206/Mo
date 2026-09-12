//! Opt-in smoke test against a real, pinned librime dynamic library.
//!
//! The caller must deploy Rime data first and arrange for `rime.dll` to be in
//! the Windows loader search path. This example is intentionally excluded from
//! the default workspace tests.

use std::error::Error;
use std::io;

use mo_rime::{Engine, EngineConfig};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let shared_data_dir = arguments
        .next()
        .ok_or_else(|| io::Error::other("missing shared data directory"))?;
    let user_data_dir = arguments
        .next()
        .ok_or_else(|| io::Error::other("missing user data directory"))?;
    if arguments.next().is_some() {
        return Err(io::Error::other("expected exactly two directory arguments").into());
    }

    let config = EngineConfig::new(
        shared_data_dir.to_string_lossy(),
        user_data_dir.to_string_lossy(),
    );
    let engine = Engine::open(config)?;
    let mut session = engine.create_session()?;

    for key in b"nihao" {
        if !session.process_key(i32::from(*key), 0) {
            return Err(
                io::Error::other(format!("librime rejected input byte 0x{key:02x}")).into(),
            );
        }
        // A real frontend requests a fresh owned snapshot after every key.
        // Keep the linked smoke aligned with the Broker path instead of only
        // observing the final composition.
        if session.take_commit()?.is_some() {
            return Err(io::Error::other("librime committed before the selection key").into());
        }
        let _ = session.context()?;
        let _ = session.status()?;
    }

    let context = session
        .context()?
        .ok_or_else(|| io::Error::other("librime returned no context after `nihao`"))?;
    let candidates = context
        .menu
        .candidates
        .iter()
        .map(|candidate| candidate.text.as_str())
        .collect::<Vec<_>>();
    if !candidates.contains(&"你好") {
        return Err(io::Error::other(format!(
            "expected `你好` among candidates after `nihao`; got {candidates:?}"
        ))
        .into());
    }

    if !session.process_key(i32::from(b' '), 0) {
        return Err(io::Error::other("librime rejected the candidate commit key").into());
    }
    let commit = session
        .take_commit()?
        .ok_or_else(|| io::Error::other("librime produced no commit after Space"))?;
    if commit.text != "你好" {
        return Err(io::Error::other(format!(
            "expected committed text `你好`; got {:?}",
            commit.text
        ))
        .into());
    }

    session.close()?;
    println!("real librime smoke passed: nihao -> {}", commit.text);
    Ok(())
}
