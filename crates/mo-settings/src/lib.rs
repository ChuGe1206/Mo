//! Typed, versioned settings and fail-closed persistence for Mo.
//!
//! The ordinary settings UI owns this format. Users never need to edit Rime
//! YAML or Lua, and this crate deliberately does not generate executable data.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Maximum accepted settings file size, including the header.
pub const MAX_SETTINGS_BYTES: usize = 16 * 1024;

/// Returns the fixed installed settings file beneath an OS-resolved
/// `FOLDERID_LocalAppData` root. The caller must supply that trusted root rather
/// than an environment-variable expansion.
pub fn installed_settings_path(local_app_data_root: &Path) -> PathBuf {
    local_app_data_root
        .join("Mo")
        .join("Profile")
        .join("settings-v1.mo")
}

/// Creates only Mo's direct settings directories beneath a trusted
/// `FOLDERID_LocalAppData` root. Existing files, links and reparse points are
/// rejected instead of followed.
pub fn ensure_installed_settings_directory(
    local_app_data_root: &Path,
) -> Result<PathBuf, StoreError> {
    checked_directory(local_app_data_root)?;
    let mo = local_app_data_root.join("Mo");
    let profile = mo.join("Profile");
    for directory in [&mo, &profile] {
        match fs::create_dir(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        checked_directory(directory)?;
    }
    Ok(profile)
}

fn checked_directory(path: &Path) -> Result<(), StoreError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir() || is_reparse_point(&metadata) {
        return Err(StoreError::UnsafeDirectory);
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

const HEADER: &str = "mo-settings";
const FORMAT: &str = "1";
const REQUIRED_FIELDS: [&str; 9] = [
    "format",
    "input_scheme",
    "character_set",
    "candidate_page_size",
    "show_comments",
    "emoji",
    "local_learning",
    "privacy_mode",
    "theme",
];

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// User-facing input scheme. Only values represented here may enter persisted
/// settings; runtime activation remains a separate, explicitly tested step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputScheme {
    FullPinyin,
    DoublePinyinNatural,
    DoublePinyinFlypy,
    DoublePinyinMicrosoft,
    DoublePinyinSogou,
}

impl InputScheme {
    fn as_str(self) -> &'static str {
        match self {
            Self::FullPinyin => "full_pinyin",
            Self::DoublePinyinNatural => "double_pinyin_natural",
            Self::DoublePinyinFlypy => "double_pinyin_flypy",
            Self::DoublePinyinMicrosoft => "double_pinyin_microsoft",
            Self::DoublePinyinSogou => "double_pinyin_sogou",
        }
    }

    fn parse(value: &str) -> Result<Self, SettingsError> {
        match value {
            "full_pinyin" => Ok(Self::FullPinyin),
            "double_pinyin_natural" => Ok(Self::DoublePinyinNatural),
            "double_pinyin_flypy" => Ok(Self::DoublePinyinFlypy),
            "double_pinyin_microsoft" => Ok(Self::DoublePinyinMicrosoft),
            "double_pinyin_sogou" => Ok(Self::DoublePinyinSogou),
            _ => Err(SettingsError::InvalidValue("input_scheme")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharacterSet {
    Simplified,
    Traditional,
}

impl CharacterSet {
    fn as_str(self) -> &'static str {
        match self {
            Self::Simplified => "simplified",
            Self::Traditional => "traditional",
        }
    }

    fn parse(value: &str) -> Result<Self, SettingsError> {
        match value {
            "simplified" => Ok(Self::Simplified),
            "traditional" => Ok(Self::Traditional),
            _ => Err(SettingsError::InvalidValue("character_set")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Theme {
    System,
    Light,
    Dark,
}

/// Settings that a native frontend may apply without changing engine state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationPlan {
    pub show_comments: bool,
    pub theme: Theme,
}

/// Desired engine behavior. This is deliberately data, not a set of librime
/// option names: translating it into native operations requires a separately
/// validated adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnginePreferences {
    pub input_scheme: InputScheme,
    pub character_set: CharacterSet,
    pub candidate_page_size: u8,
    pub emoji: bool,
    pub local_learning: bool,
    pub privacy_mode: bool,
    pub effective_learning: bool,
}

/// Validated settings split by the component that will eventually apply them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePlan {
    pub presentation: PresentationPlan,
    pub engine: EnginePreferences,
}

impl RuntimePlan {
    #[must_use]
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            presentation: PresentationPlan {
                show_comments: settings.show_comments,
                theme: settings.theme,
            },
            engine: EnginePreferences {
                input_scheme: settings.input_scheme,
                character_set: settings.character_set,
                candidate_page_size: settings.candidate_page_size,
                emoji: settings.emoji,
                local_learning: settings.local_learning,
                privacy_mode: settings.privacy_mode,
                effective_learning: settings.effective_learning(),
            },
        }
    }
}

impl Theme {
    fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    fn parse(value: &str) -> Result<Self, SettingsError> {
        match value {
            "system" => Ok(Self::System),
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err(SettingsError::InvalidValue("theme")),
        }
    }
}

/// Version 1 of Mo's ordinary-user settings surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settings {
    pub input_scheme: InputScheme,
    pub character_set: CharacterSet,
    pub candidate_page_size: u8,
    pub show_comments: bool,
    pub emoji: bool,
    pub local_learning: bool,
    pub privacy_mode: bool,
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            input_scheme: InputScheme::FullPinyin,
            character_set: CharacterSet::Simplified,
            candidate_page_size: 5,
            show_comments: true,
            emoji: true,
            local_learning: true,
            privacy_mode: false,
            theme: Theme::System,
        }
    }
}

