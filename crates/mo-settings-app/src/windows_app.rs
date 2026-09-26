use std::ffi::c_void;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::{null, null_mut};

use mo_settings::{CharacterSet, InputScheme, Theme};
use mo_settings_app::{DocumentHealth, SettingsController};
use windows_sys::Win32::Foundation::{ERROR_CLASS_ALREADY_EXISTS, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    COLOR_WINDOW, DEFAULT_GUI_FONT, GetStockObject, HBRUSH, UpdateWindow,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const CLASS_NAME: &str = "Mo.Settings.Window.v1";
const SETTINGS_CHANGED_MESSAGE: &str = "Mo.Settings.Changed.v1";
const ID_THEME: usize = 100;
const ID_SAVE: usize = 101;
const ID_RESTORE: usize = 102;
const ID_RELOAD: usize = 103;
const ID_SCHEME: usize = 104;
const ID_CHARACTER_SET: usize = 105;

struct AppState {
    controller: SettingsController,
    summary: HWND,
    scheme: HWND,
    character_set: HWND,
    theme: HWND,
    save: HWND,
    status: HWND,
}

impl AppState {
    fn new(controller: SettingsController) -> Self {
        Self {
            controller,
            summary: null_mut(),
            scheme: null_mut(),
            character_set: null_mut(),
            theme: null_mut(),
            save: null_mut(),
            status: null_mut(),
        }
    }

    unsafe fn create_controls(&mut self, window: HWND) -> io::Result<()> {
        unsafe {
            create_control(
                window,
                "STATIC",
                "Mo 墨输入法 · 设置",
                WS_CHILD | WS_VISIBLE,
                28,
                24,
                520,
                34,
                0,
            )?;
            create_control(
                window,
                "STATIC",
                "简单、离线、无需编辑 Rime 配置文件",
                WS_CHILD | WS_VISIBLE,
                30,
                60,
                520,
                24,
                0,
            )?;
            create_control(
                window,
                "STATIC",
                "其他配置（暂未接入引擎）",
                WS_CHILD | WS_VISIBLE,
                28,
                104,
                520,
                24,
                0,
            )?;
            self.summary = create_control(
                window,
                "STATIC",
                "",
                WS_CHILD | WS_VISIBLE | WS_DISABLED,
                30,
                134,
                540,
                78,
                0,
            )?;
            create_control(
                window,
                "STATIC",
                "输入方案",
                WS_CHILD | WS_VISIBLE,
                28,
                238,
                150,
                24,
                0,
            )?;
            self.scheme = create_control(
                window,
                "COMBOBOX",
                "",
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
                178,
                234,
                250,
                180,
                ID_SCHEME,
            )?;
            for label in ["全拼", "自然码双拼", "小鹤双拼", "微软双拼", "搜狗双拼"]
            {
                let label = wide(label);
                SendMessageW(self.scheme, CB_ADDSTRING, 0, label.as_ptr() as LPARAM);
            }
            create_control(
                window,
                "STATIC",
                "字符模式",
                WS_CHILD | WS_VISIBLE,
                28,
                294,
                150,
                24,
                0,
            )?;
            self.character_set = create_control(
                window,
                "COMBOBOX",
                "",
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
                178,
                290,
                220,
                100,
                ID_CHARACTER_SET,
            )?;
            for label in ["简体", "繁体"] {
                let label = wide(label);
                SendMessageW(
                    self.character_set,
                    CB_ADDSTRING,
                    0,
                    label.as_ptr() as LPARAM,
                );
            }
            create_control(
                window,
                "STATIC",
                "候选窗主题",
                WS_CHILD | WS_VISIBLE,
                28,
                350,
                150,
                24,
                0,
            )?;
            self.theme = create_control(
                window,
                "COMBOBOX",
                "",
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
                178,
                346,
                220,
                160,
                ID_THEME,
            )?;
            for label in ["跟随系统", "浅色", "深色"] {
                let label = wide(label);
                SendMessageW(self.theme, CB_ADDSTRING, 0, label.as_ptr() as LPARAM);
            }
            create_control(
                window,
                "STATIC",
                "主题立即刷新；方案和简繁在输入法新建会话时生效。",
                WS_CHILD | WS_VISIBLE,
                30,
                394,
                540,
                24,
                0,
            )?;
            self.status = create_control(
                window,
                "STATIC",
                "",
                WS_CHILD | WS_VISIBLE,
                30,
                438,
                540,
                48,
                0,
            )?;
            self.save = create_control(
                window,
                "BUTTON",
                "保存设置",
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
                28,
                510,
                142,
                38,
                ID_SAVE,
            )?;
            create_control(
                window,
                "BUTTON",
                "重新读取",
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
                188,
                510,
                142,
                38,
                ID_RELOAD,
            )?;
            create_control(
                window,
                "BUTTON",
                "恢复默认设置",
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
                348,
                510,
                180,
                38,
                ID_RESTORE,
            )?;
            self.render();
        }
        Ok(())
    }

    unsafe fn render(&self) {
        let settings = self.controller.settings();
        let summary = format!(
            "候选数量：{}\r\n注释：{}    Emoji：{}\r\n本地学习：{}    隐私模式：{}",
            settings.candidate_page_size,
            on_off(settings.show_comments),
            on_off(settings.emoji),
            on_off(settings.local_learning),
            on_off(settings.privacy_mode),
        );
        unsafe {
            set_text(self.summary, &summary);
            SendMessageW(
                self.scheme,
                CB_SETCURSEL,
                scheme_index(settings.input_scheme),
                0,
            );
            SendMessageW(
                self.character_set,
                CB_SETCURSEL,
                character_index(settings.character_set),
                0,
            );
            SendMessageW(self.theme, CB_SETCURSEL, theme_index(settings.theme), 0);
            EnableWindow(self.save, i32::from(self.controller.can_save_changes()));
            match self.controller.health() {
                DocumentHealth::Ready if self.controller.stored() => {
                    set_text(
                        self.status,
                        "设置文件已加载。保存使用原子替换，不写入 YAML/Lua。",
                    );
                }
                DocumentHealth::Ready => {
                    set_text(
                        self.status,
                        "首次运行：正在使用产品默认设置，尚未创建用户文件。",
                    );
                }
                DocumentHealth::RecoveryRequired(error) => {
                    set_text(
                        self.status,
                        &format!(
                            "设置文件无法读取：{error}\r\n如需覆盖，请明确点击“恢复默认设置”。"
                        ),
                    );
                }
            }
        }
    }

    unsafe fn save_preferences(&mut self) {
        let scheme = match unsafe { SendMessageW(self.scheme, CB_GETCURSEL, 0, 0) } {
            0 => InputScheme::FullPinyin,
            1 => InputScheme::DoublePinyinNatural,
            2 => InputScheme::DoublePinyinFlypy,
            3 => InputScheme::DoublePinyinMicrosoft,
            4 => InputScheme::DoublePinyinSogou,
            _ => {
                unsafe { set_text(self.status, "请选择有效的输入方案。") };
                return;
            }
        };
        let character_set = match unsafe { SendMessageW(self.character_set, CB_GETCURSEL, 0, 0) } {
            0 => CharacterSet::Simplified,
            1 => CharacterSet::Traditional,
            _ => {
                unsafe { set_text(self.status, "请选择有效的字符模式。") };
                return;
            }
        };
        let index = unsafe { SendMessageW(self.theme, CB_GETCURSEL, 0, 0) };
        let theme = match index {
            0 => Theme::System,
            1 => Theme::Light,
            2 => Theme::Dark,
            _ => {
                unsafe { set_text(self.status, "请选择有效的候选窗主题。") };
                return;
            }
        };
        match self
            .controller
            .save_primary_preferences(scheme, character_set, theme)
        {
            Ok(()) => unsafe {
                self.render();
                if notify_settings_changed() {
                    set_text(
                        self.status,
                        "设置已安全保存；主题刷新通知已发出。方案和简繁在新会话生效。",
                    );
                } else {
                    set_text(
                        self.status,
                        "设置已安全保存；通知失败，主题将在下次连接时生效。",
                    );
                }
            },
            Err(error) => unsafe { set_text(self.status, &error.to_string()) },
        }
    }

    unsafe fn restore_defaults(&mut self) {
        match self.controller.restore_defaults() {
            Ok(()) => unsafe {
                self.render();
                if notify_settings_changed() {
                    set_text(
                        self.status,
                        "已恢复默认设置；主题刷新通知已发出，方案和简繁在新会话生效。",
                    );
                } else {
                    set_text(
                        self.status,
                        "已恢复并安全保存默认设置；通知失败，将在下次连接时生效。",
                    );
                }
            },
            Err(error) => unsafe { set_text(self.status, &error.to_string()) },
        }
    }

    unsafe fn reload(&mut self) {
        self.controller.reload();
        unsafe { self.render() };
    }
}

fn notify_settings_changed() -> bool {
    let name = wide(SETTINGS_CHANGED_MESSAGE);
    unsafe {
        let message = RegisterWindowMessageW(name.as_ptr());
        let broadcast = 0xffffusize as HWND;
        message != 0 && PostMessageW(broadcast, message, 0, 0) != 0
    }
}

pub fn run() -> io::Result<()> {
    let roots = mo_windows_platform::runtime_roots()?;
    let mut state = Box::new(AppState::new(SettingsController::open(
        roots.local_app_data,
    )));
    let class_name = wide(CLASS_NAME);
    let title = wide("Mo 墨输入法设置");
    unsafe {
        let instance = GetModuleHandleW(null());
        if instance.is_null() {
            return Err(io::Error::last_os_error());
        }
        let definition = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as usize as HBRUSH,
            lpszClassName: class_name.as_ptr(),
            ..std::mem::zeroed()
        };
        if RegisterClassW(&definition) == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_CLASS_ALREADY_EXISTS as i32) {
                return Err(error);
            }
        }
        let window = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            620,
            620,
            null_mut(),
            null_mut(),
            instance,
            (&mut *state as *mut AppState).cast::<c_void>(),
        );
        if window.is_null() {
            return Err(io::Error::last_os_error());
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
                break;
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

pub fn show_fatal_error(error: &io::Error) {
    let title = wide("Mo 设置无法启动");
    let message = wide(&error.to_string());
    unsafe {
        MessageBoxW(
            null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        window_proc_inner(window, message, wparam, lparam)
    }))
    .unwrap_or_else(|_| {
        unsafe { PostQuitMessage(1) };
        0
    })
}

