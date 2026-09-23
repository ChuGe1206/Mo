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
const ID_THEME: usize = 100;
const ID_SAVE: usize = 101;
const ID_RESTORE: usize = 102;
const ID_RELOAD: usize = 103;

struct AppState {
    controller: SettingsController,
    summary: HWND,
    theme: HWND,
    save: HWND,
    status: HWND,
}

impl AppState {
    fn new(controller: SettingsController) -> Self {
        Self {
            controller,
            summary: null_mut(),
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
                "当前输入配置",
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
                116,
                0,
            )?;
            create_control(
                window,
                "STATIC",
                "以上引擎选项正在接入，当前版本只读显示。",
                WS_CHILD | WS_VISIBLE,
                30,
                252,
                540,
                24,
                0,
            )?;
            create_control(
                window,
                "STATIC",
                "候选窗主题",
                WS_CHILD | WS_VISIBLE,
                28,
                298,
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
                294,
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
                "主题在 TIP 下次连接或显式刷新后生效。",
                WS_CHILD | WS_VISIBLE,
                30,
                336,
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
                382,
                540,
                48,
                0,
            )?;
            self.save = create_control(
                window,
                "BUTTON",
                "保存主题",
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
                28,
                452,
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
                452,
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
                452,
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
            "输入方案：{}\r\n字符模式：{}\r\n候选数量：{}\r\n注释：{}    Emoji：{}\r\n本地学习：{}    隐私模式：{}",
            scheme_label(settings.input_scheme),
            character_label(settings.character_set),
            settings.candidate_page_size,
            on_off(settings.show_comments),
            on_off(settings.emoji),
            on_off(settings.local_learning),
            on_off(settings.privacy_mode),
        );
        unsafe {
            set_text(self.summary, &summary);
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

    unsafe fn save_theme(&mut self) {
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
        match self.controller.save_theme(theme) {
            Ok(()) => unsafe {
                self.render();
                set_text(self.status, "主题已安全保存；下次连接候选窗时生效。");
            },
            Err(error) => unsafe { set_text(self.status, &error.to_string()) },
        }
    }

    unsafe fn restore_defaults(&mut self) {
        match self.controller.restore_defaults() {
            Ok(()) => unsafe {
                self.render();
                set_text(self.status, "已恢复并安全保存产品默认设置。");
            },
            Err(error) => unsafe { set_text(self.status, &error.to_string()) },
        }
    }

    unsafe fn reload(&mut self) {
        self.controller.reload();
        unsafe { self.render() };
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
            560,
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
                ID_SAVE => unsafe { (*state).save_theme() },
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

fn character_label(value: CharacterSet) -> &'static str {
    match value {
        CharacterSet::Simplified => "简体",
        CharacterSet::Traditional => "繁体",
    }
}

fn scheme_label(value: InputScheme) -> &'static str {
    match value {
        InputScheme::FullPinyin => "全拼",
        InputScheme::DoublePinyinNatural => "自然码双拼",
        InputScheme::DoublePinyinFlypy => "小鹤双拼",
        InputScheme::DoublePinyinMicrosoft => "微软双拼",
        InputScheme::DoublePinyinSogou => "搜狗双拼",
    }
}
