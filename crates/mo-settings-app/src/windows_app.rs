use mo_settings::{
    Appearance, CandidateSkin, CharacterSet, InputScheme, MAX_CANDIDATE_FONT_SIZE_PX,
    MAX_CANDIDATE_RADIUS_PX, MAX_SKIN_BYTES, MIN_CANDIDATE_FONT_SIZE_PX, MIN_CANDIDATE_RADIUS_PX,
    Settings, Theme,
};
use mo_settings_app::SettingsController;
use mo_settings_app::text_catalog::{TextKey, UiLocale, UiTextCatalog};
use std::ffi::c_void;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::{null, null_mut};
use std::sync::Arc;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::UI::Accessibility::*;
use windows_sys::Win32::UI::Controls::Dialogs::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

// Native control creation and MoveWindow can synchronously ask the parent to
// paint. Defer those reads until the mutable layout/settings operation ends.
thread_local! { static UI_MUTATING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
struct UiMutation {
    window: HWND,
    previous: bool,
}
impl UiMutation {
    fn new(window: HWND) -> Self {
        Self {
            window,
            previous: UI_MUTATING.with(|flag| flag.replace(true)),
        }
    }
}
impl Drop for UiMutation {
    fn drop(&mut self) {
        UI_MUTATING.with(|flag| flag.set(self.previous));
        if !self.previous && !self.window.is_null() {
            unsafe {
                InvalidateRect(self.window, null(), 0);
                EnumChildWindows(self.window, Some(invalidate_child), 0);
            }
        }
    }
}
unsafe extern "system" fn invalidate_child(window: HWND, _: LPARAM) -> i32 {
    unsafe {
        InvalidateRect(window, null(), 0);
    }
    1
}
// Win32 resource/control IDs are stable ordinals; only their declarations
// contain raw numbers. Layout dimensions below are logical pixels at BASE_DPI.
const BASE_DPI: u32 = 96;
const BASE_DPI_I32: i32 = BASE_DPI as i32;
const WINDOW_WIDTH_PX: i32 = 1116;
const WINDOW_HEIGHT_PX: i32 = 760;
const MIN_WINDOW_WIDTH_PX: i32 = 900;
const MIN_WINDOW_HEIGHT_PX: i32 = 520;
const PREVIEW_SMALL_HEIGHT_PX: i32 = 540;
const PREVIEW_INITIAL_SCROLL_PX: i32 = 100;
const KEYBOARD_FOCUS_MARGIN_PX: i32 = 8;
const SCROLL_LINE_PX: i32 = 32;
const WORD_SHIFT_BITS: u32 = u16::BITS;
const WORD_MASK: usize = u16::MAX as usize;
const FATAL_TITLE_FALLBACK: &str = "Mo settings could not start";
const SIDEBAR_WIDTH_PX: i32 = 212;
const FOOTER_HEIGHT_PX: i32 = 72;
const INITIAL_CONTENT_WIDTH_PX: i32 = 888;
const MIN_CONTENT_WIDTH_PX: i32 = 400;
const MIN_VIEWPORT_HEIGHT_PX: i32 = 120;
const CONTENT_HEIGHT_PX: i32 = 644;
const NARROW_LAYOUT_BREAKPOINT_PX: i32 = 720;
const NARROW_SKINS_CONTENT_HEIGHT_PX: i32 = 810;
const NARROW_CUSTOM_CONTENT_HEIGHT_PX: i32 = 1000;
const PAGE_PADDING_PX: i32 = 24;
const CARD_GAP_PX: i32 = 16;
const SCROLL_STEP_PX: i32 = 56;
const COMBO_ITEM_HEIGHT_PX: u32 = 34;
const ICON_RESOURCE_ID: u16 = 1;
const FONT_COUNT: usize = 5;
const BODY_FONT_INDEX: usize = 0;
const BODY_FONT_SIZE_PX: i32 = 14;
const CAPTION_FONT_SIZE_PX: i32 = 12;
const HEADING_FONT_SIZE_PX: i32 = 24;
const SECTION_FONT_SIZE_PX: i32 = 16;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettingsPage {
    Global,
    General,
    Appearance,
    Privacy,
    CustomSkin,
}
const PAGE_GLOBAL: SettingsPage = SettingsPage::Global;
const PAGE_GENERAL: SettingsPage = SettingsPage::General;
const PAGE_APPEARANCE: SettingsPage = SettingsPage::Appearance;
const PAGE_PRIVACY: SettingsPage = SettingsPage::Privacy;
const PAGE_CUSTOM_SKIN: SettingsPage = SettingsPage::CustomSkin;
const NAVIGATION_PAGES: [SettingsPage; 3] = [PAGE_GENERAL, PAGE_APPEARANCE, PAGE_PRIVACY];
const ID_NAV_GENERAL: usize = 200;
const ID_NAV_APPEARANCE: usize = 201;
const ID_NAV_PRIVACY: usize = 202;
const ID_THEME_SYSTEM: usize = 210;
const ID_THEME_LIGHT: usize = 211;
const ID_THEME_DARK: usize = 212;
const ID_SKIN_FIRST: usize = 220;
const ID_SKIN_LAST_PRESET: usize = 224;
const ID_SKIN_CUSTOM: usize = 225;
// Integer luma approximation (R/G/B weights sum to 1000). This preserves the
// existing preview color choice; it is not a WCAG contrast-ratio calculation.
const LUMA_RED_WEIGHT: u32 = 299;
const LUMA_GREEN_WEIGHT: u32 = 587;
const LUMA_BLUE_WEIGHT: u32 = 114;
const DARK_ON_ACCENT_LUMA_THRESHOLD: u32 = 150_000;
const RGB_CHANNEL_MASK: u32 = u8::MAX as u32;
const RGB_RED_SHIFT: u32 = 16;
const RGB_GREEN_SHIFT: u32 = 8;
const RGB_GREEN_MASK: u32 = RGB_CHANNEL_MASK << RGB_GREEN_SHIFT;
const WHITE_RGB: u32 = 0x00ff_ffff;
// BITMAPFILEHEADER + BITMAPINFOHEADER, for deterministic synthetic previews.
const BMP_FILE_HEADER_BYTES: usize = 14;
const BMP_INFO_HEADER_BYTES: usize = 40;
const BMP_PIXEL_OFFSET: usize = BMP_FILE_HEADER_BYTES + BMP_INFO_HEADER_BYTES;
const BMP_BITS_PER_PIXEL: u16 = 32;
const BMP_BYTES_PER_PIXEL: usize = 4;
const BMP_RESERVED_BYTES: usize = 4;
const BMP_INFO_UNUSED_BYTES: usize = 24;

const WM_APP_LAYOUT: u32 = WM_APP + 1;
const WM_APP_RESTYLE: u32 = WM_APP + 2;
const CLASS: &str = "Mo.Settings.Window.v2";
const VIEW_CLASS: &str = "Mo.Settings.Content.v2";
const ID_SAVE: usize = 101;
const ID_RESTORE: usize = 102;
const ID_RELOAD: usize = 103;
const ID_SCHEME: usize = 104;
const ID_CHARACTER: usize = 105;
const ID_COMMENTS: usize = 106;
const ID_EMOJI: usize = 107;
const ID_LEARNING: usize = 108;
const ID_PRIVACY: usize = 109;
const ID_BACKGROUND: usize = 120;
const ID_ACCENT: usize = 121;
const ID_TEXT: usize = 122;
const ID_RADIUS: usize = 123;
const ID_FONT: usize = 124;
const ID_IMPORT: usize = 125;
const ID_EXPORT: usize = 126;
const ID_BACK: usize = 127;
const ID_LAYOUT: usize = 128;
const ID_LANGUAGE: usize = 129;
const LANGUAGE_BUTTON_TOP_PX: i32 = 302;
const SCHEME_LABELS: [TextKey; 5] = [
    TextKey::FullPinyin,
    TextKey::NaturalPinyin,
    TextKey::FlypyPinyin,
    TextKey::MicrosoftPinyin,
    TextKey::SogouPinyin,
];
const CHARACTER_LABELS: [TextKey; 2] = [TextKey::SimplifiedChinese, TextKey::TraditionalChinese];
const LAYOUT_LABELS: [TextKey; 2] = [TextKey::HorizontalCandidates, TextKey::VerticalCandidates];
// Synthetic Chinese candidate data stays independent of the settings UI language.
const PREVIEW_CANDIDATES: [&str; 5] = ["春风", "春峰", "淳风", "春丰", "春锋"];

#[derive(Clone, Copy)]
struct Control {
    handle: HWND,
    page: SettingsPage,
    id: usize,
}
#[derive(Clone, Copy)]
struct Palette {
    background: u32,
    canvas: u32,
    text: u32,
    muted: u32,
    accent: u32,
    on_accent: u32,
    border: u32,
    soft: u32,
}
const fn rgb(value: u32) -> u32 {
    ((value >> RGB_RED_SHIFT) & RGB_CHANNEL_MASK)
        | (value & RGB_GREEN_MASK)
        | ((value & RGB_CHANNEL_MASK) << RGB_RED_SHIFT)
}
// Win32 MAKEINTRESOURCE: an ordinal tagged as a pointer, never dereferenced by Mo.
fn resource_id(id: u16) -> *const u16 {
    id as usize as *const u16
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn rect(x: i32, y: i32, width: i32, height: i32) -> RECT {
    RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    }
}

struct AppState {
    catalog: UiTextCatalog,
    controller: SettingsController,
    draft: Settings,
    window: HWND,
    view: HWND,
    controls: Vec<Control>,
    fonts: [HFONT; FONT_COUNT],
    background: HBRUSH,
    dpi: i32,
    page: SettingsPage,
    scroll: i32,
    content_width: i32,
    viewport_height: i32,
    status: String,
    palette: Palette,
}
impl Drop for AppState {
    fn drop(&mut self) {
        unsafe {
            for font in self.fonts {
                if !font.is_null() {
                    DeleteObject(font);
                }
            }
            if !self.background.is_null() {
                DeleteObject(self.background);
            }
        }
    }
}
impl AppState {
    fn new(controller: SettingsController, catalog: UiTextCatalog) -> Self {
        let status = catalog
            .get(if catalog.override_error().is_some() {
                TextKey::TextOverrideRejected
            } else if controller.can_save_changes() {
                TextKey::PendingSave
            } else {
                TextKey::SettingsUnreadable
            })
            .to_string();
        Self {
            catalog,
            draft: controller.settings().clone(),
            controller,
            window: null_mut(),
            view: null_mut(),
            controls: Vec::new(),
            fonts: [null_mut(); FONT_COUNT],
            background: null_mut(),
            dpi: BASE_DPI_I32,
            page: PAGE_GENERAL,
            scroll: 0,
            content_width: INITIAL_CONTENT_WIDTH_PX,
            viewport_height: CONTENT_HEIGHT_PX,
            status,
            palette: paper(),
        }
    }
    fn tr(&self, key: TextKey) -> Arc<str> {
        self.catalog.get(key)
    }
    fn render_text(&self, key: TextKey, arguments: &[(&str, &str)]) -> String {
        // UI call sites use the fixed field schemas exercised in both locales.
        // User overrides cannot introduce, remove or rename those fields.
        self.catalog
            .render(key, arguments)
            .expect("typed UI template arguments")
    }
    unsafe fn switch_language(&mut self) -> io::Result<()> {
        let profile = self
            .controller
            .path()
            .parent()
            .ok_or_else(|| io::Error::other("missing profile directory"))?;
        self.catalog = UiTextCatalog::from_profile(self.catalog.locale().next(), profile)
            .map_err(io::Error::other)?;
        for control in &self.controls {
            if let Some(key) = control_label_key(control.id) {
                unsafe {
                    SetWindowTextW(control.handle, wide(&self.tr(key)).as_ptr());
                }
            }
        }
        for (id, keys) in [
            (ID_SCHEME, SCHEME_LABELS.as_slice()),
            (ID_CHARACTER, CHARACTER_LABELS.as_slice()),
            (ID_LAYOUT, LAYOUT_LABELS.as_slice()),
        ] {
            unsafe {
                SendMessageW(self.control(id), CB_RESETCONTENT, 0, 0);
            }
            for key in keys {
                unsafe {
                    SendMessageW(
                        self.control(id),
                        CB_ADDSTRING,
                        0,
                        wide(&self.tr(*key)).as_ptr() as isize,
                    );
                }
            }
        }
        unsafe {
            SetWindowTextW(self.window, wide(&self.tr(TextKey::WindowTitle)).as_ptr());
            self.sync_controls();
        }
        self.status = self
            .tr(if self.catalog.override_error().is_some() {
                TextKey::TextOverrideRejected
            } else {
                TextKey::LanguageChanged
            })
            .to_string();
        Ok(())
    }
    fn s(&self, value: i32) -> i32 {
        value * self.dpi / BASE_DPI_I32
    }
    fn control(&self, id: usize) -> HWND {
        self.controls
            .iter()
            .find(|control| control.id == id)
            .map_or(null_mut(), |control| control.handle)
    }
    unsafe fn refresh_style(&mut self) {
        let _mutation = UiMutation::new(self.window);
        let mut contrast: HIGHCONTRASTW = unsafe { std::mem::zeroed() };
        contrast.cbSize = std::mem::size_of::<HIGHCONTRASTW>() as u32;
        let high_contrast = unsafe {
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                contrast.cbSize,
                (&mut contrast as *mut HIGHCONTRASTW).cast(),
                0,
            )
        } == 0
            || contrast.dwFlags & HCF_HIGHCONTRASTON != 0;
        // The scheme pointer is system-owned. Do not retain or free it.
        self.palette = if high_contrast {
            unsafe {
                Palette {
                    background: GetSysColor(COLOR_WINDOW),
                    canvas: GetSysColor(COLOR_BTNFACE),
                    text: GetSysColor(COLOR_WINDOWTEXT),
                    muted: GetSysColor(COLOR_WINDOWTEXT),
                    accent: GetSysColor(COLOR_HIGHLIGHT),
                    on_accent: GetSysColor(COLOR_HIGHLIGHTTEXT),
                    border: GetSysColor(COLOR_WINDOWFRAME),
                    soft: GetSysColor(COLOR_BTNFACE),
                }
            }
        } else if self.draft.appearance.window_theme == Theme::Dark
            || (self.draft.appearance.window_theme == Theme::System && system_dark())
        {
            midnight()
        } else {
            paper()
        };
        unsafe {
            if !self.background.is_null() {
                DeleteObject(self.background);
            }
            self.background = CreateSolidBrush(self.palette.background);
            let old_fonts = self.fonts;
            for (index, size) in [
                BODY_FONT_SIZE_PX,
                CAPTION_FONT_SIZE_PX,
                HEADING_FONT_SIZE_PX,
                SECTION_FONT_SIZE_PX,
                self.draft.appearance.font_size as i32,
            ]
            .into_iter()
            .enumerate()
            {
                self.fonts[index] = CreateFontW(
                    -self.s(size),
                    0,
                    0,
                    0,
                    if index == 2 { 500 } else { 400 },
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    OUT_DEFAULT_PRECIS as u32,
                    CLIP_DEFAULT_PRECIS as u32,
                    CLEARTYPE_QUALITY as u32,
                    DEFAULT_PITCH as u32,
                    wide("Microsoft YaHei UI").as_ptr(),
                );
            }
            for control in &self.controls {
                SendMessageW(
                    control.handle,
                    WM_SETFONT,
                    self.fonts[BODY_FONT_INDEX] as usize,
                    1,
                );
            }
            for font in old_fonts {
                if !font.is_null() {
                    DeleteObject(font);
                }
            }
        }
    }
    unsafe fn create(&mut self, window: HWND) -> io::Result<()> {
        self.window = window;
        let _mutation = UiMutation::new(window);
        self.dpi = unsafe { GetDpiForWindow(window) }.max(BASE_DPI) as i32;
        unsafe { self.refresh_style() };
        self.view = unsafe {
            CreateWindowExW(
                WS_EX_CONTROLPARENT,
                wide(VIEW_CLASS).as_ptr(),
                null(),
                WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_VSCROLL,
                0,
                0,
                1,
                1,
                window,
                null_mut(),
                GetModuleHandleW(null()),
                (self as *mut Self).cast(),
            )
        };
        if self.view.is_null() {
            return Err(io::Error::last_os_error());
        }
        for (id, label) in [
            (ID_NAV_GENERAL, &self.tr(TextKey::General)),
            (ID_NAV_APPEARANCE, &self.tr(TextKey::Appearance)),
            (ID_NAV_PRIVACY, &self.tr(TextKey::Privacy)),
            (ID_SAVE, &self.tr(TextKey::Save)),
            (ID_RESTORE, &self.tr(TextKey::Restore)),
            (ID_RELOAD, &self.tr(TextKey::Reload)),
            (ID_LANGUAGE, &self.tr(TextKey::LanguageButton)),
        ] {
            unsafe {
                self.add(
                    window,
                    PAGE_GLOBAL,
                    id,
                    "BUTTON",
                    label,
                    BS_OWNERDRAW as u32,
                )?;
            }
        }
        unsafe {
            self.add(
                self.view,
                PAGE_GENERAL,
                ID_SCHEME,
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST as u32
                    | CBS_OWNERDRAWFIXED as u32
                    | CBS_HASSTRINGS as u32
                    | WS_VSCROLL,
            )?;
            self.add(
                self.view,
                PAGE_GENERAL,
                ID_CHARACTER,
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST as u32
                    | CBS_OWNERDRAWFIXED as u32
                    | CBS_HASSTRINGS as u32
                    | WS_VSCROLL,
            )?;
        }
        for key in SCHEME_LABELS {
            let label = self.tr(key);
            unsafe {
                SendMessageW(
                    self.control(ID_SCHEME),
                    CB_ADDSTRING,
                    0,
                    wide(&label).as_ptr() as isize,
                );
            }
        }
        for key in CHARACTER_LABELS {
            let label = self.tr(key);
            unsafe {
                SendMessageW(
                    self.control(ID_CHARACTER),
                    CB_ADDSTRING,
                    0,
                    wide(&label).as_ptr() as isize,
                );
            }
        }
        for (page, id, label) in [
            (PAGE_GENERAL, ID_COMMENTS, &self.tr(TextKey::Comments)),
            (PAGE_GENERAL, ID_EMOJI, &self.tr(TextKey::Emoji)),
            (PAGE_PRIVACY, ID_LEARNING, &self.tr(TextKey::Learning)),
            (PAGE_PRIVACY, ID_PRIVACY, &self.tr(TextKey::PrivacyMode)),
        ] {
            unsafe {
                self.add(self.view, page, id, "BUTTON", label, BS_OWNERDRAW as u32)?;
            }
        }
        for (index, label) in [
            &self.tr(TextKey::SystemTheme),
            &self.tr(TextKey::LightTheme),
            &self.tr(TextKey::DarkTheme),
        ]
        .into_iter()
        .enumerate()
        {
            unsafe {
                self.add(
                    self.view,
                    PAGE_APPEARANCE,
                    ID_THEME_SYSTEM + index,
                    "BUTTON",
                    label,
                    BS_OWNERDRAW as u32,
                )?;
            }
        }
        for (index, label) in [
            &self.tr(TextKey::PaperSkin),
            &self.tr(TextKey::MidnightSkin),
            &self.tr(TextKey::MintSkin),
            &self.tr(TextKey::DuskSkin),
            &self.tr(TextKey::BambooSkin),
            &self.tr(TextKey::CustomSkin),
        ]
        .into_iter()
        .enumerate()
        {
            unsafe {
                self.add(
                    self.view,
                    PAGE_APPEARANCE,
                    ID_SKIN_FIRST + index,
                    "BUTTON",
                    label,
                    BS_OWNERDRAW as u32,
                )?;
            }
        }
        for (id, label) in [
            (ID_BACKGROUND, &self.tr(TextKey::BackgroundColor)),
            (ID_ACCENT, &self.tr(TextKey::AccentColor)),
            (ID_TEXT, &self.tr(TextKey::TextColor)),
            (ID_IMPORT, &self.tr(TextKey::ImportSkin)),
            (ID_EXPORT, &self.tr(TextKey::ExportSkin)),
            (ID_BACK, &self.tr(TextKey::BackToSkins)),
        ] {
            unsafe {
                self.add(
                    self.view,
                    PAGE_CUSTOM_SKIN,
                    id,
                    "BUTTON",
                    label,
                    BS_OWNERDRAW as u32,
                )?;
            }
        }
        for id in [ID_RADIUS, ID_FONT, ID_LAYOUT] {
            unsafe {
                self.add(
                    self.view,
                    PAGE_CUSTOM_SKIN,
                    id,
                    "COMBOBOX",
                    "",
                    CBS_DROPDOWNLIST as u32
                        | CBS_OWNERDRAWFIXED as u32
                        | CBS_HASSTRINGS as u32
                        | WS_VSCROLL,
                )?;
            }
        }
        for value in MIN_CANDIDATE_RADIUS_PX..=MAX_CANDIDATE_RADIUS_PX {
            unsafe {
                SendMessageW(
                    self.control(ID_RADIUS),
                    CB_ADDSTRING,
                    0,
                    wide(&format!("{value} px")).as_ptr() as isize,
                );
            }
        }
        for value in MIN_CANDIDATE_FONT_SIZE_PX..=MAX_CANDIDATE_FONT_SIZE_PX {
            unsafe {
                SendMessageW(
                    self.control(ID_FONT),
                    CB_ADDSTRING,
                    0,
                    wide(&format!("{value} px")).as_ptr() as isize,
                );
            }
        }
        for key in LAYOUT_LABELS {
            let label = self.tr(key);
            unsafe {
                SendMessageW(
                    self.control(ID_LAYOUT),
                    CB_ADDSTRING,
                    0,
                    wide(&label).as_ptr() as isize,
                );
            }
        }
        unsafe {
            self.sync_controls();
            self.layout();
        }
        Ok(())
    }
    unsafe fn add(
        &mut self,
        parent: HWND,
        page: SettingsPage,
        id: usize,
        class: &str,
        text: &str,
        style: u32,
    ) -> io::Result<()> {
        let handle = unsafe {
            CreateWindowExW(
                0,
                wide(class).as_ptr(),
                wide(text).as_ptr(),
                WS_CHILD | WS_TABSTOP | style,
                0,
                0,
                1,
                1,
                parent,
                id as *mut c_void,
                GetModuleHandleW(null()),
                null(),
            )
        };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        unsafe {
            SendMessageW(handle, WM_SETFONT, self.fonts[BODY_FONT_INDEX] as usize, 0);
        }
        self.controls.push(Control { handle, page, id });
        Ok(())
    }
    unsafe fn sync_controls(&mut self) {
        let _mutation = UiMutation::new(self.window);
        unsafe {
            SendMessageW(
                self.control(ID_SCHEME),
                CB_SETCURSEL,
                self.draft.input_scheme as usize,
                0,
            );
            SendMessageW(
                self.control(ID_CHARACTER),
                CB_SETCURSEL,
                self.draft.character_set as usize,
                0,
            );
            SendMessageW(
                self.control(ID_RADIUS),
                CB_SETCURSEL,
                (self.draft.appearance.radius - MIN_CANDIDATE_RADIUS_PX) as usize,
                0,
            );
            SendMessageW(
                self.control(ID_FONT),
                CB_SETCURSEL,
                (self.draft.appearance.font_size - MIN_CANDIDATE_FONT_SIZE_PX) as usize,
                0,
            );
            SendMessageW(
                self.control(ID_LAYOUT),
                CB_SETCURSEL,
                usize::from(self.draft.appearance.vertical),
                0,
            );
            EnableWindow(
                self.control(ID_SAVE),
                i32::from(self.controller.can_save_changes()),
            );
        }
        for (id, name, enabled) in [
            (
                ID_COMMENTS,
                &self.tr(TextKey::Comments),
                self.draft.show_comments,
            ),
            (ID_EMOJI, &self.tr(TextKey::Emoji), self.draft.emoji),
            (
                ID_LEARNING,
                &self.tr(TextKey::Learning),
                self.draft.local_learning,
            ),
            (
                ID_PRIVACY,
                &self.tr(TextKey::PrivacyMode),
                self.draft.privacy_mode,
            ),
        ] {
            unsafe {
                SetWindowTextW(
                    self.control(id),
                    wide(&self.render_text(
                        TextKey::ToggleState,
                        &[
                            ("label", name),
                            (
                                "state",
                                &self.tr(if enabled {
                                    TextKey::Enabled
                                } else {
                                    TextKey::Disabled
                                }),
                            ),
                        ],
                    ))
                    .as_ptr(),
                );
            }
        }
    }
    fn content_height(&self) -> i32 {
        if self.page == PAGE_CUSTOM_SKIN && self.content_width < NARROW_LAYOUT_BREAKPOINT_PX {
            return NARROW_CUSTOM_CONTENT_HEIGHT_PX;
        }
        if self.page == PAGE_APPEARANCE && self.content_width < NARROW_LAYOUT_BREAKPOINT_PX {
            NARROW_SKINS_CONTENT_HEIGHT_PX
        } else {
            CONTENT_HEIGHT_PX
        }
    }
    unsafe fn layout(&mut self) {
        let _mutation = UiMutation::new(self.window);
        let mut client: RECT = unsafe { std::mem::zeroed() };
        unsafe {
            GetClientRect(self.window, &mut client);
        }
        let width = client.right * BASE_DPI_I32 / self.dpi;
        let height = client.bottom * BASE_DPI_I32 / self.dpi;
        self.content_width = (width - SIDEBAR_WIDTH_PX).max(MIN_CONTENT_WIDTH_PX);
        self.viewport_height = (height - FOOTER_HEIGHT_PX).max(MIN_VIEWPORT_HEIGHT_PX);
        self.scroll = self
            .scroll
            .clamp(0, (self.content_height() - self.viewport_height).max(0));
        unsafe {
            MoveWindow(
                self.view,
                self.s(SIDEBAR_WIDTH_PX),
                0,
                self.s(self.content_width),
                self.s(self.viewport_height),
                1,
            );
        }
        let info = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
            nMin: 0,
            nMax: self.content_height() - 1,
            nPage: self.viewport_height as u32,
            nPos: self.scroll,
            nTrackPos: 0,
        };
        unsafe {
            SetScrollInfo(self.view, SB_VERT, &info, 1);
        }
        let w = self.content_width;
        let columns = if w < NARROW_LAYOUT_BREAKPOINT_PX {
            2
        } else {
            3
        };
        let card_width = (w - PAGE_PADDING_PX * 2 - (columns - 1) * CARD_GAP_PX) / columns;
        for control in &self.controls {
            let id = control.id;
            let logical = match id {
                ID_NAV_GENERAL..=ID_NAV_PRIVACY => {
                    rect(16, 122 + (id as i32 - ID_NAV_GENERAL as i32) * 52, 180, 44)
                }
                ID_RESTORE => rect(236, height - 56, 110, 40),
                ID_RELOAD => rect(width - 276, height - 56, 120, 40),
                ID_SAVE => rect(width - 144, height - 56, 120, 40),
                ID_SCHEME => rect(w - 244, 156, 196, 200),
                ID_CHARACTER => rect(w - 244, 236, 196, 120),
                ID_COMMENTS => rect(w - 96, 410, 48, 32),
                ID_EMOJI => rect(w - 96, 490, 48, 32),
                ID_LEARNING => rect(w - 96, 310, 48, 32),
                ID_PRIVACY => rect(w - 96, 390, 48, 32),
                ID_THEME_SYSTEM..=ID_THEME_DARK => rect(
                    w - 316 + (id as i32 - ID_THEME_SYSTEM as i32) * 96,
                    108,
                    92,
                    32,
                ),
                ID_SKIN_FIRST..=ID_SKIN_CUSTOM => {
                    let index = id as i32 - ID_SKIN_FIRST as i32;
                    rect(
                        24 + (index % columns) * (card_width + CARD_GAP_PX),
                        168 + (index / columns) * 152,
                        card_width,
                        136,
                    )
                }
                ID_LANGUAGE => rect(16, LANGUAGE_BUTTON_TOP_PX, 180, 36),
                ID_BACK => rect(24, 88, 120, 32),
                ID_BACKGROUND => rect(44, 182, 240, 40),
                ID_ACCENT => rect(44, 230, 240, 40),
                ID_TEXT => rect(44, 278, 240, 40),
                ID_RADIUS => rect(160, 330, 124, 170),
                ID_FONT => rect(160, 378, 124, 170),
                ID_LAYOUT => rect(44, 430, 240, 100),
                ID_IMPORT => rect(
                    24,
                    if w < NARROW_LAYOUT_BREAKPOINT_PX {
                        904
                    } else {
                        552
                    },
                    140,
                    40,
                ),
                ID_EXPORT => rect(
                    180,
                    if w < NARROW_LAYOUT_BREAKPOINT_PX {
                        904
                    } else {
                        552
                    },
                    140,
                    40,
                ),
                _ => rect(0, 0, 1, 1),
            };
            let visible = control.page == PAGE_GLOBAL || control.page == self.page;
            unsafe {
                ShowWindow(control.handle, if visible { SW_SHOW } else { SW_HIDE });
                MoveWindow(
                    control.handle,
                    self.s(logical.left),
                    self.s(logical.top
                        - if control.page != PAGE_GLOBAL {
                            self.scroll
                        } else {
                            0
                        }),
                    self.s(logical.right - logical.left),
                    self.s(logical.bottom - logical.top),
                    1,
                );
            }
        }
        unsafe {
            InvalidateRect(self.window, null(), 0);
            InvalidateRect(self.view, null(), 0);
        }
    }
    unsafe fn command(&mut self, id: usize, notification: u16) {
        let _mutation = UiMutation::new(self.window);
        match id {
            ID_LANGUAGE => {
                if let Err(error) = unsafe { self.switch_language() } {
                    self.status = error.to_string();
                }
            }
            ID_NAV_GENERAL..=ID_NAV_PRIVACY => {
                self.page = NAVIGATION_PAGES[id - ID_NAV_GENERAL];
                self.scroll = 0;
            }
            ID_BACK => {
                self.page = PAGE_APPEARANCE;
                self.scroll = 0;
            }
            ID_COMMENTS => self.draft.show_comments = !self.draft.show_comments,
            ID_EMOJI => self.draft.emoji = !self.draft.emoji,
            ID_LEARNING => self.draft.local_learning = !self.draft.local_learning,
            ID_PRIVACY => self.draft.privacy_mode = !self.draft.privacy_mode,
            ID_THEME_SYSTEM..=ID_THEME_DARK => {
                self.draft.appearance.window_theme =
                    [Theme::System, Theme::Light, Theme::Dark][id - ID_THEME_SYSTEM];
                unsafe {
                    self.refresh_style();
                }
            }
            ID_SKIN_FIRST..=ID_SKIN_LAST_PRESET => {
                self.draft.appearance.skin = CandidateSkin::from_index(
                    (id - ID_SKIN_FIRST + CandidateSkin::Paper as usize) as u8,
                )
                .unwrap()
            }
            ID_SKIN_CUSTOM => {
                self.page = PAGE_CUSTOM_SKIN;
                self.scroll = 0;
                self.draft.appearance.skin = CandidateSkin::Custom;
            }
            ID_SCHEME if notification == CBN_SELCHANGE as u16 => {
                let index = unsafe { SendMessageW(self.control(id), CB_GETCURSEL, 0, 0) };
                if let Some(value) = [
                    InputScheme::FullPinyin,
                    InputScheme::DoublePinyinNatural,
                    InputScheme::DoublePinyinFlypy,
                    InputScheme::DoublePinyinMicrosoft,
                    InputScheme::DoublePinyinSogou,
                ]
                .get(index as usize)
                {
                    self.draft.input_scheme = *value;
                }
            }
            ID_CHARACTER if notification == CBN_SELCHANGE as u16 => {
                self.draft.character_set =
                    if unsafe { SendMessageW(self.control(id), CB_GETCURSEL, 0, 0) } == 1 {
                        CharacterSet::Traditional
                    } else {
                        CharacterSet::Simplified
                    };
            }
            ID_LAYOUT if notification == CBN_SELCHANGE as u16 => {
                self.draft.appearance.vertical =
                    unsafe { SendMessageW(self.control(id), CB_GETCURSEL, 0, 0) } == 1;
            }
            ID_RADIUS | ID_FONT if notification == CBN_SELCHANGE as u16 => {
                let index = unsafe { SendMessageW(self.control(id), CB_GETCURSEL, 0, 0) };
                let maximum_index = if id == ID_RADIUS {
                    MAX_CANDIDATE_RADIUS_PX - MIN_CANDIDATE_RADIUS_PX
                } else {
                    MAX_CANDIDATE_FONT_SIZE_PX - MIN_CANDIDATE_FONT_SIZE_PX
                };
                if (0..=isize::from(maximum_index)).contains(&index) {
                    if id == ID_RADIUS {
                        self.draft.appearance.radius = index as u8 + MIN_CANDIDATE_RADIUS_PX;
                    } else {
                        self.draft.appearance.font_size = index as u8 + MIN_CANDIDATE_FONT_SIZE_PX;
                    }
                }
                unsafe {
                    self.refresh_style();
                }
            }
            ID_BACKGROUND | ID_ACCENT | ID_TEXT => unsafe {
                self.choose_color(id);
            },
            ID_IMPORT => unsafe {
                self.skin_file(false);
            },
            ID_EXPORT => unsafe {
                self.skin_file(true);
            },
            ID_RELOAD => {
                self.controller.reload();
                self.draft = self.controller.settings().clone();
                self.status = if self.controller.can_save_changes() {
                    self.tr(TextKey::SettingsReloaded).to_string()
                } else {
                    self.tr(TextKey::RestoreRequired).to_string()
                };
                unsafe {
                    self.refresh_style();
                }
            }
            ID_RESTORE => {
                if unsafe {
                    MessageBoxW(
                        self.window,
                        wide(&self.tr(TextKey::RestoreConfirmation)).as_ptr(),
                        wide(&self.tr(TextKey::Restore)).as_ptr(),
                        MB_YESNO | MB_ICONQUESTION,
                    )
                } == IDYES
                {
                    match self.controller.restore_defaults() {
                        Ok(()) => {
                            self.draft = self.controller.settings().clone();
                            self.status = self.tr(TextKey::DefaultsRestored).to_string();
                            notify_settings();
                            unsafe {
                                self.refresh_style();
                            }
                        }
                        Err(error) => self.status = error.to_string(),
                    }
                }
            }
            ID_SAVE => match self.controller.save_settings(self.draft.clone()) {
                Ok(()) => {
                    self.status = self.tr(TextKey::SettingsSaved).to_string();
                    notify_settings();
                }
                Err(error) => self.status = error.to_string(),
            },
            _ => {}
        }
        unsafe {
            self.sync_controls();
            self.layout();
        }
    }
    unsafe fn choose_color(&mut self, id: usize) {
        let value = match id {
            ID_BACKGROUND => self.draft.appearance.background,
            ID_ACCENT => self.draft.appearance.accent,
            _ => self.draft.appearance.text,
        };
        let mut colors = [0u32; 16];
        let mut choice: CHOOSECOLORW = unsafe { std::mem::zeroed() };
        choice.lStructSize = std::mem::size_of::<CHOOSECOLORW>() as u32;
        choice.hwndOwner = self.window;
        choice.rgbResult = rgb(value);
        choice.lpCustColors = colors.as_mut_ptr();
        choice.Flags = CC_FULLOPEN | CC_RGBINIT;
        if unsafe { ChooseColorW(&mut choice) } != 0 {
            let color = rgb(choice.rgbResult);
            match id {
                ID_BACKGROUND => self.draft.appearance.background = color,
                ID_ACCENT => self.draft.appearance.accent = color,
                _ => self.draft.appearance.text = color,
            }
        }
    }
    unsafe fn skin_file(&mut self, save: bool) {
        let mut path = [0u16; 32768];
        let filter = wide(&format!(
            "{} (*.mo-skin)\0*.mo-skin\0\0",
            self.tr(TextKey::SkinFileType)
        ));
        let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
        dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        dialog.hwndOwner = self.window;
        dialog.lpstrFilter = filter.as_ptr();
        dialog.lpstrFile = path.as_mut_ptr();
        dialog.nMaxFile = path.len() as u32;
        let extension = wide("mo-skin");
        dialog.lpstrDefExt = extension.as_ptr();
        dialog.Flags = OFN_NOCHANGEDIR
            | OFN_PATHMUSTEXIST
            | if save {
                OFN_OVERWRITEPROMPT
            } else {
                OFN_FILEMUSTEXIST
            };
        let selected = unsafe {
            if save {
                GetSaveFileNameW(&mut dialog)
            } else {
                GetOpenFileNameW(&mut dialog)
            }
        };
        if selected == 0 {
            return;
        }
        let length = path
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(path.len());
        let path = std::path::PathBuf::from(String::from_utf16_lossy(&path[..length]));
        let result = if save {
            self.draft
                .appearance
                .encode_skin()
                .map_err(io::Error::other)
                .and_then(|bytes| std::fs::write(&path, bytes))
        } else {
            use std::io::Read;
            std::fs::File::open(&path).and_then(|file| {
                let mut bytes = Vec::new();
                file.take((MAX_SKIN_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)?;
                let mut appearance = Appearance::decode_skin(&bytes).map_err(io::Error::other)?;
                appearance.window_theme = self.draft.appearance.window_theme;
                self.draft.appearance = appearance;
                Ok(())
            })
        };
        self.status = match result {
            Ok(()) => {
                if save {
                    self.tr(TextKey::SkinExported).to_string()
                } else {
                    self.tr(TextKey::SkinImported).to_string()
                }
            }
            Err(error) => self.render_text(
                TextKey::SkinOperationFailed,
                &[("error", &error.to_string())],
            ),
        };
    }
}
// Named design palettes store RGB source data in one declaration each.
fn control_label_key(id: usize) -> Option<TextKey> {
    match id {
        ID_NAV_GENERAL => Some(TextKey::General),
        ID_NAV_APPEARANCE => Some(TextKey::Appearance),
        ID_NAV_PRIVACY => Some(TextKey::Privacy),
        ID_SAVE => Some(TextKey::Save),
        ID_RESTORE => Some(TextKey::Restore),
        ID_RELOAD => Some(TextKey::Reload),
        ID_LANGUAGE => Some(TextKey::LanguageButton),
        ID_COMMENTS => Some(TextKey::Comments),
        ID_EMOJI => Some(TextKey::Emoji),
        ID_LEARNING => Some(TextKey::Learning),
        ID_PRIVACY => Some(TextKey::PrivacyMode),
        ID_THEME_SYSTEM => Some(TextKey::SystemTheme),
        ID_THEME_LIGHT => Some(TextKey::LightTheme),
        ID_THEME_DARK => Some(TextKey::DarkTheme),
        ID_BACKGROUND => Some(TextKey::BackgroundColor),
        ID_ACCENT => Some(TextKey::AccentColor),
        ID_TEXT => Some(TextKey::TextColor),
        ID_IMPORT => Some(TextKey::ImportSkin),
        ID_EXPORT => Some(TextKey::ExportSkin),
        ID_BACK => Some(TextKey::BackToSkins),
        ID_SKIN_FIRST..=ID_SKIN_CUSTOM => [
            TextKey::PaperSkin,
            TextKey::MidnightSkin,
            TextKey::MintSkin,
            TextKey::DuskSkin,
            TextKey::BambooSkin,
            TextKey::CustomSkin,
        ]
        .get(id - ID_SKIN_FIRST)
        .copied(),
        _ => None,
    }
}

