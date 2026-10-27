//! Opt-in, synthetic printable-key audit against a sealed real librime payload.
//!
//! Use a fresh disposable user directory. Output contains only case identifiers
//! and result categories, never preedit, candidate or committed text.

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use mo_domain::{EngineCommand, KeyEvent, KeyModifiers, KeyState, SessionOptions};
    use mo_engine::EngineActor;
    use mo_rime::{Engine, EngineConfig, RimeBackend};
    use std::{io, path::PathBuf, thread, time::Duration};

    const SLOW_KEY_INTERVAL: Duration = Duration::from_millis(200);
    const REPEATED_SLASH_COUNT: usize = 4;
    const XK_SHIFT_LEFT: u32 = 0xffe1;
    let paths = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let [dll, shared, user] = paths.as_slice() else {
        return Err(io::Error::other(
            "usage: key_smoke <absolute-rime.dll> <absolute-shared> <fresh-user>",
        )
        .into());
    };
    if paths.iter().any(|p| !p.is_absolute()) || user.exists() {
        return Err(
            io::Error::other("absolute payload paths and a fresh user directory required").into(),
        );
    }
    std::fs::create_dir_all(user)?;
    let mut config = EngineConfig::new(
        shared
            .to_str()
            .ok_or_else(|| io::Error::other("shared path is not Unicode"))?,
        user.to_str()
            .ok_or_else(|| io::Error::other("user path is not Unicode"))?,
    );
    config.prebuilt_data_dir = Some(shared.join("build").to_string_lossy().into_owned());
    config.staging_dir = config.prebuilt_data_dir.clone();
    let mut actor = EngineActor::new(RimeBackend::with_prepared_resources(Engine::load(
        config, dll,
    )?)?);
    let mut swallowed = 0;
    let mode_token = actor.create_session(
        SessionOptions::new()
            .with_schema("rime_ice")
            .with_option("mo_disable_learning", true),
    )?;
    for ascii_mode in [false, true, false] {
        let changed = actor.dispatch(
            mode_token,
            EngineCommand::SetOption {
                name: "ascii_mode".into(),
                value: ascii_mode,
            },
        )?;
        let inspected = actor.dispatch(mode_token, EngineCommand::Inspect)?;
        if changed.status.ascii_mode != ascii_mode
            || inspected.status.ascii_mode != ascii_mode
            || inspected.handled
            || inspected.commit.is_some()
            || inspected.composition.is_some()
        {
            return Err(
                io::Error::other("authoritative input mode inspection/toggle failed").into(),
            );
        }
    }
    actor.destroy_session(mode_token)?;
    println!("Real input mode inspection and idle toggles passed.");
    for ascii_mode in [false, true] {
        for byte in b'!'..=b'~' {
            let token = actor.create_session(
                SessionOptions::new()
                    .with_schema("rime_ice")
                    .with_option("mo_disable_learning", true),
            )?;
            if ascii_mode {
                actor.dispatch(
                    token,
                    EngineCommand::Key(KeyEvent::new(
                        XK_SHIFT_LEFT,
                        KeyModifiers::SHIFT,
                        KeyState::Pressed,
                        None,
                        false,
                    )),
                )?;
                let switched = actor.dispatch(
                    token,
                    EngineCommand::Key(KeyEvent::new(
                        XK_SHIFT_LEFT,
                        KeyModifiers::NONE,
                        KeyState::Released,
                        None,
                        false,
                    )),
                )?;
                if !switched.status.ascii_mode {
                    return Err(io::Error::other("ASCII switch failed").into());
                }
            }
            let key =
                actor.dispatch(token, EngineCommand::Key(KeyEvent::text(char::from(byte))))?;
            let invisible = key.handled
                && key.commit.is_none()
                && key.composition.is_none()
                && key.candidates.is_empty();
            swallowed += usize::from(invisible);
            println!(
                "case={byte} ascii_mode={ascii_mode} handled={} commit={} composition={} candidates={} invisible={invisible}",
                key.handled,
                key.commit.is_some(),
                key.composition.is_some(),
                !key.candidates.is_empty()
            );
            actor.destroy_session(token)?;
        }
    }
    let token = actor.create_session(
        SessionOptions::new()
            .with_schema("rime_ice")
            .with_option("mo_disable_learning", true),
    )?;
    let unsupported = actor.dispatch(
        token,
        EngineCommand::Key(KeyEvent::new(
            0,
            KeyModifiers::NONE,
            KeyState::Pressed,
            None,
            false,
        )),
    )?;
    if unsupported.handled || unsupported.commit.is_some() || unsupported.composition.is_some() {
        return Err(io::Error::other("NoSymbol changed idle input").into());
    }
    actor.destroy_session(token)?;
    // Physical Shift down -> shifted symbol -> symbol up -> Shift up.
    // These are the 21 shifted number-row/OEM symbols of the supported layout.
    for character in ")!@#$%^&*(:+<_>?~{|}\"".chars() {
        let token = actor.create_session(
            SessionOptions::new()
                .with_schema("rime_ice")
                .with_option("mo_disable_learning", true),
        )?;
        actor.dispatch(
            token,
            EngineCommand::Key(KeyEvent::new(
                XK_SHIFT_LEFT,
                KeyModifiers::SHIFT,
                KeyState::Pressed,
                None,
                false,
            )),
        )?;
        let down = actor.dispatch(
            token,
            EngineCommand::Key(KeyEvent::new(
                u32::from(character),
                KeyModifiers::SHIFT,
                KeyState::Pressed,
                Some(character),
                false,
            )),
        )?;
        if !down.handled
            || down.commit.as_ref().is_none_or(String::is_empty)
            || down.composition.is_some()
        {
            return Err(io::Error::other("shifted punctuation did not commit").into());
        }
        let up = actor.dispatch(
            token,
            EngineCommand::Key(KeyEvent::new(
                u32::from(character),
                KeyModifiers::SHIFT,
                KeyState::Released,
                None,
                false,
            )),
        )?;
        let shift_up = actor.dispatch(
            token,
            EngineCommand::Key(KeyEvent::new(
                XK_SHIFT_LEFT,
                KeyModifiers::NONE,
                KeyState::Released,
                None,
                false,
            )),
        )?;
        if up.commit.is_some() || shift_up.commit.is_some() || shift_up.status.ascii_mode {
            return Err(
                io::Error::other("shifted punctuation replayed or toggled ASCII mode").into(),
            );
        }
        actor.destroy_session(token)?;
    }
    for modifiers in [
        KeyModifiers::CONTROL,
        KeyModifiers::ALT,
        KeyModifiers::SUPER,
    ] {
        let token = actor.create_session(
            SessionOptions::new()
                .with_schema("rime_ice")
                .with_option("mo_disable_learning", true),
        )?;
        let shortcut = actor.dispatch(
            token,
            EngineCommand::Key(KeyEvent::new(
                u32::from(b'/'),
                modifiers,
                KeyState::Pressed,
                None,
                false,
            )),
        )?;
        if shortcut.handled || shortcut.commit.is_some() || shortcut.composition.is_some() {
            return Err(io::Error::other("modified slash shortcut was consumed").into());
        }
        actor.destroy_session(token)?;
    }
    println!("Real shifted punctuation and slash shortcuts passed.");
    for slow in [false, true] {
        let token = actor.create_session(
            SessionOptions::new()
                .with_schema("rime_ice")
                .with_option("mo_disable_learning", true),
        )?;
        for index in 0..REPEATED_SLASH_COUNT {
            if slow {
                thread::sleep(SLOW_KEY_INTERVAL);
            }
            let down = actor.dispatch(token, EngineCommand::Key(KeyEvent::text('/')))?;
            println!(
                "slash slow={slow} index={index} handled={} commit={} composition={}",
                down.handled,
                down.commit.is_some(),
                down.composition.is_some()
            );
            if !down.handled || down.commit.as_deref() != Some("/") || down.composition.is_some() {
                return Err(io::Error::other("slash did not commit exactly once").into());
            }
            let up = actor.dispatch(
                token,
                EngineCommand::Key(KeyEvent::new(
                    u32::from(b'/'),
                    KeyModifiers::NONE,
                    KeyState::Released,
                    None,
                    false,
                )),
            )?;
            if up.handled || up.commit.is_some() || up.composition.is_some() {
                return Err(io::Error::other("slash release changed or replayed input").into());
            }
        }
        actor.destroy_session(token)?;
    }
    if swallowed != 0 {
        return Err(io::Error::other("printable keys consumed without visible output").into());
    }
    println!("Real printable key audit passed.");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("key_smoke requires Windows");
    std::process::exit(1);
}
