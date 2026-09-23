use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mo_ipc::{
    CharacterSet as WireCharacterSet, InputScheme as WireInputScheme,
    SettingsOrigin as WireSettingsOrigin, SettingsSnapshot as WireSettingsSnapshot,
    Theme as WireTheme,
};
use mo_settings::{
    CharacterSet, InputScheme, RuntimeSnapshot, Settings, SettingsOrigin, SettingsRuntime, Theme,
};

#[derive(Clone)]
pub(crate) struct SettingsService {
    state: Arc<SettingsState>,
}

enum SettingsState {
    Fixed(WireSettingsSnapshot),
    File(Mutex<SettingsRuntime>),
}

impl SettingsService {
    pub(crate) fn defaults() -> Self {
        let settings = Settings::default();
        Self {
            state: Arc::new(SettingsState::Fixed(project(
                1,
                SettingsOrigin::Defaults,
                &settings,
            ))),
        }
    }

    pub(crate) fn open(path: PathBuf) -> io::Result<Self> {
        let runtime = SettingsRuntime::open(path).map_err(io::Error::other)?;
        Ok(Self {
            state: Arc::new(SettingsState::File(Mutex::new(runtime))),
        })
    }

    pub(crate) fn snapshot(&self) -> io::Result<WireSettingsSnapshot> {
        match self.state.as_ref() {
            SettingsState::Fixed(snapshot) => Ok(*snapshot),
            SettingsState::File(runtime) => {
                let mut runtime = runtime
                    .lock()
                    .map_err(|_| io::Error::other("settings state lock is poisoned"))?;
                let snapshot = runtime.refresh().map_err(io::Error::other)?;
                Ok(project_runtime(snapshot))
            }
        }
    }
}

fn project_runtime(snapshot: &RuntimeSnapshot) -> WireSettingsSnapshot {
    project(snapshot.revision, snapshot.origin, &snapshot.settings)
}

fn project(revision: u64, origin: SettingsOrigin, settings: &Settings) -> WireSettingsSnapshot {
    WireSettingsSnapshot {
        revision,
        origin: match origin {
            SettingsOrigin::Defaults => WireSettingsOrigin::Defaults,
            SettingsOrigin::Stored => WireSettingsOrigin::Stored,
        },
        input_scheme: match settings.input_scheme {
            InputScheme::FullPinyin => WireInputScheme::FullPinyin,
            InputScheme::DoublePinyinNatural => WireInputScheme::DoublePinyinNatural,
            InputScheme::DoublePinyinFlypy => WireInputScheme::DoublePinyinFlypy,
            InputScheme::DoublePinyinMicrosoft => WireInputScheme::DoublePinyinMicrosoft,
            InputScheme::DoublePinyinSogou => WireInputScheme::DoublePinyinSogou,
        },
        character_set: match settings.character_set {
            CharacterSet::Simplified => WireCharacterSet::Simplified,
            CharacterSet::Traditional => WireCharacterSet::Traditional,
        },
        candidate_page_size: settings.candidate_page_size,
        theme: match settings.theme {
            Theme::System => WireTheme::System,
            Theme::Light => WireTheme::Light,
            Theme::Dark => WireTheme::Dark,
        },
        show_comments: settings.show_comments,
        emoji: settings.emoji,
        local_learning: settings.local_learning,
        privacy_mode: settings.privacy_mode,
        effective_learning: settings.effective_learning(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use mo_settings::{Settings, Theme, save_atomic};

    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("mo-broker-settings-{}-{nonce}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn shared_file_service_refreshes_and_recovers_without_losing_revision() {
        let fixture = Fixture::new();
        let path = fixture.0.join("settings.mo");
        let service = SettingsService::open(path.clone()).unwrap();
        let clone = service.clone();
        let initial = service.snapshot().unwrap();
        assert_eq!(initial.origin, WireSettingsOrigin::Defaults);

        let changed = Settings {
            theme: Theme::Dark,
            ..Settings::default()
        };
        save_atomic(&path, &changed).unwrap();
        let refreshed = clone.snapshot().unwrap();
        assert_eq!(refreshed.revision, initial.revision + 1);
        assert_eq!(refreshed.theme, WireTheme::Dark);

        fs::write(&path, b"broken").unwrap();
        assert!(service.snapshot().is_err());
        save_atomic(&path, &Settings::default()).unwrap();
        let recovered = service.snapshot().unwrap();
        assert_eq!(recovered.revision, refreshed.revision + 1);
        assert_eq!(recovered.theme, WireTheme::System);
    }
}