impl Settings {
    /// Privacy mode suppresses learning without destroying the user's persisted
    /// learning preference.
    pub fn effective_learning(&self) -> bool {
        self.local_learning && !self.privacy_mode
    }

    pub fn validate(&self) -> Result<(), SettingsError> {
        if !(3..=9).contains(&self.candidate_page_size) {
            return Err(SettingsError::InvalidValue("candidate_page_size"));
        }
        Ok(())
    }

    /// Encodes a canonical, deterministic representation.
    pub fn encode(&self) -> Result<String, SettingsError> {
        self.validate()?;
        Ok(format!(
            "{HEADER}\nformat={FORMAT}\ninput_scheme={}\ncharacter_set={}\n\
             candidate_page_size={}\nshow_comments={}\nemoji={}\nlocal_learning={}\n\
             privacy_mode={}\ntheme={}\n",
            self.input_scheme.as_str(),
            self.character_set.as_str(),
            self.candidate_page_size,
            self.show_comments,
            self.emoji,
            self.local_learning,
            self.privacy_mode,
            self.theme.as_str(),
        ))
    }

    /// Decodes one complete settings document. Unknown, missing or duplicate
    /// fields fail closed so newer settings are never silently downgraded.
    pub fn decode(bytes: &[u8]) -> Result<Self, SettingsError> {
        if bytes.len() > MAX_SETTINGS_BYTES {
            return Err(SettingsError::TooLarge);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| SettingsError::InvalidUtf8)?;
        let mut lines = text.split('\n');
        let header = clean_line(lines.next().unwrap_or_default())?;
        if header != HEADER {
            return Err(SettingsError::InvalidHeader);
        }

        let mut fields = BTreeMap::new();
        for raw_line in lines {
            let line = clean_line(raw_line)?;
            if line.is_empty() {
                continue;
            }
            let (name, value) = line.split_once('=').ok_or(SettingsError::MalformedLine)?;
            if name.is_empty()
                || value.is_empty()
                || name
                    .bytes()
                    .any(|byte| !byte.is_ascii_lowercase() && byte != b'_')
            {
                return Err(SettingsError::MalformedLine);
            }
            if fields.insert(name, value).is_some() {
                return Err(SettingsError::DuplicateField);
            }
        }
        let Some(format) = fields.get("format") else {
            return Err(SettingsError::MissingField("format"));
        };
        if *format != FORMAT {
            return Err(SettingsError::UnsupportedVersion);
        }
        if fields.keys().any(|name| !REQUIRED_FIELDS.contains(name)) {
            return Err(SettingsError::UnknownField);
        }
        for required in REQUIRED_FIELDS {
            if !fields.contains_key(required) {
                return Err(SettingsError::MissingField(required));
            }
        }

        let settings = Self {
            input_scheme: InputScheme::parse(fields["input_scheme"])?,
            character_set: CharacterSet::parse(fields["character_set"])?,
            candidate_page_size: fields["candidate_page_size"]
                .parse()
                .map_err(|_| SettingsError::InvalidValue("candidate_page_size"))?,
            show_comments: parse_bool("show_comments", fields["show_comments"])?,
            emoji: parse_bool("emoji", fields["emoji"])?,
            local_learning: parse_bool("local_learning", fields["local_learning"])?,
            privacy_mode: parse_bool("privacy_mode", fields["privacy_mode"])?,
            theme: Theme::parse(fields["theme"])?,
        };
        settings.validate()?;
        Ok(settings)
    }
}

fn clean_line(line: &str) -> Result<&str, SettingsError> {
    let line = line.strip_suffix('\r').unwrap_or(line);
    if line.contains('\r') {
        return Err(SettingsError::MalformedLine);
    }
    Ok(line)
}

