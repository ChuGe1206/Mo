#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include "mo_settings_change_window.h"

namespace mo::windows_tip {
namespace {

constexpr wchar_t kWindowClassName[] = L"Mo.Settings.ChangeWindow.v1";

}  // namespace

SettingsChangeWindow::~SettingsChangeWindow() noexcept {
    Stop();
}

bool SettingsChangeWindow::Start(
    HINSTANCE module, Callback callback, void* context) noexcept {
    if (module == nullptr || callback == nullptr || window_ != nullptr) {
        return false;
    }
    const UINT message = RegisterWindowMessageW(kSettingsChangedMessageName);
    if (message == 0) {
        return false;
    }

    WNDCLASSEXW definition{};
    definition.cbSize = sizeof(definition);
    definition.lpfnWndProc = WindowProc;
    definition.hInstance = module;
    definition.lpszClassName = kWindowClassName;
    if (RegisterClassExW(&definition) == 0) {
        if (GetLastError() != ERROR_CLASS_ALREADY_EXISTS) {
            return false;
        }
        WNDCLASSEXW existing{};
        existing.cbSize = sizeof(existing);
        if (!GetClassInfoExW(module, kWindowClassName, &existing)
            || existing.lpfnWndProc != WindowProc) {
            return false;
        }
    }

    module_ = module;
    message_ = message;
    thread_id_ = GetCurrentThreadId();
    callback_ = callback;
    context_ = context;
    window_ = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        kWindowClassName,
        L"",
        WS_POPUP,
        0,
        0,
        0,
        0,
        nullptr,
        nullptr,
        module,
        this);
    if (window_ == nullptr) {
        module_ = nullptr;
        message_ = 0;
        thread_id_ = 0;
        callback_ = nullptr;
        context_ = nullptr;
        UnregisterClassW(kWindowClassName, module);
        return false;
    }
    return true;
}

void SettingsChangeWindow::Stop() noexcept {
    const HWND window = window_;
    const HINSTANCE module = module_;
    if (window != nullptr) {
        if (thread_id_ == GetCurrentThreadId()) {
            DestroyWindow(window);
        } else {
            // The Win32 window belongs to its creating thread. Wait for its
            // WM_CLOSE handler to finish before releasing the callback owner;
            // a timed-out send would leave GWLP_USERDATA pointing at freed
            // TextService memory.
            SendMessageW(window, WM_CLOSE, 0, 0);
        }
    }
    window_ = nullptr;
    module_ = nullptr;
    message_ = 0;
    thread_id_ = 0;
    callback_ = nullptr;
    context_ = nullptr;
    if (module != nullptr) {
        // This succeeds for the last receiver. If another TextService window
        // still exists, the class stays registered until that instance stops.
        UnregisterClassW(kWindowClassName, module);
    }
}

LRESULT CALLBACK SettingsChangeWindow::WindowProc(
    HWND window, UINT message, WPARAM word, LPARAM parameter) noexcept {
    auto* self = reinterpret_cast<SettingsChangeWindow*>(
        GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
        const auto* create = reinterpret_cast<const CREATESTRUCTW*>(parameter);
        self = static_cast<SettingsChangeWindow*>(create->lpCreateParams);
        SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self));
    }
    if (self != nullptr && message == self->message_) {
        const Callback callback = self->callback_;
        void* const context = self->context_;
        if (callback != nullptr) {
            callback(context);
        }
        return 0;
    }
    if (message == WM_NCDESTROY) {
        SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        if (self != nullptr && self->window_ == window) {
            self->window_ = nullptr;
        }
    }
    return DefWindowProcW(window, message, word, parameter);
}

bool BroadcastSettingsChanged() noexcept {
    const UINT message = RegisterWindowMessageW(kSettingsChangedMessageName);
    return message != 0 && PostMessageW(HWND_BROADCAST, message, 0, 0) != FALSE;
}

}  // namespace mo::windows_tip
