//! Non-UI controller for Mo's native Windows settings frontend.

use std::fmt;
use std::path::{Path, PathBuf};

use mo_settings::{
    LoadedSettings, Settings, StoreError, Theme, ensure_installed_settings_directory,
    installed_settings_path, load, save_atomic,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentHealth {
    Ready,
    RecoveryRequired(String),
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

    /// Saves the only setting currently applied by the native frontend.
    /// Other preferences are preserved byte-for-byte at the typed model level.
    pub fn save_theme(&mut self, theme: Theme) -> Result<(), ControllerError> {
        if !self.can_save_changes() {
            return Err(ControllerError::RecoveryRequired);
        }
        let mut changed = self.settings.clone();
        changed.theme = theme;
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