const PAPER_PALETTE: Palette = Palette {
    background: rgb(WHITE_RGB),
    canvas: rgb(0x00f4_f6f5),
    text: rgb(0x0025_352f),
    muted: rgb(0x0062_6f68),
    accent: rgb(0x0017_6d5e),
    on_accent: rgb(WHITE_RGB),
    border: rgb(0x00e0_e6e2),
    soft: rgb(0x00ea_f3ee),
};
const MIDNIGHT_PALETTE: Palette = Palette {
    background: rgb(0x0024_2b29),
    canvas: rgb(0x001d_2321),
    text: rgb(0x00ef_f5f1),
    muted: rgb(0x00b0_beb5),
    accent: rgb(0x009a_dac5),
    on_accent: rgb(0x0015_352c),
    border: rgb(0x004c_5952),
    soft: rgb(0x0035_443d),
};
#[derive(Clone, Copy)]
struct CandidateColors {
    background: u32,
    text: u32,
    accent: u32,
    muted: u32,
    border: u32,
}
const MINT_COLORS: CandidateColors = CandidateColors {
    background: 0x00f0_f8f2,
    text: 0x0021_3d2c,
    accent: 0x0028_6748,
    muted: 0x0052_6a59,
    border: 0x00c7_decf,
};
const DUSK_COLORS: CandidateColors = CandidateColors {
    background: 0x00ff_f4ef,
    text: 0x004a_3029,
    accent: 0x0098_503f,
    muted: 0x0079_5c50,
    border: 0x00e8_d2c7,
};
const BAMBOO_COLORS: CandidateColors = CandidateColors {
    background: 0x00f4_f4ea,
    text: 0x0030_3c2a,
    accent: 0x0052_6143,
    muted: 0x0060_684f,
    border: 0x00d4_dac5,
};
fn paper() -> Palette {
    PAPER_PALETTE
}
fn midnight() -> Palette {
    MIDNIGHT_PALETTE
}
fn skin_palette(index: u8, custom: Appearance) -> Palette {
    let colors = match CandidateSkin::from_index(index).unwrap_or_default() {
        CandidateSkin::Midnight => return midnight(),
        CandidateSkin::Mint => MINT_COLORS,
        CandidateSkin::Dusk => DUSK_COLORS,
        CandidateSkin::Bamboo => BAMBOO_COLORS,
        CandidateSkin::Custom => CandidateColors {
            background: custom.background,
            text: custom.text,
            accent: custom.accent,
            muted: custom.text,
            border: custom.text,
        },
        CandidateSkin::FollowTheme | CandidateSkin::Paper => return paper(),
    };
    let brightness = ((colors.accent >> RGB_RED_SHIFT) & RGB_CHANNEL_MASK) * LUMA_RED_WEIGHT
        + ((colors.accent >> RGB_GREEN_SHIFT) & RGB_CHANNEL_MASK) * LUMA_GREEN_WEIGHT
        + (colors.accent & RGB_CHANNEL_MASK) * LUMA_BLUE_WEIGHT;
    Palette {
        background: rgb(colors.background),
        canvas: rgb(colors.background),
        text: rgb(colors.text),
        muted: rgb(colors.muted),
        accent: rgb(colors.accent),
        on_accent: if brightness > DARK_ON_ACCENT_LUMA_THRESHOLD {
            0
        } else {
            rgb(WHITE_RGB)
        },
        border: rgb(colors.border),
        soft: rgb(colors.background),
    }
}
unsafe fn fill(dc: HDC, bounds: RECT, color: u32) {
    unsafe {
        let brush = CreateSolidBrush(color);
        if !brush.is_null() {
            FillRect(dc, &bounds, brush);
            DeleteObject(brush);
        }
    }
}
unsafe fn rounded(dc: HDC, bounds: RECT, color: u32, border: u32, radius: i32) {
    unsafe {
        let brush = CreateSolidBrush(color);
        let pen = CreatePen(PS_SOLID, 1, border);
        if brush.is_null() || pen.is_null() {
            if !brush.is_null() {
                DeleteObject(brush);
            }
            if !pen.is_null() {
                DeleteObject(pen);
            }
            return;
        }
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, pen);
        RoundRect(
            dc,
            bounds.left,
            bounds.top,
            bounds.right,
            bounds.bottom,
            radius * 2,
            radius * 2,
        );
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(brush);
        DeleteObject(pen);
    }
}
impl AppState {
    unsafe fn text(&self, dc: HDC, text: &str, bounds: RECT, color: u32, font: usize, flags: u32) {
        let mut bounds = bounds;
        unsafe {
            let old = SelectObject(dc, self.fonts[font]);
            let previous = SetTextColor(dc, color);
            let background = SetBkMode(dc, TRANSPARENT as i32);
            DrawTextW(
                dc,
                wide(text).as_ptr(),
                -1,
                &mut bounds,
                flags | DT_NOPREFIX,
            );
            SetBkMode(dc, background);
            SetTextColor(dc, previous);
            SelectObject(dc, old);
        }
    }
    fn box_at(&self, x: i32, y: i32, width: i32, height: i32) -> RECT {
        rect(self.s(x), self.s(y), self.s(width), self.s(height))
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn label(
        &self,
        dc: HDC,
        text: &str,
        x: i32,
        y: i32,
        width: i32,
        color: u32,
        font: usize,
    ) {
        unsafe {
            self.text(
                dc,
                text,
                self.box_at(x, y, width, 28),
                color,
                font,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
            );
        }
    }
    unsafe fn paint_root(&self, dc: HDC) {
        let mut client: RECT = unsafe { std::mem::zeroed() };
        unsafe {
            GetClientRect(self.window, &mut client);
        }
        let height = client.bottom * BASE_DPI_I32 / self.dpi;
        unsafe {
            fill(dc, client, self.palette.background);
            fill(
                dc,
                self.box_at(0, 0, SIDEBAR_WIDTH_PX, height),
                self.palette.canvas,
            );
            let icon = LoadImageW(
                GetModuleHandleW(null()),
                resource_id(ICON_RESOURCE_ID),
                IMAGE_ICON,
                self.s(36),
                self.s(36),
                LR_DEFAULTCOLOR,
            );
            if !icon.is_null() {
                DrawIconEx(
                    dc,
                    self.s(20),
                    self.s(26),
                    icon,
                    self.s(36),
                    self.s(36),
                    0,
                    null_mut(),
                    DI_NORMAL,
                );
                DestroyIcon(icon);
            }
            self.label(dc, "Mo", 68, 30, 120, self.palette.text, 2);
            self.label(
                dc,
                &self.tr(TextKey::BrandTagline),
                20,
                76,
                180,
                self.palette.muted,
                1,
            );
            self.label(
                dc,
                &self.tr(TextKey::DevelopmentBrand),
                20,
                height - 82,
                180,
                self.palette.muted,
                1,
            );
            self.label(
                dc,
                &self.tr(TextKey::BrandSubtitle),
                20,
                height - 54,
                180,
                self.palette.muted,
                1,
            );
            fill(
                dc,
                self.box_at(
                    SIDEBAR_WIDTH_PX,
                    height - FOOTER_HEIGHT_PX,
                    client.right * BASE_DPI_I32 / self.dpi - SIDEBAR_WIDTH_PX,
                    1,
                ),
                self.palette.border,
            );
            self.text(
                dc,
                &self.status,
                self.box_at(
                    352,
                    height - 60,
                    (client.right * BASE_DPI_I32 / self.dpi - 648).max(100),
                    48,
                ),
                self.palette.muted,
                1,
                DT_LEFT | DT_WORDBREAK,
            );
        }
    }
    unsafe fn heading(&self, dc: HDC, title: &str, subtitle: &str) {
        unsafe {
            self.label(
                dc,
                title,
                24,
                24,
                self.content_width - 48,
                self.palette.text,
                2,
            );
            self.label(
                dc,
                subtitle,
                24,
                62,
                self.content_width - 48,
                self.palette.muted,
                1,
            );
        }
    }
    unsafe fn setting_row(&self, dc: HDC, title: &str, description: &str, y: i32) {
        unsafe {
            self.label(
                dc,
                title,
                48,
                y,
                self.content_width - 330,
                self.palette.text,
                0,
            );
            self.label(
                dc,
                description,
                48,
                y + 28,
                self.content_width - 330,
                self.palette.muted,
                1,
            );
        }
    }
    unsafe fn candidate_preview(&self, dc: HDC, bounds: RECT, palette: Palette, vertical: bool) {
        let width = (bounds.right - bounds.left) * BASE_DPI_I32 / self.dpi;
        unsafe {
            rounded(
                dc,
                bounds,
                palette.background,
                palette.border,
                self.s(self.draft.appearance.radius as i32),
            );
            let saved = SaveDC(dc);
            OffsetViewportOrgEx(dc, bounds.left, bounds.top, null_mut());
            self.label(dc, "chun feng", 12, 8, width - 90, palette.text, 0);
            self.label(
                dc,
                &self.tr(TextKey::CandidateMode),
                width - 68,
                8,
                56,
                palette.muted,
                1,
            );
            let count = if !vertical && width < 300 { 2 } else { 5 };
            for (index, text) in PREVIEW_CANDIDATES.into_iter().take(count).enumerate() {
                let (x, y, item_width) = if vertical {
                    (12, 44 + index as i32 * 36, width - 24)
                } else {
                    (
                        12 + index as i32 * ((width - 24) / count as i32),
                        42,
                        (width - 24) / count as i32 - 4,
                    )
                };
                if index == 0 {
                    rounded(
                        dc,
                        self.box_at(x, y, item_width, 32),
                        palette.accent,
                        palette.accent,
                        self.s(8),
                    );
                }
                self.label(
                    dc,
                    &format!("{} {text}", index + 1),
                    x + 8,
                    y + 2,
                    item_width - 8,
                    if index == 0 {
                        palette.on_accent
                    } else {
                        palette.text
                    },
                    if width < 300 {
                        0
                    } else if self.draft.appearance.skin == CandidateSkin::Custom {
                        4
                    } else {
                        3
                    },
                );
            }
            RestoreDC(dc, saved);
        }
    }
    unsafe fn paint_content(&self, dc: HDC) {
        let mut client: RECT = unsafe { std::mem::zeroed() };
        unsafe {
            GetClientRect(self.view, &mut client);
        }
        unsafe {
            fill(dc, client, self.palette.background);
            let saved = SaveDC(dc);
            OffsetViewportOrgEx(dc, 0, -self.s(self.scroll), null_mut());
            let w = self.content_width;
            match self.page {
                PAGE_GENERAL => {
                    self.heading(
                        dc,
                        &self.tr(TextKey::General),
                        &self.tr(TextKey::GeneralSubtitle),
                    );
                    rounded(
                        dc,
                        self.box_at(24, 104, w - 48, 216),
                        self.palette.background,
                        self.palette.border,
                        self.s(12),
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::InputHabits),
                        48,
                        120,
                        w - 96,
                        self.palette.text,
                        3,
                    );
                    self.setting_row(
                        dc,
                        &self.tr(TextKey::InputScheme),
                        &self.tr(TextKey::InputSchemeHelp),
                        156,
                    );
                    self.setting_row(
                        dc,
                        &self.tr(TextKey::OutputCharacters),
                        &self.tr(TextKey::OutputCharactersHelp),
                        236,
                    );
                    rounded(
                        dc,
                        self.box_at(24, 344, w - 48, 232),
                        self.palette.background,
                        self.palette.border,
                        self.s(12),
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::CandidatesSection),
                        48,
                        360,
                        w - 96,
                        self.palette.text,
                        3,
                    );
                    self.setting_row(
                        dc,
                        &self.tr(TextKey::Comments),
                        &self.tr(TextKey::CommentsHelp),
                        408,
                    );
                    self.setting_row(
                        dc,
                        &self.tr(TextKey::Emoji),
                        &self.tr(TextKey::EmojiHelp),
                        488,
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::InputChangeNotice),
                        24,
                        592,
                        w - 48,
                        self.palette.muted,
                        1,
                    );
                }
                PAGE_APPEARANCE => {
                    self.heading(
                        dc,
                        &self.tr(TextKey::Appearance),
                        &self.tr(TextKey::AppearanceSubtitle),
                    );
                    rounded(
                        dc,
                        self.box_at(24, 96, w - 48, 56),
                        self.palette.canvas,
                        self.palette.canvas,
                        self.s(12),
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::WindowTheme),
                        36,
                        110,
                        180,
                        self.palette.text,
                        0,
                    );
                    let columns = if w < NARROW_LAYOUT_BREAKPOINT_PX {
                        2
                    } else {
                        3
                    };
                    let preview_y = 168 + (6 / columns) * 152 + 8;
                    rounded(
                        dc,
                        self.box_at(24, preview_y, w - 48, 120),
                        self.palette.canvas,
                        self.palette.canvas,
                        self.s(12),
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::LivePreview),
                        36,
                        preview_y + 8,
                        160,
                        self.palette.text,
                        0,
                    );
                    let palette =
                        skin_palette(self.draft.appearance.skin as u8, self.draft.appearance);
                    self.candidate_preview(
                        dc,
                        self.box_at(36, preview_y + 36, w - 72, 76),
                        palette,
                        false,
                    );
                }
                PAGE_PRIVACY => {
                    self.heading(
                        dc,
                        &self.tr(TextKey::Privacy),
                        &self.tr(TextKey::PrivacySubtitle),
                    );
                    rounded(
                        dc,
                        self.box_at(24, 104, w - 48, 104),
                        self.palette.soft,
                        self.palette.soft,
                        self.s(12),
                    );
                    draw_svg_mask(
                        dc,
                        self.box_at(48, 128, 24, 24),
                        include_bytes!("../../../assets/ui/shield.mask"),
                        self.palette.muted,
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::LocalLearningTitle),
                        84,
                        124,
                        w - 132,
                        self.palette.text,
                        3,
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::LocalLearningHelp),
                        48,
                        160,
                        w - 96,
                        self.palette.muted,
                        1,
                    );
                    rounded(
                        dc,
                        self.box_at(24, 232, w - 48, 240),
                        self.palette.background,
                        self.palette.border,
                        self.s(12),
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::LearningPreferences),
                        48,
                        252,
                        w - 96,
                        self.palette.text,
                        3,
                    );
                    self.setting_row(
                        dc,
                        &self.tr(TextKey::Learning),
                        &self.tr(TextKey::LearningHelp),
                        308,
                    );
                    self.setting_row(
                        dc,
                        &self.tr(TextKey::PrivacyMode),
                        &self.tr(TextKey::PrivacyHelp),
                        388,
                    );
                    rounded(
                        dc,
                        self.box_at(24, 496, w - 48, 96),
                        self.palette.canvas,
                        self.palette.canvas,
                        self.s(12),
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::PrivacyCallout),
                        44,
                        508,
                        w - 88,
                        self.palette.text,
                        0,
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::PrivacyResumeNotice),
                        44,
                        546,
                        w - 88,
                        self.palette.muted,
                        1,
                    );
                }
                _ => {
                    self.heading(
                        dc,
                        &self.tr(TextKey::CustomSkin),
                        &self.tr(TextKey::CustomSkinSubtitle),
                    );
                    rounded(
                        dc,
                        self.box_at(24, 140, 284, 350),
                        self.palette.canvas,
                        self.palette.canvas,
                        self.s(12),
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::MySkin),
                        44,
                        150,
                        240,
                        self.palette.text,
                        3,
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::CornerRadius),
                        44,
                        330,
                        100,
                        self.palette.text,
                        0,
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::CandidateFontSize),
                        44,
                        378,
                        110,
                        self.palette.text,
                        0,
                    );
                    let narrow = w < NARROW_LAYOUT_BREAKPOINT_PX;
                    let x = if narrow { 24 } else { 332 };
                    let y = if narrow { 504 } else { 140 };
                    let width = if narrow { w - 48 } else { w - 356 };
                    rounded(
                        dc,
                        self.box_at(x, y, width, 360),
                        self.palette.soft,
                        self.palette.soft,
                        self.s(12),
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::MySkinPreview),
                        x + 16,
                        y + 20,
                        width - 32,
                        self.palette.text,
                        0,
                    );
                    self.candidate_preview(
                        dc,
                        self.box_at(x + 16, y + 64, width - 32, 238),
                        skin_palette(6, self.draft.appearance),
                        self.draft.appearance.vertical,
                    );
                    self.label(
                        dc,
                        &self.tr(TextKey::SkinPreviewHelp),
                        x + 16,
                        y + 320,
                        width - 32,
                        self.palette.muted,
                        1,
                    );
                }
            }
            RestoreDC(dc, saved);
        }
    }
    unsafe fn draw_button(&self, item: &DRAWITEMSTRUCT) {
        let id = item.CtlID as usize;
        let dc = item.hDC;
        let bounds = item.rcItem;
        let width = (bounds.right - bounds.left) * BASE_DPI_I32 / self.dpi;
        let height = (bounds.bottom - bounds.top) * BASE_DPI_I32 / self.dpi;
        let pressed = item.itemState & ODS_SELECTED != 0;
        let disabled = item.itemState & ODS_DISABLED != 0;
        let selected = (ID_NAV_GENERAL..=ID_NAV_PRIVACY).contains(&id)
            && (if self.page == PAGE_CUSTOM_SKIN {
                PAGE_APPEARANCE
            } else {
                self.page
            }) == NAVIGATION_PAGES[id - ID_NAV_GENERAL];
        let theme_selected = (ID_THEME_SYSTEM..=ID_THEME_DARK).contains(&id)
            && self.draft.appearance.window_theme as usize == id - ID_THEME_SYSTEM;
        let primary = id == ID_SAVE;
        unsafe {
            if [ID_SCHEME, ID_CHARACTER, ID_RADIUS, ID_FONT, ID_LAYOUT].contains(&id) {
                let index = if item.itemID == u32::MAX {
                    SendMessageW(self.control(id), CB_GETCURSEL, 0, 0) as u32
                } else {
                    item.itemID
                };
                let text = match id {
                    ID_SCHEME => SCHEME_LABELS
                        .get(index as usize)
                        .map(|key| self.tr(*key).to_string())
                        .unwrap_or_default(),
                    ID_CHARACTER => CHARACTER_LABELS
                        .get(index as usize)
                        .map(|key| self.tr(*key).to_string())
                        .unwrap_or_default(),
                    ID_RADIUS => format!("{} px", index + u32::from(MIN_CANDIDATE_RADIUS_PX)),
                    ID_FONT => format!("{} px", index + u32::from(MIN_CANDIDATE_FONT_SIZE_PX)),
                    ID_LAYOUT => LAYOUT_LABELS
                        .get(index as usize)
                        .map(|key| self.tr(*key).to_string())
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                fill(
                    dc,
                    bounds,
                    if pressed {
                        self.palette.accent
                    } else {
                        self.palette.background
                    },
                );
                self.text(
                    dc,
                    &text,
                    RECT {
                        left: bounds.left + self.s(10),
                        ..bounds
                    },
                    if pressed {
                        self.palette.on_accent
                    } else {
                        self.palette.text
                    },
                    0,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
                );
            } else if (ID_SKIN_FIRST..=ID_SKIN_CUSTOM).contains(&id) {
                let index = (id - ID_SKIN_FIRST + CandidateSkin::Paper as usize) as u8;
                let palette = skin_palette(index, self.draft.appearance);
                let border = if self.draft.appearance.skin as u8 == index {
                    self.palette.accent
                } else {
                    self.palette.border
                };
                rounded(dc, bounds, self.palette.background, border, self.s(12));
                let titles = [
                    &self.tr(TextKey::PaperSkin),
                    &self.tr(TextKey::MidnightSkin),
                    &self.tr(TextKey::MintSkin),
                    &self.tr(TextKey::DuskSkin),
                    &self.tr(TextKey::BambooSkin),
                    &self.tr(TextKey::CustomSkin),
                ];
                let descriptions = [
                    &self.tr(TextKey::PaperDescription),
                    &self.tr(TextKey::MidnightDescription),
                    &self.tr(TextKey::MintDescription),
                    &self.tr(TextKey::DuskDescription),
                    &self.tr(TextKey::BambooDescription),
                    &self.tr(TextKey::CustomDescription),
                ];
                if id != ID_SKIN_CUSTOM {
                    self.candidate_preview(dc, self.box_at(12, 12, width - 24, 78), palette, false);
                    self.label(
                        dc,
                        titles[id - ID_SKIN_FIRST],
                        12,
                        100,
                        width / 2,
                        self.palette.text,
                        0,
                    );
                    self.label(
                        dc,
                        descriptions[id - ID_SKIN_FIRST],
                        width / 2,
                        100,
                        width / 2 - 12,
                        self.palette.muted,
                        1,
                    );
                } else {
                    draw_svg_mask(
                        dc,
                        self.box_at(width / 2 - 12, 20, 24, 24),
                        include_bytes!("../../../assets/ui/add.mask"),
                        self.palette.muted,
                    );
                    self.text(
                        dc,
                        titles[5],
                        self.box_at(12, 62, width - 24, 28),
                        self.palette.text,
                        0,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                    self.text(
                        dc,
                        descriptions[5],
                        self.box_at(12, 94, width - 24, 28),
                        self.palette.muted,
                        1,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                }
            } else if [ID_COMMENTS, ID_EMOJI, ID_LEARNING, ID_PRIVACY].contains(&id) {
                let enabled = match id {
                    ID_COMMENTS => self.draft.show_comments,
                    ID_EMOJI => self.draft.emoji,
                    ID_LEARNING => self.draft.local_learning,
                    _ => self.draft.privacy_mode,
                };
                fill(dc, bounds, self.palette.background);
                let track = self.box_at(2, 4, 44, 24);
                rounded(
                    dc,
                    track,
                    if enabled {
                        self.palette.accent
                    } else {
                        self.palette.border
                    },
                    if enabled {
                        self.palette.accent
                    } else {
                        self.palette.border
                    },
                    self.s(12),
                );
                let knob = self.box_at(if enabled { 24 } else { 4 }, 6, 20, 20);
                rounded(
                    dc,
                    knob,
                    if enabled {
                        self.palette.on_accent
                    } else {
                        self.palette.background
                    },
                    if enabled {
                        self.palette.on_accent
                    } else {
                        self.palette.background
                    },
                    self.s(10),
                );
            } else {
                let background = if primary {
                    self.palette.accent
                } else if selected || theme_selected || pressed {
                    self.palette.soft
                } else if (ID_NAV_GENERAL..=ID_NAV_PRIVACY).contains(&id) {
                    self.palette.canvas
                } else {
                    self.palette.background
                };
                rounded(
                    dc,
                    bounds,
                    background,
                    if primary {
                        self.palette.accent
                    } else if selected {
                        self.palette.soft
                    } else if (ID_NAV_GENERAL..=ID_NAV_PRIVACY).contains(&id) {
                        self.palette.canvas
                    } else {
                        self.palette.border
                    },
                    self.s(8),
                );
                let text = match id {
                    ID_NAV_GENERAL => self.render_text(
                        TextKey::GeneralNavigation,
                        &[("label", &self.tr(TextKey::General))],
                    ),
                    ID_NAV_APPEARANCE => self.render_text(
                        TextKey::AppearanceNavigation,
                        &[("label", &self.tr(TextKey::Appearance))],
                    ),
                    ID_NAV_PRIVACY => self.render_text(
                        TextKey::PrivacyNavigation,
                        &[("label", &self.tr(TextKey::Privacy))],
                    ),
                    ID_LANGUAGE => self.tr(TextKey::LanguageButton).to_string(),
                    ID_SAVE => self.tr(TextKey::Save).to_string(),
                    ID_RESTORE => self.tr(TextKey::Restore).to_string(),
                    ID_RELOAD => self.tr(TextKey::Reload).to_string(),
                    ID_THEME_SYSTEM => self.tr(TextKey::SystemTheme).to_string(),
                    ID_THEME_LIGHT => self.tr(TextKey::LightTheme).to_string(),
                    ID_THEME_DARK => self.tr(TextKey::DarkTheme).to_string(),
                    ID_BACKGROUND => self.render_text(
                        TextKey::BackgroundColorValue,
                        &[(
                            "color",
                            &format!("{:06X}", self.draft.appearance.background),
                        )],
                    ),
                    ID_ACCENT => self.render_text(
                        TextKey::AccentColorValue,
                        &[("color", &format!("{:06X}", self.draft.appearance.accent))],
                    ),
                    ID_TEXT => self.render_text(
                        TextKey::TextColorValue,
                        &[("color", &format!("{:06X}", self.draft.appearance.text))],
                    ),
                    ID_IMPORT => self.tr(TextKey::ImportSkin).to_string(),
                    ID_EXPORT => self.tr(TextKey::ExportSkin).to_string(),
                    ID_BACK => self.render_text(
                        TextKey::BackToSkinsNavigation,
                        &[("label", &self.tr(TextKey::BackToSkins))],
                    ),
                    _ => String::new(),
                };
                self.text(
                    dc,
                    &text,
                    self.box_at(8, 0, width - 16, height),
                    if disabled {
                        self.palette.muted
                    } else if primary {
                        self.palette.on_accent
                    } else if selected {
                        self.palette.accent
                    } else {
                        self.palette.text
                    },
                    0,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
                );
            }
            if item.itemState & ODS_FOCUS != 0 {
                let mut focus = bounds;
                InflateRect(&mut focus, -self.s(3), -self.s(3));
                DrawFocusRect(dc, &focus);
            }
        }
    }
}
fn notify_settings() {
    unsafe {
        let message = RegisterWindowMessageW(wide("Mo.Settings.Changed.v1").as_ptr());
        if message != 0 {
            PostMessageW(0xffffusize as HWND, message, 0, 0);
        }
    }
}
unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        window_inner(window, message, wparam, lparam)
    }))
    .unwrap_or_else(|_| {
        unsafe {
            PostQuitMessage(1);
        }
        0
    })
}
unsafe fn window_inner(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message == WM_NCCREATE {
        let creation = unsafe { &*(lparam as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, creation.lpCreateParams as isize);
        }
    }
    if message == WM_MEASUREITEM {
        let measure = unsafe { &mut *(lparam as *mut MEASUREITEMSTRUCT) };
        measure.itemHeight =
            unsafe { GetDpiForWindow(window) }.max(BASE_DPI) * COMBO_ITEM_HEIGHT_PX / BASE_DPI;
        return 1;
    }
    if UI_MUTATING.with(|flag| flag.get()) {
        if matches!(
            message,
            WM_GETMINMAXINFO
                | WM_SETTINGCHANGE
                | WM_SYSCOLORCHANGE
                | WM_THEMECHANGED
                | WM_DPICHANGED
        ) {
            if message != WM_GETMINMAXINFO {
                unsafe {
                    PostMessageW(window, WM_APP + 2, 0, 0);
                }
            }
            return unsafe { DefWindowProcW(window, message, wparam, lparam) };
        }
        if message == WM_SIZE {
            unsafe {
                PostMessageW(window, WM_APP + 1, 0, 0);
            }
            return 0;
        }
        if matches!(
            message,
            WM_PAINT
                | WM_ERASEBKGND
                | WM_DRAWITEM
                | WM_CTLCOLORSTATIC
                | WM_CTLCOLORBTN
                | WM_CTLCOLOREDIT
                | WM_CTLCOLORLISTBOX
                | WM_COMMAND
        ) {
            return unsafe { DefWindowProcW(window, message, wparam, lparam) };
        }
    }
    let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) as *mut AppState };
    if pointer.is_null() {
        return unsafe { DefWindowProcW(window, message, wparam, lparam) };
    }
    if matches!(
        message,
        WM_DRAWITEM
            | WM_CTLCOLORSTATIC
            | WM_CTLCOLORBTN
            | WM_CTLCOLOREDIT
            | WM_CTLCOLORLISTBOX
            | WM_PAINT
            | WM_ERASEBKGND
    ) {
        let state = unsafe { &*pointer };
        return match message {
            WM_DRAWITEM => {
                unsafe {
                    state.draw_button(&*(lparam as *const DRAWITEMSTRUCT));
                }
                1
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                unsafe {
                    SetTextColor(wparam as HDC, state.palette.text);
                    SetBkColor(wparam as HDC, state.palette.background);
                }
                state.background as isize
            }
            WM_PAINT => {
                let mut paint = unsafe { std::mem::zeroed() };
                let dc = unsafe { BeginPaint(window, &mut paint) };
                if !dc.is_null() {
                    unsafe {
                        state.paint_root(dc);
                    }
                }
                unsafe {
                    EndPaint(window, &paint);
                }
                0
            }
            WM_ERASEBKGND => 1,
            _ => unreachable!(),
        };
    }
    if !matches!(
        message,
        WM_CREATE
            | WM_SIZE
            | WM_APP_LAYOUT
            | WM_COMMAND
            | WM_MOUSEWHEEL
            | WM_VSCROLL
            | WM_DPICHANGED
            | WM_SETTINGCHANGE
            | WM_SYSCOLORCHANGE
            | WM_THEMECHANGED
            | WM_APP_RESTYLE
            | WM_GETMINMAXINFO
            | WM_DESTROY
    ) {
        return unsafe { DefWindowProcW(window, message, wparam, lparam) };
    }
    let _mutation = UiMutation::new(window);
    let state = unsafe { &mut *pointer };
    match message {
        WM_CREATE => {
            if unsafe { state.create(window) }.is_ok() {
                0
            } else {
                -1
            }
        }
        WM_SIZE | WM_APP_LAYOUT => {
            if !state.view.is_null() {
                unsafe {
                    state.layout();
                }
            }
            0
        }
        WM_COMMAND => {
            unsafe {
                state.command(
                    wparam & WORD_MASK,
                    ((wparam >> WORD_SHIFT_BITS) & WORD_MASK) as u16,
                );
            }
            0
        }
        WM_MOUSEWHEEL => {
            state.scroll -= ((wparam >> WORD_SHIFT_BITS) as u16 as i16 as i32 / WHEEL_DELTA as i32)
                * SCROLL_STEP_PX;
            unsafe {
                state.layout();
            }
            0
        }
        WM_VSCROLL => {
            let code = (wparam & WORD_MASK) as i32;
            state.scroll += match code {
                SB_LINEUP => -SCROLL_LINE_PX,
                SB_LINEDOWN => SCROLL_LINE_PX,
                SB_PAGEUP => -state.viewport_height,
                SB_PAGEDOWN => state.viewport_height,
                _ => 0,
            };
            if code == SB_THUMBTRACK || code == SB_THUMBPOSITION {
                let mut info: SCROLLINFO = unsafe { std::mem::zeroed() };
                info.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
                info.fMask = SIF_TRACKPOS;
                unsafe {
                    GetScrollInfo(state.view, SB_VERT, &mut info);
                }
                state.scroll = info.nTrackPos;
            }
            unsafe {
                state.layout();
            }
            0
        }
        WM_DPICHANGED => {
            state.dpi = (wparam & WORD_MASK) as i32;
            let suggested = unsafe { &*(lparam as *const RECT) };
            unsafe {
                state.refresh_style();
                SetWindowPos(
                    window,
                    null_mut(),
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                state.layout();
            }
            0
        }
        WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_THEMECHANGED | WM_APP_RESTYLE => {
            state.dpi = unsafe { GetDpiForWindow(window) }.max(BASE_DPI) as i32;
            unsafe {
                state.refresh_style();
                state.layout();
            }
            0
        }
        WM_GETMINMAXINFO => {
            let info = unsafe { &mut *(lparam as *mut MINMAXINFO) };
            let mut monitor: MONITORINFO = unsafe { std::mem::zeroed() };
            monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            unsafe {
                GetMonitorInfoW(
                    MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST),
                    &mut monitor,
                );
            }
            info.ptMinTrackSize.x = state
                .s(MIN_WINDOW_WIDTH_PX)
                .min(monitor.rcWork.right - monitor.rcWork.left);
            info.ptMinTrackSize.y = state
                .s(MIN_WINDOW_HEIGHT_PX)
                .min(monitor.rcWork.bottom - monitor.rcWork.top);
            0
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
unsafe extern "system" fn viewport_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        if message == WM_NCCREATE {
            let creation = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(window, GWLP_USERDATA, creation.lpCreateParams as isize);
        }
        let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut AppState;
        if pointer.is_null() {
            return DefWindowProcW(window, message, wparam, lparam);
        }
        if UI_MUTATING.with(|flag| flag.get()) && (message == WM_PAINT || message == WM_ERASEBKGND)
        {
            return DefWindowProcW(window, message, wparam, lparam);
        }
        match message {
            WM_COMMAND | WM_DRAWITEM | WM_MEASUREITEM | WM_MOUSEWHEEL | WM_VSCROLL
            | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                SendMessageW(GetParent(window), message, wparam, lparam)
            }
            WM_PAINT => {
                let mut paint = std::mem::zeroed();
                let dc = BeginPaint(window, &mut paint);
                if !dc.is_null() {
                    (&*pointer).paint_content(dc);
                }
                EndPaint(window, &paint);
                0
            }
            WM_ERASEBKGND => 1,
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }))
    .unwrap_or(0)
}
#[derive(Debug)]
struct StartupOptions {
    preview: Option<std::path::PathBuf>,
    locale: Option<UiLocale>,
}
fn parse_startup_options(
    arguments: impl IntoIterator<Item = std::ffi::OsString>,
) -> io::Result<StartupOptions> {
    let mut arguments = arguments.into_iter();
    let mut preview = None;
    let mut selected_locale = None;
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--preview-dir") if preview.is_none() => {
                preview =
                    Some(std::path::PathBuf::from(arguments.next().ok_or_else(
                        || io::Error::other("missing preview directory"),
                    )?));
            }
            Some("--language") if selected_locale.is_none() => {
                let value = arguments
                    .next()
                    .ok_or_else(|| io::Error::other("missing UI language"))?;
                selected_locale = Some(
                    value
                        .to_str()
                        .ok_or_else(|| io::Error::other("invalid UI language"))?
                        .parse::<UiLocale>()
                        .map_err(io::Error::other)?,
                );
            }
            _ => return Err(io::Error::other("invalid settings arguments")),
        }
    }
    Ok(StartupOptions {
        preview,
        locale: selected_locale,
    })
}

