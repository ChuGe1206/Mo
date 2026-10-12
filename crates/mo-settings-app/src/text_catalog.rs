//! Bounded, data-only UI localization, independent of engine settings and input.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;
use std::sync::Arc;

/// Maximum UTF-8 catalog size before parsing.
pub const MAX_CATALOG_BYTES: usize = 32 * 1024;
/// Maximum UTF-8 bytes per displayed value.
pub const MAX_TEXT_BYTES: usize = 512;
const MAX_LOCALE_CONFIG_BYTES: usize = 128;
const CATALOG_HEADER: &str = "mo-ui-text";
const CATALOG_FORMAT: &str = "format=1";
const LOCALE_CONFIG_NAME: &str = "ui-language-v1.mo";

/// Supported UI languages; values never select arbitrary resource paths.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UiLocale {
    #[default]
    ZhCn,
    EnUs,
}
impl UiLocale {
    /// Stable language identifier used in headers and filenames.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::EnUs => "en-US",
        }
    }
    /// The other bundled language for the current settings window.
    pub const fn next(self) -> Self {
        match self {
            Self::ZhCn => Self::EnUs,
            Self::EnUs => Self::ZhCn,
        }
    }
    /// Reads an optional language preference beside engine settings.
    ///
    /// # Errors
    /// Reports invalid format, excessive size or I/O errors without writing.
    pub fn from_profile(profile: &Path) -> Result<Option<Self>, TextError> {
        let bytes = match read_bounded(&profile.join(LOCALE_CONFIG_NAME), MAX_LOCALE_CONFIG_BYTES) {
            Err(TextError::Io(io::ErrorKind::NotFound)) => return Ok(None),
            result => result?,
        };
        let text = std::str::from_utf8(&bytes).map_err(|_| TextError::InvalidUtf8)?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut lines = text.lines();
        if lines.next() != Some("mo-ui-language") || lines.next() != Some(CATALOG_FORMAT) {
            return Err(TextError::InvalidHeader);
        }
        let locale = lines
            .next()
            .and_then(|line| line.strip_prefix("locale="))
            .ok_or(TextError::InvalidHeader)?
            .parse()?;
        if lines.next().is_some() {
            return Err(TextError::InvalidHeader);
        }
        Ok(Some(locale))
    }
}
impl std::str::FromStr for UiLocale {
    type Err = TextError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "zh-CN" => Ok(Self::ZhCn),
            "en-US" => Ok(Self::EnUs),
            _ => Err(TextError::UnsupportedLocale),
        }
    }
}
// One declaration owns the enum, stable resource names and complete key list.
macro_rules! text_keys {
    ($($variant:ident => $key:literal),+ $(,)?) => {
        /// Semantic keys shared by native controls, painting and accessibility.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
        pub enum TextKey { $(#[doc = concat!("UI resource key ", $key, ".")] $variant),+ }
        impl TextKey {
            /// Complete translation inventory.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            /// Stable dotted resource key.
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $key),+ }
            }
        }
    };
}
text_keys! {
    PendingSave => "pending.save",
    SettingsUnreadable => "settings.unreadable",
    General => "general",
    Appearance => "appearance",
    Privacy => "privacy",
    Save => "save",
    Restore => "restore",
    Reload => "reload",
    FullPinyin => "full.pinyin",
    NaturalPinyin => "natural.pinyin",
    FlypyPinyin => "flypy.pinyin",
    MicrosoftPinyin => "microsoft.pinyin",
    SogouPinyin => "sogou.pinyin",
    SimplifiedChinese => "simplified.chinese",
    TraditionalChinese => "traditional.chinese",
    Comments => "comments",
    Emoji => "emoji",
    Learning => "learning",
    PrivacyMode => "privacy.mode",
    SystemTheme => "system.theme",
    LightTheme => "light.theme",
    DarkTheme => "dark.theme",
    PaperSkin => "paper.skin",
    MidnightSkin => "midnight.skin",
    MintSkin => "mint.skin",
    DuskSkin => "dusk.skin",
    BambooSkin => "bamboo.skin",
    CustomSkin => "custom.skin",
    BackgroundColor => "background.color",
    AccentColor => "accent.color",
    TextColor => "text.color",
    ImportSkin => "import.skin",
    ExportSkin => "export.skin",
    BackToSkins => "back.to.skins",
    HorizontalCandidates => "horizontal.candidates",
    VerticalCandidates => "vertical.candidates",
    Enabled => "enabled",
    Disabled => "disabled",
    SettingsReloaded => "settings.reloaded",
    RestoreRequired => "restore.required",
    RestoreConfirmation => "restore.confirmation",
    DefaultsRestored => "defaults.restored",
    SettingsSaved => "settings.saved",
    SkinFileType => "skin.file.type",
    SkinExported => "skin.exported",
    SkinImported => "skin.imported",
    SkinOperationFailed => "skin.operation.failed",
    BrandTagline => "brand.tagline",
    DevelopmentBrand => "development.brand",
    BrandSubtitle => "brand.subtitle",
    CandidateMode => "candidate.mode",
    GeneralSubtitle => "general.subtitle",
    InputHabits => "input.habits",
    InputScheme => "input.scheme",
    InputSchemeHelp => "input.scheme.help",
    OutputCharacters => "output.characters",
    OutputCharactersHelp => "output.characters.help",
    CandidatesSection => "candidates.section",
    CommentsHelp => "comments.help",
    EmojiHelp => "emoji.help",
    InputChangeNotice => "input.change.notice",
    AppearanceSubtitle => "appearance.subtitle",
    WindowTheme => "window.theme",
    LivePreview => "live.preview",
    PrivacySubtitle => "privacy.subtitle",
    LocalLearningTitle => "local.learning.title",
    LocalLearningHelp => "local.learning.help",
    LearningPreferences => "learning.preferences",
    LearningHelp => "learning.help",
    PrivacyHelp => "privacy.help",
    PrivacyCallout => "privacy.callout",
    PrivacyResumeNotice => "privacy.resume.notice",
    CustomSkinSubtitle => "custom.skin.subtitle",
    MySkin => "my.skin",
    CornerRadius => "corner.radius",
    CandidateFontSize => "candidate.font.size",
    MySkinPreview => "my.skin.preview",
    SkinPreviewHelp => "skin.preview.help",
    PaperDescription => "paper.description",
    MidnightDescription => "midnight.description",
    MintDescription => "mint.description",
    DuskDescription => "dusk.description",
    BambooDescription => "bamboo.description",
    CustomDescription => "custom.description",
    GeneralNavigation => "general.navigation",
    AppearanceNavigation => "appearance.navigation",
    PrivacyNavigation => "privacy.navigation",
    BackgroundColorValue => "background.color.value",
    AccentColorValue => "accent.color.value",
    TextColorValue => "text.color.value",
    BackToSkinsNavigation => "back.to.skins.navigation",
    WindowTitle => "window.title",
    FatalStartupTitle => "fatal.startup.title",
    ToggleState => "toggle.state",
    LanguageButton => "language.button",
    LanguageChanged => "language.changed",
    TextOverrideRejected => "text.override.rejected",
}
impl TextKey {
    fn from_resource_key(key: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|candidate| candidate.as_str() == key)
    }
    fn placeholders(self) -> &'static [&'static str] {
        match self {
            Self::SkinOperationFailed => &["error"],
            Self::BackgroundColorValue | Self::AccentColorValue | Self::TextColorValue => {
                &["color"]
            }
            Self::ToggleState => &["label", "state"],
            Self::GeneralNavigation
            | Self::AppearanceNavigation
            | Self::PrivacyNavigation
            | Self::BackToSkinsNavigation => &["label"],
            _ => &[],
        }
    }
}
/// Resource errors contain no untrusted text or file contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextError {
    TooLarge,
    InvalidUtf8,
    InvalidHeader,
    UnsupportedLocale,
    UnknownKey,
    DuplicateKey,
    InvalidValue,
    InvalidPlaceholder,
    MissingDefaults,
    MissingArgument,
    UnsafeFile,
    Io(io::ErrorKind),
}
impl fmt::Display for TextError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "UI text resource error: {self:?}")
    }
}
impl std::error::Error for TextError {}

