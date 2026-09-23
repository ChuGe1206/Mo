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
    MissingParent,
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "settings I/O failed: {error}"),
            Self::InvalidDocument(error) => {
                write!(formatter, "settings document is invalid: {error}")
            }
            Self::UnsafeFileType => formatter.write_str("settings path is not a regular file"),
            Self::MissingParent => formatter.write_str("settings parent directory does not exist"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidDocument(error) => Some(error),
            Self::UnsafeFileType | Self::MissingParent => None,
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
}
