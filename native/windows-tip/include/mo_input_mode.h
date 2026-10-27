#pragma once
#include <windows.h>
#include <msctf.h>
#include <ctfutb.h>
#include <wrl/client.h>

namespace mo::windows_tip {

// Windows' documented GUID_LBI_INPUTMODE is not declared by every SDK header.
inline constexpr GUID kInputModeItemGuid = {
    0x2c77a81e, 0x41cc, 0x4178, {0xa3, 0xa7, 0x5f, 0x8a, 0x98, 0x75, 0x68, 0xe6},
};

enum class InputModeState { Unknown, Chinese, English, Disabled };
enum class InputModeText : UINT { Chinese = 400, English, Unavailable, Description };

// Each returned icon belongs to the caller (DestroyIcon). Monochrome masks let
// TSF tint the glyph for the taskbar theme and high-contrast settings.
HICON CreateInputModeIcon(InputModeState state, int width, int height) noexcept;

class InputModeButton final : public ITfLangBarItemButton, public ITfSource {
public:
    using ToggleCallback = void (*)(void*) noexcept;
    InputModeButton(HINSTANCE module, volatile LONG* live_objects,
        ToggleCallback callback, void* owner) noexcept;
    void Update(InputModeState state, bool can_toggle, bool visible) noexcept;
    void Detach() noexcept;
    STDMETHODIMP QueryInterface(REFIID iid, void** object) noexcept override;
    STDMETHODIMP_(ULONG) AddRef() noexcept override;
    STDMETHODIMP_(ULONG) Release() noexcept override;
    STDMETHODIMP GetInfo(TF_LANGBARITEMINFO* info) noexcept override;
    STDMETHODIMP GetStatus(DWORD* status) noexcept override;
    STDMETHODIMP Show(BOOL visible) noexcept override;
    STDMETHODIMP GetTooltipString(BSTR* text) noexcept override;
    STDMETHODIMP OnClick(TfLBIClick click, POINT, const RECT*) noexcept override;
    STDMETHODIMP InitMenu(ITfMenu*) noexcept override;
    STDMETHODIMP OnMenuSelect(UINT) noexcept override;
    STDMETHODIMP GetIcon(HICON* icon) noexcept override;
    STDMETHODIMP GetText(BSTR* text) noexcept override;
    STDMETHODIMP AdviseSink(REFIID iid, IUnknown* sink, DWORD* cookie) noexcept override;
    STDMETHODIMP UnadviseSink(DWORD cookie) noexcept override;
private:
    ~InputModeButton() noexcept;
    HRESULT CopyText(InputModeText key, BSTR* text) const noexcept;
    volatile LONG references_ = 1;
    HINSTANCE module_;
    volatile LONG* live_objects_;
    ToggleCallback callback_;
    void* owner_;
    InputModeState state_ = InputModeState::Unknown;
    bool can_toggle_ = false, visible_ = false, notification_pending_ = false;
    DWORD cookie_ = TF_INVALID_COOKIE, next_cookie_ = 1;
    Microsoft::WRL::ComPtr<ITfLangBarItemSink> sink_;
};
}