/// Immutable text catalog. Lookups share strings instead of allocating copies.
#[derive(Clone, Debug)]
pub struct UiTextCatalog {
    locale: UiLocale,
    values: BTreeMap<TextKey, Arc<str>>,
    override_error: Option<TextError>,
}
impl UiTextCatalog {
    /// Loads complete compiled defaults.
    ///
    /// # Errors
    /// Reports incomplete or invalid bundled data; both locales have tests.
    pub fn bundled(locale: UiLocale) -> Result<Self, TextError> {
        let source = match locale {
            UiLocale::ZhCn => include_str!("../../../assets/ui/text/zh-CN.mo"),
            UiLocale::EnUs => include_str!("../../../assets/ui/text/en-US.mo"),
        };
        let values = parse(source.as_bytes(), locale)?;
        if TextKey::ALL.iter().any(|key| !values.contains_key(key)) {
            return Err(TextError::MissingDefaults);
        }
        Ok(Self {
            locale,
            values,
            override_error: None,
        })
    }
    /// Reads a locale-specific partial override from a trusted profile directory.
    /// Invalid overrides apply no values; embedded defaults remain available.
    ///
    /// # Errors
    /// Returns only an invalid bundled-default error. Optional file errors are
    /// available through override_error().
    pub fn from_profile(locale: UiLocale, profile: &Path) -> Result<Self, TextError> {
        let mut result = Self::bundled(locale)?;
        let path = profile.join(format!("ui-text-{}.mo", locale.as_str()));
        match read_bounded(&path, MAX_CATALOG_BYTES) {
            Err(TextError::Io(io::ErrorKind::NotFound)) => {}
            Ok(bytes) => match result.with_override(&bytes) {
                Ok(updated) => result = updated,
                Err(error) => result.override_error = Some(error),
            },
            Err(error) => result.override_error = Some(error),
        }
        Ok(result)
    }
    /// Applies a validated partial override without mutating the original.
    ///
    /// # Errors
    /// Rejects unknown/duplicate keys, wrong locales, invalid placeholders,
    /// control characters and excessive file/value sizes.
    pub fn with_override(&self, bytes: &[u8]) -> Result<Self, TextError> {
        let overlay = parse(bytes, self.locale)?;
        let mut result = self.clone();
        result.values.extend(overlay);
        result.override_error = None;
        Ok(result)
    }
    /// Active UI locale, independent of input scheme and candidate language.
    pub const fn locale(&self) -> UiLocale {
        self.locale
    }
    /// Optional override failure without exposing its contents.
    pub const fn override_error(&self) -> Option<TextError> {
        self.override_error
    }
    /// Returns shared text for a typed key.
    ///
    /// # Panics
    /// Only if an internal complete-catalog invariant is violated. Public
    /// constructors reject incomplete defaults and overrides cannot remove keys.
    pub fn get(&self, key: TextKey) -> Arc<str> {
        // Both constructors verify the full inventory; overlays only extend it.
        self.values
            .get(&key)
            .expect("complete catalog constructor invariant")
            .clone()
    }
    /// Substitutes named values in one pass. Values are never interpreted as
    /// additional fields, paths, expressions or executable content.
    ///
    /// # Errors
    /// Returns MissingArgument when arguments differ from the key's schema.
    pub fn render(&self, key: TextKey, arguments: &[(&str, &str)]) -> Result<String, TextError> {
        let supplied: BTreeSet<_> = arguments.iter().map(|(key, _)| *key).collect();
        let required: BTreeSet<_> = key.placeholders().iter().copied().collect();
        if supplied != required || supplied.len() != arguments.len() {
            return Err(TextError::MissingArgument);
        }
        let source = self.get(key);
        let mut output = String::new();
        let mut rest = source.as_ref();
        while let Some((before, after)) = rest.split_once('{') {
            output.push_str(before);
            let (field, tail) = after.split_once('}').ok_or(TextError::InvalidPlaceholder)?;
            let value = arguments
                .iter()
                .find(|(key, _)| *key == field)
                .ok_or(TextError::MissingArgument)?
                .1;
            output.push_str(value);
            rest = tail;
        }
        output.push_str(rest);
        Ok(output)
    }
}
fn parse(bytes: &[u8], locale: UiLocale) -> Result<BTreeMap<TextKey, Arc<str>>, TextError> {
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err(TextError::TooLarge);
    }
    let source = std::str::from_utf8(bytes).map_err(|_| TextError::InvalidUtf8)?;
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut lines = source.lines();
    if lines.next() != Some(CATALOG_HEADER) || lines.next() != Some(CATALOG_FORMAT) {
        return Err(TextError::InvalidHeader);
    }
    let expected_locale = format!("locale={}", locale.as_str());
    if lines.next() != Some(expected_locale.as_str()) {
        return Err(TextError::UnsupportedLocale);
    }
    let mut result = BTreeMap::new();
    for line in lines {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, value) = line.split_once('=').ok_or(TextError::InvalidValue)?;
        let key = TextKey::from_resource_key(name).ok_or(TextError::UnknownKey)?;
        if value.is_empty() || value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
            return Err(TextError::InvalidValue);
        }
        validate_placeholders(key, value)?;
        if result.insert(key, Arc::from(value)).is_some() {
            return Err(TextError::DuplicateKey);
        }
    }
    Ok(result)
}
fn validate_placeholders(key: TextKey, value: &str) -> Result<(), TextError> {
    let mut fields = BTreeSet::new();
    let mut rest = value;
    while let Some((before, after)) = rest.split_once('{') {
        if before.contains('}') {
            return Err(TextError::InvalidPlaceholder);
        }
        let (field, tail) = after.split_once('}').ok_or(TextError::InvalidPlaceholder)?;
        if !key.placeholders().contains(&field) {
            return Err(TextError::InvalidPlaceholder);
        }
        fields.insert(field);
        rest = tail;
    }
    if rest.contains('}') || fields != key.placeholders().iter().copied().collect() {
        return Err(TextError::InvalidPlaceholder);
    }
    Ok(())
}
fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, TextError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| TextError::Io(error.kind()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(TextError::UnsafeFile);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(TextError::UnsafeFile);
        }
    }
    let file = File::open(path).map_err(|error| TextError::Io(error.kind()))?;
    let mut bytes = Vec::new();
    file.take((maximum + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| TextError::Io(error.kind()))?;
    if bytes.len() > maximum {
        return Err(TextError::TooLarge);
    }
    Ok(bytes)
}
