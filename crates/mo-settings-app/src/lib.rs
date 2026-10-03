//! Non-UI controller for Mo's native Windows settings frontend.

use std::fmt;
use std::path::{Path, PathBuf};

use mo_settings::{
    CharacterSet, InputScheme, LoadedSettings, Settings, StoreError, Theme,
    ensure_installed_settings_directory, installed_settings_path, load, save_atomic,
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
        self.persist(changed)
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
        self.persist(changed)
    }

    /// Explicit user recovery. This is the only path that overwrites a corrupt
    /// or future settings document with product defaults.
    pub fn restore_defaults(&mut self) -> Result<(), ControllerError> {
        self.persist(Settings::default())
    }

    pub fn reload(&mut self) {
        *self = Self::open(self.local_app_data_root.clone());
    }

    fn persist(&mut self, settings: Settings) -> Result<(), ControllerError> {
        let directory = ensure_installed_settings_directory(&self.local_app_data_root)?;
        if self.path.parent() != Some(directory.as_path()) {
            return Err(ControllerError::PathMismatch);
        }
        save_atomic(&self.path, &settings)?;
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
    use std::time::{SystemTime, UNIX_EPOCH};

    use mo_settings::{InputScheme, LoadedSettings, load};

    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("mo-settings-app-{}-{nonce}", std::process::id()));
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
}