fn parse_bool(field: &'static str, value: &str) -> Result<bool, SettingsError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(SettingsError::InvalidValue(field)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsError {
    TooLarge,
    InvalidUtf8,
    InvalidHeader,
    UnsupportedVersion,
    MalformedLine,
    UnknownField,
    DuplicateField,
    MissingField(&'static str),
    InvalidValue(&'static str),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("settings file exceeds the size limit"),
            Self::InvalidUtf8 => formatter.write_str("settings file is not valid UTF-8"),
            Self::InvalidHeader => formatter.write_str("settings file header is invalid"),
            Self::UnsupportedVersion => {
                formatter.write_str("settings format version is unsupported")
            }
            Self::MalformedLine => formatter.write_str("settings file contains a malformed line"),
            Self::UnknownField => formatter.write_str("settings file contains an unknown field"),
            Self::DuplicateField => formatter.write_str("settings file contains a duplicate field"),
            Self::MissingField(field) => write!(formatter, "settings file is missing {field}"),
            Self::InvalidValue(field) => write!(formatter, "settings value for {field} is invalid"),
        }
    }
}

impl std::error::Error for SettingsError {}

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    InvalidDocument(SettingsError),
    UnsafeFileType,
    UnsafeDirectory,
    MissingParent,
    Changed,
    WriteBusy,
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "settings I/O failed: {error}"),
            Self::InvalidDocument(error) => {
                write!(formatter, "settings document is invalid: {error}")
            }
            Self::UnsafeFileType => formatter.write_str("settings path is not a regular file"),
            Self::UnsafeDirectory => {
                formatter.write_str("settings directory is not a safe directory")
            }
            Self::MissingParent => formatter.write_str("settings parent directory does not exist"),
            Self::Changed => formatter.write_str("settings changed since they were loaded"),
            Self::WriteBusy => formatter.write_str("another settings writer holds the write guard"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidDocument(error) => Some(error),
            Self::UnsafeFileType
            | Self::UnsafeDirectory
            | Self::MissingParent
            | Self::Changed
            | Self::WriteBusy => None,
        }
    }
}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<SettingsError> for StoreError {
    fn from(error: SettingsError) -> Self {
        Self::InvalidDocument(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoadedSettings {
    Defaults(Settings),
    Stored(Settings),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsOrigin {
    Defaults,
    Stored,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSnapshot {
    pub revision: u64,
    pub origin: SettingsOrigin,
    pub settings: Settings,
    pub plan: RuntimePlan,
}

/// Owns the last successfully decoded settings and advances its revision only
/// when the semantic document or its default/stored origin changes. A failed
/// refresh never discards the last known-good snapshot.
#[derive(Debug)]
pub struct SettingsRuntime {
    path: PathBuf,
    snapshot: RuntimeSnapshot,
}

impl SettingsRuntime {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, RuntimeError> {
        let path = path.into();
        let (origin, settings) = loaded_parts(load(&path)?);
        let plan = RuntimePlan::from_settings(&settings);
        Ok(Self {
            path,
            snapshot: RuntimeSnapshot {
                revision: 1,
                origin,
                settings,
                plan,
            },
        })
    }

    #[must_use]
    pub fn snapshot(&self) -> &RuntimeSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn refresh(&mut self) -> Result<&RuntimeSnapshot, RuntimeError> {
        let (origin, settings) = loaded_parts(load(&self.path)?);
        if origin != self.snapshot.origin || settings != self.snapshot.settings {
            let revision = self
                .snapshot
                .revision
                .checked_add(1)
                .ok_or(RuntimeError::RevisionExhausted)?;
            self.snapshot = RuntimeSnapshot {
                revision,
                plan: RuntimePlan::from_settings(&settings),
                origin,
                settings,
            };
        }
        Ok(&self.snapshot)
    }
}

fn loaded_parts(loaded: LoadedSettings) -> (SettingsOrigin, Settings) {
    match loaded {
        LoadedSettings::Defaults(settings) => (SettingsOrigin::Defaults, settings),
        LoadedSettings::Stored(settings) => (SettingsOrigin::Stored, settings),
    }
}

#[derive(Debug)]
pub enum RuntimeError {
    Store(StoreError),
    RevisionExhausted,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(formatter, "could not refresh settings: {error}"),
            Self::RevisionExhausted => formatter.write_str("settings revision space is exhausted"),
        }
    }
}

impl std::error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::RevisionExhausted => None,
        }
    }
}

