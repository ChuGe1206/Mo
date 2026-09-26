#pragma once

#include <windows.h>

namespace mo::windows_tip {

inline constexpr wchar_t kSettingsChangedMessageName[] = L"Mo.Settings.Changed.v1";

// Hidden top-level receiver for the best-effort settings invalidation signal.
// The message carries no settings data or authority: receivers always reload
// the typed snapshot from their already-authenticated Broker connection.
class SettingsChangeWindow final {
public:
    using Callback = void (*)(void*) noexcept;

    SettingsChangeWindow() noexcept = default;
    ~SettingsChangeWindow() noexcept;
    SettingsChangeWindow(const SettingsChangeWindow&) = delete;
    SettingsChangeWindow& operator=(const SettingsChangeWindow&) = delete;

    bool Start(HINSTANCE module, Callback callback, void* context) noexcept;
    void Stop() noexcept;
    bool active() const noexcept { return window_ != nullptr; }

private:
    static LRESULT CALLBACK WindowProc(HWND, UINT, WPARAM, LPARAM) noexcept;

    HINSTANCE module_ = nullptr;
    HWND window_ = nullptr;
    UINT message_ = 0;
    DWORD thread_id_ = 0;
    Callback callback_ = nullptr;
    void* context_ = nullptr;
};

// Broadcasts only an invalidation hint. Failure is non-fatal: a TIP that did
// not receive it loads the latest snapshot on its next Broker connection.
bool BroadcastSettingsChanged() noexcept;

}  // namespace mo::windows_tip