pub fn run() -> io::Result<()> {
    let StartupOptions {
        preview,
        locale: selected_locale,
    } = parse_startup_options(std::env::args_os().skip(1))?;
    if preview.as_ref().is_some_and(|path| !path.is_dir()) {
        return Err(io::Error::other("preview directory must exist"));
    }
    let root = if let Some(path) = &preview {
        path.join("synthetic-root")
    } else {
        mo_windows_platform::runtime_roots()?.local_app_data
    };
    let controller = SettingsController::open(root);
    let profile = controller
        .path()
        .parent()
        .ok_or_else(|| io::Error::other("missing profile directory"))?;
    let locale = selected_locale
        .or_else(|| UiLocale::from_profile(profile).ok().flatten())
        .unwrap_or_default();
    let catalog = UiTextCatalog::from_profile(locale, profile).map_err(io::Error::other)?;
    let mut state = Box::new(AppState::new(controller, catalog));
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(null());
        for (name, procedure) in [
            (
                CLASS,
                Some(window_proc as unsafe extern "system" fn(_, _, _, _) -> _),
            ),
            (
                VIEW_CLASS,
                Some(viewport_proc as unsafe extern "system" fn(_, _, _, _) -> _),
            ),
        ] {
            let class_name = wide(name);
            let class = WNDCLASSW {
                lpfnWndProc: procedure,
                hInstance: instance,
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hIcon: LoadIconW(instance, resource_id(ICON_RESOURCE_ID)),
                lpszClassName: class_name.as_ptr(),
                ..std::mem::zeroed()
            };
            if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                return Err(io::Error::last_os_error());
            }
        }
        let mut monitor: MONITORINFO = std::mem::zeroed();
        monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(
            MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY),
            &mut monitor,
        );
        let system_dpi = GetDpiForSystem().max(BASE_DPI) as i32;
        state.dpi = system_dpi;
        let width = (WINDOW_WIDTH_PX * system_dpi / BASE_DPI_I32)
            .min(monitor.rcWork.right - monitor.rcWork.left);
        let height = (WINDOW_HEIGHT_PX * system_dpi / BASE_DPI_I32)
            .min(monitor.rcWork.bottom - monitor.rcWork.top);
        let window = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            wide(CLASS).as_ptr(),
            wide(&state.tr(TextKey::WindowTitle)).as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            monitor.rcWork.left + (monitor.rcWork.right - monitor.rcWork.left - width) / 2,
            monitor.rcWork.top + (monitor.rcWork.bottom - monitor.rcWork.top - height) / 2,
            width,
            height,
            null_mut(),
            null_mut(),
            instance,
            (&mut *state as *mut AppState).cast(),
        );
        if window.is_null() {
            return Err(io::Error::last_os_error());
        }
        if let Some(path) = preview {
            for (page, name) in [
                (PAGE_GENERAL, "settings-general"),
                (PAGE_APPEARANCE, "settings-appearance"),
                (PAGE_PRIVACY, "settings-privacy"),
                (PAGE_CUSTOM_SKIN, "settings-custom"),
            ] {
                state.page = page;
                state.scroll = 0;
                if page == PAGE_APPEARANCE {
                    state.draft.appearance.skin = CandidateSkin::Mint;
                }
                state.sync_controls();
                state.layout();
                capture(&state, &path.join(format!("{name}.bmp")))?;
            }
            state.page = PAGE_APPEARANCE;
            state.draft.appearance.window_theme = Theme::Dark;
            state.refresh_style();
            state.layout();
            capture(&state, &path.join("settings-dark.bmp"))?;
            state.draft.appearance.window_theme = Theme::Light;
            state.refresh_style();
            SetWindowPos(
                window,
                null_mut(),
                0,
                0,
                MIN_WINDOW_WIDTH_PX,
                PREVIEW_SMALL_HEIGHT_PX,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            state.page = PAGE_GENERAL;
            state.scroll = PREVIEW_INITIAL_SCROLL_PX;
            state.layout();
            capture(&state, &path.join("settings-small.bmp"))?;
            let original_locale = state.catalog.locale();
            state.command(ID_LANGUAGE, BN_CLICKED as u16);
            if state.catalog.locale() != original_locale.next() {
                return Err(io::Error::other("preview language switch failed"));
            }
            let mut native_caption = [0_u16; mo_settings_app::text_catalog::MAX_TEXT_BYTES + 1];
            let length = GetWindowTextW(
                state.control(ID_LANGUAGE),
                native_caption.as_mut_ptr(),
                native_caption.len() as i32,
            );
            let caption =
                String::from_utf16(&native_caption[..length as usize]).map_err(io::Error::other)?;
            if caption != state.tr(TextKey::LanguageButton).as_ref() {
                return Err(io::Error::other(
                    "preview language accessibility caption did not update",
                ));
            }
            capture(&state, &path.join("settings-language-switch.bmp"))?;
            std::fs::write(
                path.join("language-switch-state.txt"),
                format!(
                    "before={}\nafter={}\ncaption_updated=true\n",
                    original_locale.as_str(),
                    state.catalog.locale().as_str()
                ),
            )?;
            DestroyWindow(window);
            return Ok(());
        }
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        let mut message: MSG = std::mem::zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result == -1 {
                return Err(io::Error::last_os_error());
            }
            if result == 0 {
                if message.wParam != 0 {
                    return Err(io::Error::other("native settings callback failed"));
                }
                break;
            }
            if IsDialogMessageW(window, &message) != 0 {
                let focus = GetFocus();
                if state
                    .controls
                    .iter()
                    .any(|control| control.handle == focus && control.page == state.page)
                {
                    let mut bounds: RECT = std::mem::zeroed();
                    GetWindowRect(focus, &mut bounds);
                    MapWindowPoints(
                        null_mut(),
                        state.view,
                        (&mut bounds as *mut RECT).cast::<POINT>(),
                        2,
                    );
                    let top = bounds.top * BASE_DPI_I32 / state.dpi;
                    let bottom = bounds.bottom * BASE_DPI_I32 / state.dpi;
                    if top < 0 {
                        state.scroll += top - KEYBOARD_FOCUS_MARGIN_PX;
                        state.layout();
                    } else if bottom > state.viewport_height {
                        state.scroll += bottom - state.viewport_height + KEYBOARD_FOCUS_MARGIN_PX;
                        state.layout();
                    }
                }
                continue;
            }
            {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    Ok(())
}
unsafe fn capture(state: &AppState, path: &std::path::Path) -> io::Result<()> {
    let _mutation = UiMutation::new(state.window);
    let mut client: RECT = unsafe { std::mem::zeroed() };
    unsafe {
        GetClientRect(state.window, &mut client);
    }
    let width = client.right;
    let height = client.bottom;
    let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = BMP_BITS_PER_PIXEL;
    info.bmiHeader.biCompression = BI_RGB;
    let mut pixels = null_mut();
    let dc = unsafe { CreateCompatibleDC(null_mut()) };
    let bitmap = unsafe { CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0) };
    if dc.is_null() || bitmap.is_null() || pixels.is_null() {
        if !bitmap.is_null() {
            unsafe {
                DeleteObject(bitmap);
            }
        }
        if !dc.is_null() {
            unsafe {
                DeleteDC(dc);
            }
        }
        return Err(io::Error::last_os_error());
    }
    unsafe {
        let old = SelectObject(dc, bitmap);
        state.paint_root(dc);
        let saved = SaveDC(dc);
        SetViewportOrgEx(dc, state.s(SIDEBAR_WIDTH_PX), 0, null_mut());
        IntersectClipRect(
            dc,
            0,
            0,
            state.s(state.content_width),
            state.s(state.viewport_height),
        );
        state.paint_content(dc);
        RestoreDC(dc, saved);
        for control in &state.controls {
            if control.page != PAGE_GLOBAL && control.page != state.page {
                continue;
            }
            let mut bounds: RECT = std::mem::zeroed();
            GetClientRect(control.handle, &mut bounds);
            let mut origin = POINT { x: 0, y: 0 };
            MapWindowPoints(control.handle, state.window, &mut origin, 1);
            let saved = SaveDC(dc);
            if control.page != PAGE_GLOBAL {
                IntersectClipRect(
                    dc,
                    state.s(SIDEBAR_WIDTH_PX),
                    0,
                    width,
                    state.s(state.viewport_height),
                );
            }
            SetViewportOrgEx(dc, origin.x, origin.y, null_mut());
            if [ID_SCHEME, ID_CHARACTER, ID_RADIUS, ID_FONT, ID_LAYOUT].contains(&control.id) {
                SendMessageW(
                    control.handle,
                    WM_PRINT,
                    dc as usize,
                    (PRF_CLIENT | PRF_CHILDREN | PRF_ERASEBKGND) as isize,
                );
                // Hidden owner-drawn controls do not reliably send WM_DRAWITEM
                // during WM_PRINT. Use the same renderer for the selected text.
                let selected = SendMessageW(control.handle, CB_GETCURSEL, 0, 0);
                if selected < 0 {
                    return Err(io::Error::other("preview combo has no selection"));
                }
                let item = DRAWITEMSTRUCT {
                    CtlID: control.id as u32,
                    itemID: selected as u32,
                    hDC: dc,
                    rcItem: RECT {
                        left: state.s(1),
                        top: state.s(1),
                        right: bounds.right - state.s(24),
                        bottom: bounds.bottom - state.s(1),
                    },
                    ..std::mem::zeroed()
                };
                state.draw_button(&item);
            } else {
                let item = DRAWITEMSTRUCT {
                    CtlID: control.id as u32,
                    hDC: dc,
                    rcItem: bounds,
                    ..std::mem::zeroed()
                };
                state.draw_button(&item);
            }
            RestoreDC(dc, saved);
        }
        GdiFlush();
        let data = std::slice::from_raw_parts(
            pixels as *const u8,
            (width * height) as usize * BMP_BYTES_PER_PIXEL,
        );
        let mut output = Vec::with_capacity(data.len() + BMP_PIXEL_OFFSET);
        output.extend(b"BM");
        output.extend(((data.len() + BMP_PIXEL_OFFSET) as u32).to_le_bytes());
        output.extend([0; BMP_RESERVED_BYTES]);
        output.extend((BMP_PIXEL_OFFSET as u32).to_le_bytes());
        output.extend((BMP_INFO_HEADER_BYTES as u32).to_le_bytes());
        output.extend(width.to_le_bytes());
        output.extend((-height).to_le_bytes());
        output.extend(1u16.to_le_bytes());
        output.extend(BMP_BITS_PER_PIXEL.to_le_bytes());
        output.extend([0; BMP_INFO_UNUSED_BYTES]);
        output.extend(data);
        let result = std::fs::write(path, output);
        SelectObject(dc, old);
        DeleteObject(bitmap);
        DeleteDC(dc);
        result
    }
}
pub fn show_fatal_error(error: &io::Error) {
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(&error.to_string()).as_ptr(),
            wide(
                &UiTextCatalog::bundled(UiLocale::default())
                    .map(|catalog| catalog.get(TextKey::FatalStartupTitle))
                    .unwrap_or_else(|_| Arc::from(FATAL_TITLE_FALLBACK)),
            )
            .as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
fn system_dark() -> bool {
    let mut value = 1u32;
    let mut bytes = 4u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize").as_ptr(),
            wide("AppsUseLightTheme").as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            (&mut value as *mut u32).cast(),
            &mut bytes,
        ) == 0
            && value == 0
    }
}
unsafe fn draw_svg_mask(dc: HDC, slot: RECT, bytes: &[u8], tint: u32) {
    if bytes.is_empty() {
        return;
    }
    let mut offset = 1usize;
    let mut alpha = &[][..];
    let mut size = 0i32;
    for _ in 0..bytes[0] {
        let Some(header) = bytes.get(offset..offset + 6) else {
            return;
        };
        size = u16::from_le_bytes([header[0], header[1]]) as i32;
        let length = u32::from_le_bytes([header[2], header[3], header[4], header[5]]) as usize;
        offset += 6;
        if size <= 0 || size > 80 || length != (size * size) as usize {
            return;
        }
        let Some(data) = bytes.get(offset..offset + length) else {
            return;
        };
        alpha = data;
        offset += length;
        if size >= slot.right - slot.left {
            break;
        }
    }
    if alpha.is_empty() {
        return;
    }
    let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = size;
    info.bmiHeader.biHeight = -size;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = BMP_BITS_PER_PIXEL;
    info.bmiHeader.biCompression = BI_RGB;
    let source = unsafe { CreateCompatibleDC(dc) };
    let mut pixels = null_mut();
    let bitmap = unsafe { CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0) };
    if source.is_null() || bitmap.is_null() || pixels.is_null() {
        if !source.is_null() {
            unsafe {
                DeleteDC(source);
            }
        }
        if !bitmap.is_null() {
            unsafe {
                DeleteObject(bitmap);
            }
        }
        return;
    }
    unsafe {
        let output = std::slice::from_raw_parts_mut(pixels as *mut u8, alpha.len() * 4);
        for (i, alpha) in alpha.iter().enumerate() {
            output[i * 4] = (((tint >> 16) & 255) * *alpha as u32 / 255) as u8;
            output[i * 4 + 1] = (((tint >> 8) & 255) * *alpha as u32 / 255) as u8;
            output[i * 4 + 2] = ((tint & 255) * *alpha as u32 / 255) as u8;
            output[i * 4 + 3] = *alpha;
        }
        let old = SelectObject(source, bitmap);
        AlphaBlend(
            dc,
            slot.left,
            slot.top,
            slot.right - slot.left,
            slot.bottom - slot.top,
            source,
            0,
            0,
            size,
            size,
            BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            },
        );
        SelectObject(source, old);
        DeleteObject(bitmap);
        DeleteDC(source);
    }
}

#[cfg(test)]
mod startup_tests {
    use super::*;
    fn parse(values: &[&str]) -> io::Result<StartupOptions> {
        parse_startup_options(values.iter().map(std::ffi::OsString::from))
    }
    #[test]
    fn language_and_preview_options_are_independent() {
        let options =
            parse(&["--language", "en-US", "--preview-dir", "synthetic preview"]).unwrap();
        assert_eq!(options.locale, Some(UiLocale::EnUs));
        assert_eq!(
            options.preview,
            Some(std::path::PathBuf::from("synthetic preview"))
        );
        assert!(parse(&[]).unwrap().locale.is_none());
    }
    #[test]
    fn malformed_or_duplicate_options_fail_before_window_creation() {
        for arguments in [
            vec!["--language"],
            vec!["--preview-dir"],
            vec!["--language", "../en-US"],
            vec!["--language", "zh-CN", "--language", "en-US"],
            vec!["--preview-dir", "a", "--preview-dir", "b"],
            vec!["--unknown"],
        ] {
            assert!(parse(&arguments).is_err());
        }
    }
}
