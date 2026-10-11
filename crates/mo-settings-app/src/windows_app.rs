use mo_settings::{Appearance, CandidateSkin, CharacterSet, InputScheme, Settings, Theme};
use mo_settings_app::SettingsController;
use std::ffi::c_void;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::{null, null_mut};
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

#[derive(Clone, Copy)]
struct Control {
    handle: HWND,
    page: i32,
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
fn rgb(value: u32) -> u32 {
    ((value >> 16) & 255) | (value & 0xff00) | ((value & 255) << 16)
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
    controller: SettingsController,
    draft: Settings,
    window: HWND,
    view: HWND,
    controls: Vec<Control>,
    fonts: [HFONT; 5],
    background: HBRUSH,
    dpi: i32,
    page: i32,
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
    fn new(controller: SettingsController) -> Self {
        let status = if controller.can_save_changes() {
            "更改后请保存"
        } else {
            "设置无法读取，请重新读取或明确恢复默认"
        }
        .to_owned();
        Self {
            draft: controller.settings().clone(),
            controller,
            window: null_mut(),
            view: null_mut(),
            controls: Vec::new(),
            fonts: [null_mut(); 5],
            background: null_mut(),
            dpi: 96,
            page: 0,
            scroll: 0,
            content_width: 888,
            viewport_height: 644,
            status,
            palette: paper(),
        }
    }
    fn s(&self, value: i32) -> i32 {
        value * self.dpi / 96
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
            for (index, size) in [14, 12, 24, 16, self.draft.appearance.font_size as i32]
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
                SendMessageW(control.handle, WM_SETFONT, self.fonts[0] as usize, 1);
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
        self.dpi = unsafe { GetDpiForWindow(window) }.max(96) as i32;
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
            (200, "常用设置"),
            (201, "外观与皮肤"),
            (202, "隐私与学习"),
            (ID_SAVE, "保存更改"),
            (ID_RESTORE, "恢复默认"),
            (ID_RELOAD, "重新读取"),
        ] {
            unsafe {
                self.add(window, -1, id, "BUTTON", label, BS_OWNERDRAW as u32)?;
            }
        }
        unsafe {
            self.add(
                self.view,
                0,
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
                0,
                ID_CHARACTER,
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST as u32
                    | CBS_OWNERDRAWFIXED as u32
                    | CBS_HASSTRINGS as u32
                    | WS_VSCROLL,
            )?;
        }
        for label in ["全拼", "自然码双拼", "小鹤双拼", "微软双拼", "搜狗双拼"] {
            unsafe {
                SendMessageW(
                    self.control(ID_SCHEME),
                    CB_ADDSTRING,
                    0,
                    wide(label).as_ptr() as isize,
                );
            }
        }
        for label in ["简体中文", "繁体中文"] {
            unsafe {
                SendMessageW(
                    self.control(ID_CHARACTER),
                    CB_ADDSTRING,
                    0,
                    wide(label).as_ptr() as isize,
                );
            }
        }
        for (page, id, label) in [
            (0, ID_COMMENTS, "候选注释"),
            (0, ID_EMOJI, "Emoji 候选"),
            (2, ID_LEARNING, "本地学习"),
            (2, ID_PRIVACY, "隐私模式"),
        ] {
            unsafe {
                self.add(self.view, page, id, "BUTTON", label, BS_OWNERDRAW as u32)?;
            }
        }
        for (index, label) in ["跟随系统", "浅色", "深色"].into_iter().enumerate() {
            unsafe {
                self.add(
                    self.view,
                    1,
                    210 + index,
                    "BUTTON",
                    label,
                    BS_OWNERDRAW as u32,
                )?;
            }
        }
        for (index, label) in ["纸白", "夜墨", "薄荷", "暮霞", "墨竹", "自定义皮肤"]
            .into_iter()
            .enumerate()
        {
            unsafe {
                self.add(
                    self.view,
                    1,
                    220 + index,
                    "BUTTON",
                    label,
                    BS_OWNERDRAW as u32,
                )?;
            }
        }
        for (id, label) in [
            (ID_BACKGROUND, "背景颜色"),
            (ID_ACCENT, "强调颜色"),
            (ID_TEXT, "文字颜色"),
            (ID_IMPORT, "导入本地皮肤"),
            (ID_EXPORT, "导出当前皮肤"),
            (ID_BACK, "返回皮肤"),
        ] {
            unsafe {
                self.add(self.view, 3, id, "BUTTON", label, BS_OWNERDRAW as u32)?;
            }
        }
        for id in [ID_RADIUS, ID_FONT, ID_LAYOUT] {
            unsafe {
                self.add(
                    self.view,
                    3,
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
        for value in 8..=20 {
            unsafe {
                SendMessageW(
                    self.control(ID_RADIUS),
                    CB_ADDSTRING,
                    0,
                    wide(&format!("{value} px")).as_ptr() as isize,
                );
            }
        }
        for value in 12..=24 {
            unsafe {
                SendMessageW(
                    self.control(ID_FONT),
                    CB_ADDSTRING,
                    0,
                    wide(&format!("{value} px")).as_ptr() as isize,
                );
            }
        }
        for label in ["紧凑横排", "竖排候选"] {
            unsafe {
                SendMessageW(
                    self.control(ID_LAYOUT),
                    CB_ADDSTRING,
                    0,
                    wide(label).as_ptr() as isize,
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
        page: i32,
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
            SendMessageW(handle, WM_SETFONT, self.fonts[0] as usize, 0);
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
                (self.draft.appearance.radius - 8) as usize,
                0,
            );
            SendMessageW(
                self.control(ID_FONT),
                CB_SETCURSEL,
                (self.draft.appearance.font_size - 12) as usize,
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
            (ID_COMMENTS, "候选注释", self.draft.show_comments),
            (ID_EMOJI, "Emoji 候选", self.draft.emoji),
            (ID_LEARNING, "本地学习", self.draft.local_learning),
            (ID_PRIVACY, "隐私模式", self.draft.privacy_mode),
        ] {
            unsafe {
                SetWindowTextW(
                    self.control(id),
                    wide(&format!(
                        "{name}：{}",
                        if enabled { "已开启" } else { "已关闭" }
                    ))
                    .as_ptr(),
                );
            }
        }
    }
    fn content_height(&self) -> i32 {
        if self.page == 3 && self.content_width < 720 {
            return 1000;
        }
        if self.page == 1 && self.content_width < 720 {
            810
        } else {
            644
        }
    }
    unsafe fn layout(&mut self) {
        let _mutation = UiMutation::new(self.window);
        let mut client: RECT = unsafe { std::mem::zeroed() };
        unsafe {
            GetClientRect(self.window, &mut client);
        }
        let width = client.right * 96 / self.dpi;
        let height = client.bottom * 96 / self.dpi;
        self.content_width = (width - 212).max(400);
        self.viewport_height = (height - 72).max(120);
        self.scroll = self
            .scroll
            .clamp(0, (self.content_height() - self.viewport_height).max(0));
        unsafe {
            MoveWindow(
                self.view,
                self.s(212),
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
        let columns = if w < 720 { 2 } else { 3 };
        let card_width = (w - 48 - (columns - 1) * 16) / columns;
        for control in &self.controls {
            let id = control.id;
            let logical = match id {
                200..=202 => rect(16, 122 + (id as i32 - 200) * 52, 180, 44),
                ID_RESTORE => rect(236, height - 56, 110, 40),
                ID_RELOAD => rect(width - 276, height - 56, 120, 40),
                ID_SAVE => rect(width - 144, height - 56, 120, 40),
                ID_SCHEME => rect(w - 244, 156, 196, 200),
                ID_CHARACTER => rect(w - 244, 236, 196, 120),
                ID_COMMENTS => rect(w - 96, 410, 48, 32),
                ID_EMOJI => rect(w - 96, 490, 48, 32),
                ID_LEARNING => rect(w - 96, 310, 48, 32),
                ID_PRIVACY => rect(w - 96, 390, 48, 32),
                210..=212 => rect(w - 316 + (id as i32 - 210) * 96, 108, 92, 32),
                220..=225 => {
                    let index = id as i32 - 220;
                    rect(
                        24 + (index % columns) * (card_width + 16),
                        168 + (index / columns) * 152,
                        card_width,
                        136,
                    )
                }
                ID_BACK => rect(24, 88, 120, 32),
                ID_BACKGROUND => rect(44, 182, 240, 40),
                ID_ACCENT => rect(44, 230, 240, 40),
                ID_TEXT => rect(44, 278, 240, 40),
                ID_RADIUS => rect(160, 330, 124, 170),
                ID_FONT => rect(160, 378, 124, 170),
                ID_LAYOUT => rect(44, 430, 240, 100),
                ID_IMPORT => rect(24, if w < 720 { 904 } else { 552 }, 140, 40),
                ID_EXPORT => rect(180, if w < 720 { 904 } else { 552 }, 140, 40),
                _ => rect(0, 0, 1, 1),
            };
            let visible = control.page == -1 || control.page == self.page;
            unsafe {
                ShowWindow(control.handle, if visible { SW_SHOW } else { SW_HIDE });
                MoveWindow(
                    control.handle,
                    self.s(logical.left),
                    self.s(logical.top - if control.page >= 0 { self.scroll } else { 0 }),
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
            200..=202 => {
                self.page = id as i32 - 200;
                self.scroll = 0;
            }
            ID_BACK => {
                self.page = 1;
                self.scroll = 0;
            }
            ID_COMMENTS => self.draft.show_comments = !self.draft.show_comments,
            ID_EMOJI => self.draft.emoji = !self.draft.emoji,
            ID_LEARNING => self.draft.local_learning = !self.draft.local_learning,
            ID_PRIVACY => self.draft.privacy_mode = !self.draft.privacy_mode,
            210..=212 => {
                self.draft.appearance.window_theme =
                    [Theme::System, Theme::Light, Theme::Dark][id - 210];
                unsafe {
                    self.refresh_style();
                }
            }
            220..=224 => {
                self.draft.appearance.skin = CandidateSkin::from_index((id - 219) as u8).unwrap()
            }
            225 => {
                self.page = 3;
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
                if (0..=12).contains(&index) {
                    if id == ID_RADIUS {
                        self.draft.appearance.radius = index as u8 + 8;
                    } else {
                        self.draft.appearance.font_size = index as u8 + 12;
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
                    "设置已重新读取".into()
                } else {
                    "设置无法读取，请明确选择恢复默认".into()
                };
                unsafe {
                    self.refresh_style();
                }
            }
            ID_RESTORE => {
                if unsafe {
                    MessageBoxW(
                        self.window,
                        wide("恢复默认将覆盖当前设置，是否继续？").as_ptr(),
                        wide("恢复默认").as_ptr(),
                        MB_YESNO | MB_ICONQUESTION,
                    )
                } == IDYES
                {
                    match self.controller.restore_defaults() {
                        Ok(()) => {
                            self.draft = self.controller.settings().clone();
                            self.status = "已恢复默认设置".into();
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
                    self.status = "已保存 · 输入偏好在当前输入结束后生效".into();
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
        let filter = wide("Mo 皮肤 (*.mo-skin)\0*.mo-skin\0\0");
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
                file.take(1025).read_to_end(&mut bytes)?;
                let mut appearance = Appearance::decode_skin(&bytes).map_err(io::Error::other)?;
                appearance.window_theme = self.draft.appearance.window_theme;
                self.draft.appearance = appearance;
                Ok(())
            })
        };
        self.status = match result {
            Ok(()) => {
                if save {
                    "皮肤已导出".into()
                } else {
                    "皮肤已导入预览，保存后应用".into()
                }
            }
            Err(error) => format!("皮肤操作失败：{error}"),
        };
    }
}
fn paper() -> Palette {
    Palette {
        background: rgb(0xffffff),
        canvas: rgb(0xf4f6f5),
        text: rgb(0x25352f),
        muted: rgb(0x626f68),
        accent: rgb(0x176d5e),
        on_accent: rgb(0xffffff),
        border: rgb(0xe0e6e2),
        soft: rgb(0xeaf3ee),
    }
}
fn midnight() -> Palette {
    Palette {
        background: rgb(0x242b29),
        canvas: rgb(0x1d2321),
        text: rgb(0xeff5f1),
        muted: rgb(0xb0beb5),
        accent: rgb(0x9adac5),
        on_accent: rgb(0x15352c),
        border: rgb(0x4c5952),
        soft: rgb(0x35443d),
    }
}
fn skin_palette(index: u8, custom: Appearance) -> Palette {
    let (background, text, accent, muted, border) = match index {
        2 => return midnight(),
        3 => (0xf0f8f2, 0x213d2c, 0x286748, 0x526a59, 0xc7decf),
        4 => (0xfff4ef, 0x4a3029, 0x98503f, 0x795c50, 0xe8d2c7),
        5 => (0xf4f4ea, 0x303c2a, 0x526143, 0x60684f, 0xd4dac5),
        6 => (
            custom.background,
            custom.text,
            custom.accent,
            custom.text,
            custom.text,
        ),
        _ => return paper(),
    };
    let brightness =
        ((accent >> 16) & 255) * 299 + ((accent >> 8) & 255) * 587 + (accent & 255) * 114;
    Palette {
        background: rgb(background),
        canvas: rgb(background),
        text: rgb(text),
        muted: rgb(muted),
        accent: rgb(accent),
        on_accent: if brightness > 150000 {
            0
        } else {
            rgb(0xffffff)
        },
        border: rgb(border),
        soft: rgb(background),
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
        let height = client.bottom * 96 / self.dpi;
        unsafe {
            fill(dc, client, self.palette.background);
            fill(dc, self.box_at(0, 0, 212, height), self.palette.canvas);
            let icon = LoadImageW(
                GetModuleHandleW(null()),
                resource_id(1),
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
            self.label(dc, "让表达更自然", 20, 76, 180, self.palette.muted, 1);
            self.label(
                dc,
                "Mo 墨 · 开发版",
                20,
                height - 82,
                180,
                self.palette.muted,
                1,
            );
            self.label(
                dc,
                "轻盈表达 · 自在输入",
                20,
                height - 54,
                180,
                self.palette.muted,
                1,
            );
            fill(
                dc,
                self.box_at(212, height - 72, client.right * 96 / self.dpi - 212, 1),
                self.palette.border,
            );
            self.text(
                dc,
                &self.status,
                self.box_at(
                    352,
                    height - 60,
                    (client.right * 96 / self.dpi - 648).max(100),
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
        let width = (bounds.right - bounds.left) * 96 / self.dpi;
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
            self.label(dc, "中 · Mo", width - 68, 8, 56, palette.muted, 1);
            let count = if !vertical && width < 300 { 2 } else { 5 };
            for (index, text) in ["春风", "春峰", "淳风", "春丰", "春锋"]
                .into_iter()
                .take(count)
                .enumerate()
            {
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
                0 => {
                    self.heading(dc, "常用设置", "把常用选项放在手边，让每一次输入都顺手。");
                    rounded(
                        dc,
                        self.box_at(24, 104, w - 48, 216),
                        self.palette.background,
                        self.palette.border,
                        self.s(12),
                    );
                    self.label(dc, "输入习惯", 48, 120, w - 96, self.palette.text, 3);
                    self.setting_row(dc, "输入方案", "全拼或常用双拼方案", 156);
                    self.setting_row(dc, "输出文字", "按阅读习惯选择简体或繁体", 236);
                    rounded(
                        dc,
                        self.box_at(24, 344, w - 48, 232),
                        self.palette.background,
                        self.palette.border,
                        self.s(12),
                    );
                    self.label(dc, "候选与表达", 48, 360, w - 96, self.palette.text, 3);
                    self.setting_row(dc, "候选注释", "展开候选时显示拼音等辅助信息", 408);
                    self.setting_row(
                        dc,
                        "Emoji 候选",
                        "中文输入时联想表情；手动面板独立可用",
                        488,
                    );
                    self.label(
                        dc,
                        "输入方案将在当前输入结束后生效。",
                        24,
                        592,
                        w - 48,
                        self.palette.muted,
                        1,
                    );
                }
                1 => {
                    self.heading(dc, "外观与皮肤", "选一种喜欢的样子，输入依然轻盈。");
                    rounded(
                        dc,
                        self.box_at(24, 96, w - 48, 56),
                        self.palette.canvas,
                        self.palette.canvas,
                        self.s(12),
                    );
                    self.label(dc, "设置界面主题", 36, 110, 180, self.palette.text, 0);
                    let columns = if w < 720 { 2 } else { 3 };
                    let preview_y = 168 + (6 / columns) * 152 + 8;
                    rounded(
                        dc,
                        self.box_at(24, preview_y, w - 48, 120),
                        self.palette.canvas,
                        self.palette.canvas,
                        self.s(12),
                    );
                    self.label(dc, "实时预览", 36, preview_y + 8, 160, self.palette.text, 0);
                    let palette =
                        skin_palette(self.draft.appearance.skin as u8, self.draft.appearance);
                    self.candidate_preview(
                        dc,
                        self.box_at(36, preview_y + 36, w - 72, 76),
                        palette,
                        false,
                    );
                }
                2 => {
                    self.heading(dc, "隐私与学习", "你可以决定，输入习惯如何被记住。");
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
                    self.label(dc, "学习，留在本机", 84, 124, w - 132, self.palette.text, 3);
                    self.label(
                        dc,
                        "本地学习帮助候选排序；隐私模式会暂停学习。",
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
                    self.label(dc, "学习偏好", 48, 252, w - 96, self.palette.text, 3);
                    self.setting_row(dc, "本地学习", "允许 Mo 记住在本机的选词习惯", 308);
                    self.setting_row(dc, "隐私模式", "暂停学习，不改变本地学习开关的选择", 388);
                    rounded(
                        dc,
                        self.box_at(24, 496, w - 48, 96),
                        self.palette.canvas,
                        self.palette.canvas,
                        self.s(12),
                    );
                    self.label(
                        dc,
                        "暂时不想被记住？",
                        44,
                        508,
                        w - 88,
                        self.palette.text,
                        0,
                    );
                    self.label(
                        dc,
                        "关闭隐私模式后，会按原来的本地学习设置继续。",
                        44,
                        546,
                        w - 88,
                        self.palette.muted,
                        1,
                    );
                }
                _ => {
                    self.heading(dc, "自定义皮肤", "从喜欢的颜色开始，打造自己的输入气泡。");
                    rounded(
                        dc,
                        self.box_at(24, 140, 284, 350),
                        self.palette.canvas,
                        self.palette.canvas,
                        self.s(12),
                    );
                    self.label(dc, "我的皮肤", 44, 150, 240, self.palette.text, 3);
                    self.label(dc, "圆角", 44, 330, 100, self.palette.text, 0);
                    self.label(dc, "候选字号", 44, 378, 110, self.palette.text, 0);
                    let narrow = w < 720;
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
                        "我的皮肤 · 预览",
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
                        "装饰留在边缘，候选始终清晰。",
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
        let width = (bounds.right - bounds.left) * 96 / self.dpi;
        let height = (bounds.bottom - bounds.top) * 96 / self.dpi;
        let pressed = item.itemState & ODS_SELECTED != 0;
        let disabled = item.itemState & ODS_DISABLED != 0;
        let selected = (200..=202).contains(&id)
            && (if self.page == 3 { 1 } else { self.page }) == id as i32 - 200;
        let theme_selected =
            (210..=212).contains(&id) && self.draft.appearance.window_theme as usize == id - 210;
        let primary = id == ID_SAVE;
        unsafe {
            if [ID_SCHEME, ID_CHARACTER, ID_RADIUS, ID_FONT, ID_LAYOUT].contains(&id) {
                let index = if item.itemID == u32::MAX {
                    SendMessageW(self.control(id), CB_GETCURSEL, 0, 0) as u32
                } else {
                    item.itemID
                };
                let text = match id {
                    ID_SCHEME => ["全拼", "自然码双拼", "小鹤双拼", "微软双拼", "搜狗双拼"]
                        .get(index as usize)
                        .copied()
                        .unwrap_or("")
                        .into(),
                    ID_CHARACTER => ["简体中文", "繁体中文"]
                        .get(index as usize)
                        .copied()
                        .unwrap_or("")
                        .into(),
                    ID_RADIUS => format!("{} px", index + 8),
                    ID_FONT => format!("{} px", index + 12),
                    ID_LAYOUT => ["紧凑横排", "竖排候选"]
                        .get(index as usize)
                        .copied()
                        .unwrap_or("")
                        .into(),
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
            } else if (220..=225).contains(&id) {
                let index = (id - 219) as u8;
                let palette = skin_palette(index, self.draft.appearance);
                let border = if self.draft.appearance.skin as u8 == index {
                    self.palette.accent
                } else {
                    self.palette.border
                };
                rounded(dc, bounds, self.palette.background, border, self.s(12));
                let titles = ["纸白", "夜墨", "薄荷", "暮霞", "墨竹", "自定义皮肤"];
                let descriptions = [
                    "干净、轻盈",
                    "深色、专注",
                    "清爽、柔和",
                    "温暖、细腻",
                    "东方、留白",
                    "把喜欢的配色带进输入",
                ];
                if id != 225 {
                    self.candidate_preview(dc, self.box_at(12, 12, width - 24, 78), palette, false);
                    self.label(
                        dc,
                        titles[id - 220],
                        12,
                        100,
                        width / 2,
                        self.palette.text,
                        0,
                    );
                    self.label(
                        dc,
                        descriptions[id - 220],
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
                } else if (200..=202).contains(&id) {
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
                    } else if (200..=202).contains(&id) {
                        self.palette.canvas
                    } else {
                        self.palette.border
                    },
                    self.s(8),
                );
                let text = match id {
                    200 => "输   常用设置".into(),
                    201 => "色   外观与皮肤".into(),
                    202 => "隐   隐私与学习".into(),
                    ID_SAVE => "保存更改".into(),
                    ID_RESTORE => "恢复默认".into(),
                    ID_RELOAD => "重新读取".into(),
                    210 => "跟随系统".into(),
                    211 => "浅色".into(),
                    212 => "深色".into(),
                    ID_BACKGROUND => {
                        format!("背景颜色    #{:06X}", self.draft.appearance.background)
                    }
                    ID_ACCENT => format!("强调颜色    #{:06X}", self.draft.appearance.accent),
                    ID_TEXT => format!("文字颜色    #{:06X}", self.draft.appearance.text),
                    ID_IMPORT => "导入本地皮肤".into(),
                    ID_EXPORT => "导出当前皮肤".into(),
                    ID_BACK => "‹ 返回皮肤".into(),
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
        measure.itemHeight = unsafe { GetDpiForWindow(window) }.max(96) * 34 / 96;
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
                state.command(wparam & 0xffff, ((wparam >> 16) & 0xffff) as u16);
            }
            0
        }
        WM_MOUSEWHEEL => {
            state.scroll -= ((wparam >> 16) as u16 as i16 as i32 / 120) * 56;
            unsafe {
                state.layout();
            }
            0
        }
        WM_VSCROLL => {
            let code = (wparam & 0xffff) as i32;
            state.scroll += match code {
                SB_LINEUP => -32,
                SB_LINEDOWN => 32,
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
            state.dpi = (wparam & 0xffff) as i32;
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
            state.dpi = unsafe { GetDpiForWindow(window) }.max(96) as i32;
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
            info.ptMinTrackSize.x = state.s(900).min(monitor.rcWork.right - monitor.rcWork.left);
            info.ptMinTrackSize.y = state.s(520).min(monitor.rcWork.bottom - monitor.rcWork.top);
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
pub fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    let preview = if args.len() == 3 && args[1] == "--preview-dir" {
        Some(std::path::PathBuf::from(&args[2]))
    } else if args.len() == 1 {
        None
    } else {
        return Err(io::Error::other("invalid settings arguments"));
    };
    if preview.as_ref().is_some_and(|path| !path.is_dir()) {
        return Err(io::Error::other("preview directory must exist"));
    }
    let root = if let Some(path) = &preview {
        path.join("synthetic-root")
    } else {
        mo_windows_platform::runtime_roots()?.local_app_data
    };
    let mut state = Box::new(AppState::new(SettingsController::open(root)));
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
                hIcon: LoadIconW(instance, resource_id(1)),
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
        let system_dpi = GetDpiForSystem().max(96) as i32;
        state.dpi = system_dpi;
        let width = (1116 * system_dpi / 96).min(monitor.rcWork.right - monitor.rcWork.left);
        let height = (760 * system_dpi / 96).min(monitor.rcWork.bottom - monitor.rcWork.top);
        let window = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            wide(CLASS).as_ptr(),
            wide("Mo 墨 · 设置").as_ptr(),
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
                (0, "settings-general"),
                (1, "settings-appearance"),
                (2, "settings-privacy"),
                (3, "settings-custom"),
            ] {
                state.page = page;
                state.scroll = 0;
                if page == 1 {
                    state.draft.appearance.skin = CandidateSkin::Mint;
                }
                state.sync_controls();
                state.layout();
                capture(&state, &path.join(format!("{name}.bmp")))?;
            }
            state.page = 1;
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
                900,
                540,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            state.page = 0;
            state.scroll = 100;
            state.layout();
            capture(&state, &path.join("settings-small.bmp"))?;
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
                    let top = bounds.top * 96 / state.dpi;
                    let bottom = bounds.bottom * 96 / state.dpi;
                    if top < 0 {
                        state.scroll += top - 8;
                        state.layout();
                    } else if bottom > state.viewport_height {
                        state.scroll += bottom - state.viewport_height + 8;
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
    info.bmiHeader.biBitCount = 32;
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
        SetViewportOrgEx(dc, state.s(212), 0, null_mut());
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
            if control.page != -1 && control.page != state.page {
                continue;
            }
            let mut bounds: RECT = std::mem::zeroed();
            GetClientRect(control.handle, &mut bounds);
            let mut origin = POINT { x: 0, y: 0 };
            MapWindowPoints(control.handle, state.window, &mut origin, 1);
            let saved = SaveDC(dc);
            if control.page >= 0 {
                IntersectClipRect(dc, state.s(212), 0, width, state.s(state.viewport_height));
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
        let data = std::slice::from_raw_parts(pixels as *const u8, (width * height * 4) as usize);
        let mut output = Vec::with_capacity(data.len() + 54);
        output.extend(b"BM");
        output.extend(((data.len() + 54) as u32).to_le_bytes());
        output.extend([0; 4]);
        output.extend(54u32.to_le_bytes());
        output.extend(40u32.to_le_bytes());
        output.extend(width.to_le_bytes());
        output.extend((-height).to_le_bytes());
        output.extend(1u16.to_le_bytes());
        output.extend(32u16.to_le_bytes());
        output.extend([0; 24]);
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
            wide("Mo 设置无法启动").as_ptr(),
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
    info.bmiHeader.biBitCount = 32;
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
