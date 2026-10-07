//! Non-UI controller for Mo's native Windows settings frontend.

use std::fmt;
use std::path::{Path, PathBuf};

use mo_settings::{
    CharacterSet, InputScheme, LoadedSettings, Settings, StoreError, Theme,
    ensure_installed_settings_directory, installed_settings_path, load, save_atomic,
    save_atomic_if_unchanged,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentHealth {
    Ready,
    RecoveryRequired(String),
}

pub struct PrimaryPreferences {
    pub input_scheme: InputScheme,
    pub character_set: CharacterSet,
    pub theme: Theme,
    pub show_comments: bool,
    pub emoji: bool,
    pub local_learning: bool,
    pub privacy_mode: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsController {
    local_app_data_root: PathBuf,
    path: PathBuf,
    settings: Settings,
    stored: bool,
    health: DocumentHealth,
}

impl SettingsController {
    #[must_use]
    pub fn open(local_app_data_root: impl Into<PathBuf>) -> Self {
        let local_app_data_root = local_app_data_root.into();
        let path = installed_settings_path(&local_app_data_root);
        match load(&path) {
            Ok(LoadedSettings::Defaults(settings)) => Self {
                local_app_data_root,
                path,
                settings,
                stored: false,
                health: DocumentHealth::Ready,
            },
            Ok(LoadedSettings::Stored(settings)) => Self {
                local_app_data_root,
                path,
                settings,
                stored: true,
                health: DocumentHealth::Ready,
            },
            Err(error) => Self {
                local_app_data_root,
                path,
                settings: Settings::default(),
                stored: true,
                health: DocumentHealth::RecoveryRequired(error.to_string()),
            },
        }
    }

    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn stored(&self) -> bool {
        self.stored
    }

    #[must_use]
    pub fn health(&self) -> &DocumentHealth {
        &self.health
    }

    #[must_use]
    pub fn can_save_changes(&self) -> bool {
        matches!(self.health, DocumentHealth::Ready)
    }

    /// Saves the presentation theme without changing engine preferences.
    pub fn save_theme(&mut self, theme: Theme) -> Result<(), ControllerError> {
        if !self.can_save_changes() {
            return Err(ControllerError::RecoveryRequired);
        }
        let mut changed = self.settings.clone();
        changed.theme = theme;
        self.persist_checked(changed)
    }

    /// Saves implemented engine and presentation preferences as one atomic document.
    /// The planned candidate page size is preserved unchanged.
    pub fn save_primary_preferences(
        &mut self,
        preferences: PrimaryPreferences,
    ) -> Result<(), ControllerError> {
        if !self.can_save_changes() {
            return Err(ControllerError::RecoveryRequired);
        }
        let mut changed = self.settings.clone();
        changed.input_scheme = preferences.input_scheme;
        changed.character_set = preferences.character_set;
        changed.theme = preferences.theme;
        changed.show_comments = preferences.show_comments;
        changed.emoji = preferences.emoji;
        changed.local_learning = preferences.local_learning;
        changed.privacy_mode = preferences.privacy_mode;
        self.persist_checked(changed)
    }

    /// Explicit user recovery. This is the only path that overwrites a corrupt
    /// or future settings document with product defaults.
    pub fn restore_defaults(&mut self) -> Result<(), ControllerError> {
        self.persist(Settings::default(), None)
    }

    pub fn reload(&mut self) {
        *self = Self::open(self.local_app_data_root.clone());
    }

    fn persist_checked(&mut self, settings: Settings) -> Result<(), ControllerError> {
        let expected = if self.stored {
            LoadedSettings::Stored(self.settings.clone())
        } else {
            LoadedSettings::Defaults(self.settings.clone())
        };
        self.persist(settings, Some(expected))
    }

    fn persist(
        &mut self,
        settings: Settings,
        expected: Option<LoadedSettings>,
    ) -> Result<(), ControllerError> {
        let directory = ensure_installed_settings_directory(&self.local_app_data_root)?;
        if self.path.parent() != Some(directory.as_path()) {
            return Err(ControllerError::PathMismatch);
        }
        if let Some(expected) = expected {
            save_atomic_if_unchanged(&self.path, &settings, &expected)?;
        } else {
            save_atomic(&self.path, &settings)?;
        }
        self.settings = settings;
        self.stored = true;
        self.health = DocumentHealth::Ready;
        Ok(())
    }
}

#[derive(Debug)]
pub enum ControllerError {
    Store(StoreError),
    RecoveryRequired,
    PathMismatch,
}

impl fmt::Display for ControllerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(StoreError::Changed) => {
                formatter.write_str("设置已被修改，请点击“重新读取”后再保存")
            }
            Self::Store(StoreError::WriteBusy) => formatter.write_str("设置正在保存，请稍后重试"),
            Self::Store(error) => write!(formatter, "无法保存设置：{error}"),
            Self::RecoveryRequired => {
                formatter.write_str("设置文件已损坏或版本过新，请明确选择恢复默认设置")
            }
            Self::PathMismatch => formatter.write_str("设置目录与固定产品路径不一致"),
        }
    }
}