impl From<StoreError> for RuntimeError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl LoadedSettings {
    pub fn settings(&self) -> &Settings {
        match self {
            Self::Defaults(settings) | Self::Stored(settings) => settings,
        }
    }
}

/// Loads settings without silently replacing malformed data. Only a genuinely
/// absent file selects defaults.
pub fn load(path: &Path) -> Result<LoadedSettings, StoreError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(LoadedSettings::Defaults(Settings::default()));
        }
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_file() {
        return Err(StoreError::UnsafeFileType);
    }
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take((MAX_SETTINGS_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_SETTINGS_BYTES {
        return Err(StoreError::InvalidDocument(SettingsError::TooLarge));
    }
    Ok(LoadedSettings::Stored(Settings::decode(&bytes)?))
}

/// Writes and flushes a new file beside the destination, then atomically
/// replaces the destination. The caller owns directory creation and ACL policy.
pub fn save_atomic(path: &Path, settings: &Settings) -> Result<(), StoreError> {
    save_atomic_checked(path, settings, None)
}

/// Saves only if the semantic document and its absent/stored origin match.
/// All Mo writers share one guard. Explicit recovery uses save_atomic.
pub fn save_atomic_if_unchanged(
    path: &Path,
    settings: &Settings,
    expected: &LoadedSettings,
) -> Result<(), StoreError> {
    save_atomic_checked(path, settings, Some(expected))
}

fn save_atomic_checked(
    path: &Path,
    settings: &Settings,
    expected: Option<&LoadedSettings>,
) -> Result<(), StoreError> {
    let encoded = settings.encode()?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let Some(parent) = parent else {
        return Err(StoreError::MissingParent);
    };
    if !fs::metadata(parent)
        .map(|item| item.is_dir())
        .unwrap_or(false)
    {
        return Err(StoreError::MissingParent);
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_file() => return Err(StoreError::UnsafeFileType),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let _writer = WriteGuard::acquire(path)?;
    if let Some(expected) = expected {
        match load(path) {
            Ok(current) if &current == expected => {}
            Ok(_) | Err(StoreError::InvalidDocument(_)) => return Err(StoreError::Changed),
            Err(error) => return Err(error),
        }
    }
    let (temporary, mut file) = create_temporary_file(path)?;
    let flushed = file
        .write_all(encoded.as_bytes())
        .and_then(|()| file.sync_all());
    drop(file);
    let result = flushed
        .and_then(|()| replace_file(&temporary, path))
        .map_err(StoreError::Io);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

// Exclusively create this reserved sidecar; never adopt an existing file.
// On Windows the owning handle deletes it on close, including process exit.
struct WriteGuard {
    _file: fs::File,
    #[cfg(not(windows))]
    path: PathBuf,
}

impl WriteGuard {
    fn acquire(path: &Path) -> Result<Self, StoreError> {
        let mut name = path
            .file_name()
            .ok_or(StoreError::MissingParent)?
            .to_os_string();
        name.push(".write-lock");
        let lock_path = path.with_file_name(name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_DELETE_ON_CLOSE;
            options
                .share_mode(0)
                .custom_flags(FILE_FLAG_DELETE_ON_CLOSE);
        }
        let file = match options.open(&lock_path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(StoreError::WriteBusy);
            }
            #[cfg(windows)]
            Err(error) if matches!(error.raw_os_error(), Some(32 | 303)) => {
                return Err(StoreError::WriteBusy);
            }
            Err(error) => return Err(error.into()),
        };
        Ok(Self {
            _file: file,
            #[cfg(not(windows))]
            path: lock_path,
        })
    }
}

#[cfg(not(windows))]
impl Drop for WriteGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn create_temporary_file(path: &Path) -> Result<(PathBuf, fs::File), StoreError> {
    let parent = path.parent().ok_or(StoreError::MissingParent)?;
    let file_name = path.file_name().ok_or(StoreError::MissingParent)?;
    for _ in 0..64 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut name = file_name.to_os_string();
        name.push(format!(".tmp-{}-{sequence}", std::process::id()));
        let candidate = parent.join(name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(StoreError::Io(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a settings temporary file",
    )))
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: both buffers are live, NUL-terminated UTF-16 paths. The flags
    // request same-volume replacement after the temporary file was flushed.
    let moved = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical() -> String {
        Settings::default().encode().unwrap()
    }

    fn replace_line(document: &str, prefix: &str, replacement: &str) -> String {
        document
            .lines()
            .map(|line| {
                if line.starts_with(prefix) {
                    replacement
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "mo-settings-test-{}-{sequence}",
                std::process::id()
            ));
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
    fn defaults_are_product_defaults_and_round_trip() {
        let settings = Settings::default();
        assert_eq!(settings.input_scheme, InputScheme::FullPinyin);
        assert_eq!(settings.character_set, CharacterSet::Simplified);
        assert_eq!(settings.candidate_page_size, 5);
        assert!(settings.emoji);
        assert!(settings.local_learning);
        assert!(settings.effective_learning());
        assert_eq!(
            Settings::decode(settings.encode().unwrap().as_bytes()).unwrap(),
            settings
        );
    }

    #[test]
    fn installed_path_is_separate_from_rime_state_and_versioned() {
        let root = Path::new("C:/Users/test/AppData/Local");
        assert_eq!(
            installed_settings_path(root),
            root.join("Mo/Profile/settings-v1.mo")
        );
        assert!(
            !installed_settings_path(root)
                .to_string_lossy()
                .contains("Rime")
        );
    }

    #[test]
    fn installed_settings_directory_is_created_one_component_at_a_time() {
        let fixture = Fixture::new();
        let profile = ensure_installed_settings_directory(&fixture.0).unwrap();
        assert_eq!(profile, fixture.0.join("Mo/Profile"));
        assert!(profile.is_dir());
        assert_eq!(
            ensure_installed_settings_directory(&fixture.0).unwrap(),
            profile
        );
    }

    #[test]
    fn installed_settings_directory_rejects_file_components() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("Mo"), b"not a directory").unwrap();
        assert!(matches!(
            ensure_installed_settings_directory(&fixture.0),
            Err(StoreError::UnsafeDirectory)
        ));
    }

    #[test]
    fn every_declared_enum_value_round_trips() {
        for scheme in [
            InputScheme::FullPinyin,
            InputScheme::DoublePinyinNatural,
            InputScheme::DoublePinyinFlypy,
            InputScheme::DoublePinyinMicrosoft,
            InputScheme::DoublePinyinSogou,
        ] {
            let settings = Settings {
                input_scheme: scheme,
                ..Settings::default()
            };
            assert_eq!(
                Settings::decode(settings.encode().unwrap().as_bytes()).unwrap(),
                settings
            );
        }
        for character_set in [CharacterSet::Simplified, CharacterSet::Traditional] {
            let settings = Settings {
                character_set,
                ..Settings::default()
            };
            assert_eq!(
                Settings::decode(settings.encode().unwrap().as_bytes()).unwrap(),
                settings
            );
        }
        for theme in [Theme::System, Theme::Light, Theme::Dark] {
            let settings = Settings {
                theme,
                ..Settings::default()
            };
            assert_eq!(
                Settings::decode(settings.encode().unwrap().as_bytes()).unwrap(),
                settings
            );
        }
    }

    #[test]
    fn field_order_and_crlf_are_accepted_but_encoding_is_canonical() {
        let reordered = "mo-settings\r\ntheme=dark\r\nprivacy_mode=false\r\nlocal_learning=true\r\nemoji=true\r\nshow_comments=true\r\ncandidate_page_size=7\r\ncharacter_set=traditional\r\ninput_scheme=double_pinyin_flypy\r\nformat=1\r\n";
        let parsed = Settings::decode(reordered.as_bytes()).unwrap();
        assert_eq!(parsed.theme, Theme::Dark);
        assert_eq!(parsed.input_scheme, InputScheme::DoublePinyinFlypy);
        assert_eq!(parsed.candidate_page_size, 7);
        assert!(!parsed.encode().unwrap().contains('\r'));
    }

    #[test]
    fn privacy_mode_suppresses_but_does_not_erase_learning_preference() {
        let settings = Settings {
            privacy_mode: true,
            ..Settings::default()
        };
        assert!(settings.local_learning);
        assert!(!settings.effective_learning());
    }

    #[test]
    fn runtime_plan_separates_presentation_from_unapplied_engine_preferences() {
        let settings = Settings {
            input_scheme: InputScheme::DoublePinyinNatural,
            character_set: CharacterSet::Traditional,
            candidate_page_size: 7,
            show_comments: false,
            emoji: false,
            local_learning: true,
            privacy_mode: true,
            theme: Theme::Dark,
        };
        let plan = RuntimePlan::from_settings(&settings);
        assert_eq!(
            plan.presentation,
            PresentationPlan {
                show_comments: false,
                theme: Theme::Dark,
            }
        );
        assert_eq!(plan.engine.input_scheme, InputScheme::DoublePinyinNatural);
        assert_eq!(plan.engine.character_set, CharacterSet::Traditional);
        assert_eq!(plan.engine.candidate_page_size, 7);
        assert!(!plan.engine.emoji);
        assert!(plan.engine.local_learning);
        assert!(plan.engine.privacy_mode);
        assert!(!plan.engine.effective_learning);
    }

    #[test]
    fn candidate_page_size_is_bounded() {
        for invalid in [0, 2, 10, u8::MAX] {
            let settings = Settings {
                candidate_page_size: invalid,
                ..Settings::default()
            };
            assert_eq!(
                settings.validate(),
                Err(SettingsError::InvalidValue("candidate_page_size"))
            );
        }
        for valid in 3..=9 {
            let settings = Settings {
                candidate_page_size: valid,
                ..Settings::default()
            };
            settings.validate().unwrap();
        }
    }

    #[test]
    fn malformed_documents_fail_closed() {
        let document = canonical();
        let cases = [
            ("wrong\n", SettingsError::InvalidHeader),
            (
                &document.replace("format=1", "format=2\nfuture_field=true"),
                SettingsError::UnsupportedVersion,
            ),
            (
                &document.replace("emoji=true", "emoji=yes"),
                SettingsError::InvalidValue("emoji"),
            ),
            (
                &document.replace("theme=system", "theme=blue"),
                SettingsError::InvalidValue("theme"),
            ),
            (
                &document.replace("candidate_page_size=5", "candidate_page_size=20"),
                SettingsError::InvalidValue("candidate_page_size"),
            ),
        ];
        for (bytes, expected) in cases {
            assert_eq!(Settings::decode(bytes.as_bytes()), Err(expected));
        }
    }

    #[test]
    fn unknown_duplicate_missing_and_malformed_fields_are_rejected() {
        let document = canonical();
        let unknown = document.replace("theme=system", "theme=system\nsecret=true");
        assert_eq!(
            Settings::decode(unknown.as_bytes()),
            Err(SettingsError::UnknownField)
        );
        let duplicate = document.replace("emoji=true", "emoji=true\nemoji=false");
        assert_eq!(
            Settings::decode(duplicate.as_bytes()),
            Err(SettingsError::DuplicateField)
        );
        let missing = document.replace("emoji=true\n", "");
        assert_eq!(
            Settings::decode(missing.as_bytes()),
            Err(SettingsError::MissingField("emoji"))
        );
        let malformed = document.replace("emoji=true", "emoji =true");
        assert_eq!(
            Settings::decode(malformed.as_bytes()),
            Err(SettingsError::MalformedLine)
        );
    }

    #[test]
    fn invalid_utf8_embedded_carriage_return_and_oversize_are_rejected() {
        assert_eq!(Settings::decode(&[0xff]), Err(SettingsError::InvalidUtf8));
        let embedded = canonical().replace("emoji=true", "emoji=tr\rue");
        assert_eq!(
            Settings::decode(embedded.as_bytes()),
            Err(SettingsError::MalformedLine)
        );
        assert_eq!(
            Settings::decode(&vec![b'x'; MAX_SETTINGS_BYTES + 1]),
            Err(SettingsError::TooLarge)
        );
    }

    #[test]
    fn absent_file_uses_defaults_but_corrupt_file_is_not_hidden() {
        let fixture = Fixture::new();
        let path = fixture.0.join("settings.mo");
        assert_eq!(
            load(&path).unwrap(),
            LoadedSettings::Defaults(Settings::default())
        );
        fs::write(&path, b"broken").unwrap();
        assert!(matches!(
            load(&path),
            Err(StoreError::InvalidDocument(SettingsError::InvalidHeader))
        ));
    }

    #[test]
    fn atomic_save_creates_and_replaces_a_regular_file() {
        let fixture = Fixture::new();
        let path = fixture.0.join("settings.mo");
        save_atomic(&path, &Settings::default()).unwrap();
        assert_eq!(
            load(&path).unwrap(),
            LoadedSettings::Stored(Settings::default())
        );

        let changed = Settings {
            input_scheme: InputScheme::DoublePinyinMicrosoft,
            character_set: CharacterSet::Traditional,
            candidate_page_size: 8,
            show_comments: false,
            emoji: false,
            local_learning: false,
            privacy_mode: true,
            theme: Theme::Dark,
        };
        save_atomic(&path, &changed).unwrap();
        assert_eq!(load(&path).unwrap(), LoadedSettings::Stored(changed));
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }

    #[test]
    fn runtime_refresh_advances_only_for_semantic_or_origin_changes() {
        let fixture = Fixture::new();
        let path = fixture.0.join("settings.mo");
        let mut runtime = SettingsRuntime::open(&path).unwrap();
        assert_eq!(runtime.path(), path);
        assert_eq!(runtime.snapshot().revision, 1);
        assert_eq!(runtime.snapshot().origin, SettingsOrigin::Defaults);

        runtime.refresh().unwrap();
        assert_eq!(runtime.snapshot().revision, 1);
        save_atomic(&path, &Settings::default()).unwrap();
        runtime.refresh().unwrap();
        assert_eq!(runtime.snapshot().revision, 2);
        assert_eq!(runtime.snapshot().origin, SettingsOrigin::Stored);

        runtime.refresh().unwrap();
        assert_eq!(runtime.snapshot().revision, 2);
        let changed = Settings {
            theme: Theme::Dark,
            ..Settings::default()
        };
        save_atomic(&path, &changed).unwrap();
        runtime.refresh().unwrap();
        assert_eq!(runtime.snapshot().revision, 3);
        assert_eq!(runtime.snapshot().plan.presentation.theme, Theme::Dark);
    }

    #[test]
    fn failed_runtime_refresh_preserves_last_known_good_snapshot() {
        let fixture = Fixture::new();
        let path = fixture.0.join("settings.mo");
        let changed = Settings {
            theme: Theme::Light,
            ..Settings::default()
        };
        save_atomic(&path, &changed).unwrap();
        let mut runtime = SettingsRuntime::open(&path).unwrap();
        let accepted = runtime.snapshot().clone();

        fs::write(&path, b"broken").unwrap();
        assert!(matches!(
            runtime.refresh(),
            Err(RuntimeError::Store(StoreError::InvalidDocument(
                SettingsError::InvalidHeader
            )))
        ));
        assert_eq!(runtime.snapshot(), &accepted);

        save_atomic(&path, &Settings::default()).unwrap();
        runtime.refresh().unwrap();
        assert_eq!(runtime.snapshot().revision, accepted.revision + 1);
        assert_eq!(runtime.snapshot().settings, Settings::default());
    }

    #[test]
    fn store_rejects_missing_parent_directory_and_non_file_target() {
        let fixture = Fixture::new();
        let missing = fixture.0.join("missing/settings.mo");
        assert!(matches!(
            save_atomic(&missing, &Settings::default()),
            Err(StoreError::MissingParent)
        ));
        let directory = fixture.0.join("settings.mo");
        fs::create_dir(&directory).unwrap();
        assert!(matches!(
            save_atomic(&directory, &Settings::default()),
            Err(StoreError::UnsafeFileType)
        ));
        assert!(matches!(load(&directory), Err(StoreError::UnsafeFileType)));
    }

    #[test]
    fn temporary_name_collisions_do_not_replace_unrelated_files() {
        let fixture = Fixture::new();
        let path = fixture.0.join("settings.mo");
        let sequence = TEMP_SEQUENCE.load(Ordering::Relaxed);
        let collision = fixture
            .0
            .join(format!("settings.mo.tmp-{}-{sequence}", std::process::id()));
        fs::write(&collision, b"owned elsewhere").unwrap();
        save_atomic(&path, &Settings::default()).unwrap();
        assert_eq!(fs::read(&collision).unwrap(), b"owned elsewhere");
        assert_eq!(
            load(&path).unwrap(),
            LoadedSettings::Stored(Settings::default())
        );
    }

    #[test]
    fn helper_replaces_only_the_requested_line() {
        let changed = replace_line(&canonical(), "emoji=", "emoji=false");
        assert!(!Settings::decode(changed.as_bytes()).unwrap().emoji);
    }

    #[test]
    fn stale_snapshots_and_absent_stored_transitions_do_not_overwrite() {
        let fixture = Fixture::new();
        let path = fixture.0.join("settings.mo");
        let absent = load(&path).unwrap();
        let dark = Settings {
            theme: Theme::Dark,
            ..Settings::default()
        };
        save_atomic_if_unchanged(&path, &dark, &absent).unwrap();
        let before = fs::read(&path).unwrap();
        assert!(matches!(
            save_atomic_if_unchanged(&path, &Settings::default(), &absent),
            Err(StoreError::Changed)
        ));
        assert_eq!(fs::read(&path).unwrap(), before);
        let stored = load(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(matches!(
            save_atomic_if_unchanged(&path, &dark, &stored),
            Err(StoreError::Changed)
        ));
        assert!(!path.exists());
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
    }

    #[test]
    fn future_and_corrupt_external_documents_are_preserved() {
        let fixture = Fixture::new();
        let path = fixture.0.join("settings.mo");
        save_atomic(&path, &Settings::default()).unwrap();
        let expected = load(&path).unwrap();
        for bytes in [
            b"mo-settings\nformat=2\n".as_slice(),
            b"synthetic corrupt document",
        ] {
            fs::write(&path, bytes).unwrap();
            assert!(matches!(
                save_atomic_if_unchanged(&path, &Settings::default(), &expected),
                Err(StoreError::Changed)
            ));
            assert_eq!(fs::read(&path).unwrap(), bytes);
            assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
        }
    }

    #[test]
    fn guard_excludes_both_save_apis_and_releases_on_drop() {
        let fixture = Fixture::new();
        let path = fixture.0.join("settings.mo");
        let expected = load(&path).unwrap();
        let guard = WriteGuard::acquire(&path).unwrap();
        assert!(matches!(
            save_atomic(&path, &Settings::default()),
            Err(StoreError::WriteBusy)
        ));
        assert!(matches!(
            save_atomic_if_unchanged(&path, &Settings::default(), &expected),
            Err(StoreError::WriteBusy)
        ));
        assert!(!path.exists());
        drop(guard);
        save_atomic_if_unchanged(&path, &Settings::default(), &expected).unwrap();
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }

    #[test]
    fn existing_guard_marker_is_never_adopted_or_deleted() {
        let fixture = Fixture::new();
        let path = fixture.0.join("settings.mo");
        let marker = fixture.0.join("settings.mo.write-lock");
        fs::write(&marker, b"synthetic foreign owner").unwrap();
        assert!(matches!(
            save_atomic(&path, &Settings::default()),
            Err(StoreError::WriteBusy)
        ));
        assert_eq!(fs::read(&marker).unwrap(), b"synthetic foreign owner");
        assert!(!path.exists());
    }

    #[test]
    fn simultaneous_snapshots_cannot_both_commit() {
        let fixture = Fixture::new();
        let path = fixture.0.join("settings.mo");
        let expected = load(&path).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let children = (0..8)
            .map(|_| {
                let path = path.clone();
                let expected = expected.clone();
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    save_atomic_if_unchanged(&path, &Settings::default(), &expected)
                })
            })
            .collect::<Vec<_>>();
        let outcomes = children
            .into_iter()
            .map(|child| child.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(outcomes.iter().filter(|x| x.is_ok()).count(), 1);
        for outcome in outcomes {
            assert!(matches!(
                outcome,
                Ok(()) | Err(StoreError::Changed | StoreError::WriteBusy)
            ));
        }
        assert_eq!(
            load(&path).unwrap(),
            LoadedSettings::Stored(Settings::default())
        );
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "owned child entry point for writer_process_exit_releases_guard"]
    fn writer_guard_owned_child() {
        let directory =
            PathBuf::from(std::env::var_os("MO_SETTINGS_GUARD_CHILD_DIR").expect("owned fixture"));
        assert_eq!(directory.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            directory
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("mo-settings-test-")
        );
        assert_eq!(
            fs::read(directory.join("guard-fixture")).unwrap(),
            b"synthetic settings guard fixture"
        );
        let _guard = WriteGuard::acquire(&directory.join("settings.mo")).unwrap();
        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("guard-ready"))
            .unwrap();
        marker.write_all(b"owned guard ready").unwrap();
        marker.sync_all().unwrap();
        drop(marker);
        loop {
            std::thread::park();
        }
    }

    #[cfg(windows)]
    #[test]
    fn writer_process_exit_releases_guard() {
        use std::process::{Child, Command, Stdio};
        use std::time::{Duration, Instant};
        struct OwnedChild(Child);
        impl Drop for OwnedChild {
            fn drop(&mut self) {
                if !matches!(self.0.try_wait(), Ok(Some(_))) {
                    let _ = self.0.kill();
                }
                let _ = self.0.wait();
            }
        }
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join("guard-fixture"),
            b"synthetic settings guard fixture",
        )
        .unwrap();
        let path = fixture.0.join("settings.mo");
        let mut child = OwnedChild(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "tests::writer_guard_owned_child", "--ignored"])
                .env("MO_SETTINGS_GUARD_CHILD_DIR", &fixture.0)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let ready = fixture.0.join("guard-ready");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "owned child exited before acquiring guard"
            );
            assert!(Instant::now() < deadline, "owned guard readiness timeout");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(fs::read(ready).unwrap(), b"owned guard ready");
        assert!(matches!(
            save_atomic(&path, &Settings::default()),
            Err(StoreError::WriteBusy)
        ));
        assert!(!path.exists());
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        assert!(!fixture.0.join("settings.mo.write-lock").exists());
        save_atomic(&path, &Settings::default()).unwrap();
        assert_eq!(
            load(&path).unwrap(),
            LoadedSettings::Stored(Settings::default())
        );
        assert!(!fixture.0.join("settings.mo.write-lock").exists());
    }
}