unsafe fn window_proc_inner(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message == WM_NCCREATE {
        let creation = unsafe { &*(lparam as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, creation.lpCreateParams as isize);
        }
    }
    let state = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) as *mut AppState };
    match message {
        WM_CREATE if !state.is_null() => {
            if unsafe { (*state).create_controls(window) }.is_err() {
                return -1;
            }
            0
        }
        WM_COMMAND if !state.is_null() => {
            match wparam & 0xffff {
                ID_SAVE => unsafe { (*state).save_preferences() },
                ID_RESTORE => unsafe { (*state).restore_defaults() },
                ID_RELOAD => unsafe { (*state).reload() },
                _ => {}
            }
            0
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn create_control(
    parent: HWND,
    class: &str,
    text: &str,
    style: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    id: usize,
) -> io::Result<HWND> {
    let class = wide(class);
    let text = wide(text);
    let handle = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            text.as_ptr(),
            style,
            x,
            y,
            width,
            height,
            parent,
            id as *mut c_void,
            GetModuleHandleW(null()),
            null(),
        )
    };
    if handle.is_null() {
        Err(io::Error::last_os_error())
    } else {
        let font = unsafe { GetStockObject(DEFAULT_GUI_FONT) };
        if !font.is_null() {
            unsafe { SendMessageW(handle, WM_SETFONT, font as WPARAM, 1) };
        }
        Ok(handle)
    }
}

unsafe fn set_text(window: HWND, value: &str) {
    let value = wide(value);
    unsafe { SetWindowTextW(window, value.as_ptr()) };
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn theme_index(theme: Theme) -> usize {
    match theme {
        Theme::System => 0,
        Theme::Light => 1,
        Theme::Dark => 2,
    }
}

fn on_off(value: bool) -> &'static str {
    if value { "开" } else { "关" }
}

fn character_index(value: CharacterSet) -> usize {
    match value {
        CharacterSet::Simplified => 0,
        CharacterSet::Traditional => 1,
    }
}

fn scheme_index(value: InputScheme) -> usize {
    match value {
        InputScheme::FullPinyin => 0,
        InputScheme::DoublePinyinNatural => 1,
        InputScheme::DoublePinyinFlypy => 2,
        InputScheme::DoublePinyinMicrosoft => 3,
        InputScheme::DoublePinyinSogou => 4,
    }
}