impl std::error::Error for ControllerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::RecoveryRequired | Self::PathMismatch => None,
        }
    }
}

impl From<StoreError> for ControllerError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use mo_settings::{InputScheme, LoadedSettings, load};

    use super::*;

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            Self::with_nonce(nonce)
        }

        fn with_nonce(nonce: u128) -> Self {
            // Wall-clock precision does not guarantee distinct parallel fixtures.
            // Only successful create_dir grants this fixture cleanup ownership.
            for _ in 0..64 {
                let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "mo-settings-app-{}-{nonce}-{sequence}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("cannot create settings test fixture: {error}"),
                }
            }
            panic!("settings test fixture namespace exhausted")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn parallel_fixtures_with_the_same_timestamp_remain_independent() {
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let children = (0..8)
            .map(|_| {
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    Fixture::with_nonce(0)
                })
            })
            .collect::<Vec<_>>();
        let fixtures = children
            .into_iter()
            .map(|child| child.join().unwrap())
            .collect::<Vec<_>>();
        let paths = fixtures
            .iter()
            .map(|fixture| fixture.0.clone())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(paths.len(), 8);
        for (index, fixture) in fixtures.iter().enumerate() {
            fs::write(fixture.0.join("synthetic"), index.to_string()).unwrap();
        }
        for (index, fixture) in fixtures.iter().enumerate() {
            assert_eq!(
                fs::read_to_string(fixture.0.join("synthetic")).unwrap(),
                index.to_string()
            );
        }
        drop(fixtures);
        assert!(paths.iter().all(|path| !path.exists()));
    }

    #[test]
    fn first_run_uses_defaults_without_creating_profile() {
        let fixture = Fixture::new();
        let controller = SettingsController::open(&fixture.0);
        assert_eq!(controller.health(), &DocumentHealth::Ready);
        assert!(!controller.stored());
        assert!(!controller.path().exists());
        assert_eq!(controller.settings(), &Settings::default());
    }

    #[test]
    fn saving_theme_preserves_unavailable_engine_preferences() {
        let fixture = Fixture::new();
        let path = installed_settings_path(&fixture.0);
        ensure_installed_settings_directory(&fixture.0).unwrap();
        let original = Settings {
            input_scheme: InputScheme::DoublePinyinSogou,
            candidate_page_size: 8,
            emoji: false,
            theme: Theme::Light,
            ..Settings::default()
        };
        save_atomic(&path, &original).unwrap();

        let mut controller = SettingsController::open(&fixture.0);
        controller.save_theme(Theme::Dark).unwrap();
        let LoadedSettings::Stored(saved) = load(&path).unwrap() else {
            panic!("saved settings must be stored")
        };
        assert_eq!(saved.input_scheme, InputScheme::DoublePinyinSogou);
        assert_eq!(saved.candidate_page_size, 8);
        assert!(!saved.emoji);
        assert_eq!(saved.theme, Theme::Dark);
    }

    #[test]
    fn saving_primary_preferences_is_atomic_and_preserves_unimplemented_fields() {
        let fixture = Fixture::new();
        let path = installed_settings_path(&fixture.0);
        ensure_installed_settings_directory(&fixture.0).unwrap();
        let original = Settings {
            candidate_page_size: 8,
            emoji: false,
            local_learning: false,
            ..Settings::default()
        };
        save_atomic(&path, &original).unwrap();
        let mut controller = SettingsController::open(&fixture.0);
        controller
            .save_primary_preferences(PrimaryPreferences {
                input_scheme: InputScheme::DoublePinyinFlypy,
                character_set: CharacterSet::Traditional,
                theme: Theme::Dark,
                show_comments: false,
                emoji: true,
                local_learning: true,
                privacy_mode: true,
            })
            .unwrap();
        let LoadedSettings::Stored(saved) = load(&path).unwrap() else {
            panic!("stored")
        };
        assert_eq!(saved.input_scheme, InputScheme::DoublePinyinFlypy);
        assert_eq!(saved.character_set, CharacterSet::Traditional);
        assert_eq!(saved.theme, Theme::Dark);
        assert!(!saved.show_comments);
        assert_eq!(saved.candidate_page_size, 8);
        assert!(saved.emoji);
        assert!(saved.local_learning);
        assert!(saved.privacy_mode);
        assert!(!saved.effective_learning());
    }

    #[test]
    fn corrupt_document_requires_explicit_recovery() {
        let fixture = Fixture::new();
        ensure_installed_settings_directory(&fixture.0).unwrap();
        let path = installed_settings_path(&fixture.0);
        fs::write(&path, b"broken").unwrap();

        let mut controller = SettingsController::open(&fixture.0);
        assert!(matches!(
            controller.health(),
            DocumentHealth::RecoveryRequired(_)
        ));
        assert!(matches!(
            controller.save_theme(Theme::Dark),
            Err(ControllerError::RecoveryRequired)
        ));
        assert_eq!(fs::read(&path).unwrap(), b"broken");

        controller.restore_defaults().unwrap();
        assert_eq!(controller.health(), &DocumentHealth::Ready);
        assert_eq!(
            load(&path).unwrap(),
            LoadedSettings::Stored(Settings::default())
        );
    }

    #[test]
    fn reload_observes_external_atomic_changes() {
        let fixture = Fixture::new();
        ensure_installed_settings_directory(&fixture.0).unwrap();
        let path = installed_settings_path(&fixture.0);
        let mut controller = SettingsController::open(&fixture.0);
        let changed = Settings {
            theme: Theme::Light,
            ..Settings::default()
        };
        save_atomic(&path, &changed).unwrap();
        controller.reload();
        assert_eq!(controller.settings().theme, Theme::Light);
        assert!(controller.stored());
    }

    #[test]
    fn stale_window_preserves_other_window_preferences_until_reload() {
        let fixture = Fixture::new();
        let mut first = SettingsController::open(&fixture.0);
        let mut stale = SettingsController::open(&fixture.0);
        first.save_theme(Theme::Dark).unwrap();
        let saved = fs::read(first.path()).unwrap();
        assert!(matches!(
            stale.save_theme(Theme::Light),
            Err(ControllerError::Store(StoreError::Changed))
        ));
        assert_eq!(fs::read(first.path()).unwrap(), saved);
        assert!(!stale.stored());
        assert_eq!(stale.settings().theme, Theme::System);
        stale.reload();
        stale.save_theme(Theme::Light).unwrap();
        first.reload();
        assert_eq!(first.settings().theme, Theme::Light);
    }

    #[test]
    fn external_future_document_requires_reload_and_explicit_recovery() {
        let fixture = Fixture::new();
        let mut controller = SettingsController::open(&fixture.0);
        controller.save_theme(Theme::Dark).unwrap();
        let future = b"mo-settings\nformat=2\n";
        fs::write(controller.path(), future).unwrap();
        let error = controller.save_theme(Theme::Light).unwrap_err();
        assert!(matches!(error, ControllerError::Store(StoreError::Changed)));
        assert!(error.to_string().contains("重新读取"));
        assert_eq!(fs::read(controller.path()).unwrap(), future);
        assert_eq!(controller.settings().theme, Theme::Dark);
        controller.reload();
        assert!(!controller.can_save_changes());
        controller.restore_defaults().unwrap();
        assert_eq!(controller.settings(), &Settings::default());
    }

    #[test]
    fn stale_primary_save_cannot_disable_other_window_privacy_preference() {
        let fixture = Fixture::new();
        let mut first = SettingsController::open(&fixture.0);
        let mut stale = SettingsController::open(&fixture.0);
        let preferences = |privacy_mode, theme| PrimaryPreferences {
            input_scheme: InputScheme::FullPinyin,
            character_set: CharacterSet::Simplified,
            theme,
            show_comments: true,
            emoji: true,
            local_learning: true,
            privacy_mode,
        };
        first
            .save_primary_preferences(preferences(true, Theme::Dark))
            .unwrap();
        let before = fs::read(first.path()).unwrap();
        assert!(matches!(
            stale.save_primary_preferences(preferences(false, Theme::Light)),
            Err(ControllerError::Store(StoreError::Changed))
        ));
        assert_eq!(fs::read(first.path()).unwrap(), before);
        let stored = load(first.path()).unwrap();
        assert!(stored.settings().privacy_mode);
        assert!(!stored.settings().effective_learning());
    }
}
