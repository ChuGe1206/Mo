#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include "mo_input_mode.h"
#include "mo_tip_ids.h"
#include <olectl.h>
#include <limits>

namespace mo::windows_tip {
namespace {
constexpr int kMaximumModeIconSizePx = 128;
constexpr int kMaximumModeTextCharacters = 256;
constexpr int kModeGlyphHeightNumerator = 7;
constexpr int kModeGlyphHeightDenominator = 8;
constexpr wchar_t kModeGlyphFont[] = L"Microsoft YaHei UI";
constexpr wchar_t kChineseGlyph[] = L"中";
constexpr wchar_t kEnglishGlyph[] = L"英";
constexpr wchar_t kUnknownGlyph[] = L"?";
constexpr wchar_t kDisabledGlyph[] = L"–";
const wchar_t* Glyph(InputModeState state) noexcept {
    switch (state) {
    case InputModeState::Chinese: return kChineseGlyph;
    case InputModeState::English: return kEnglishGlyph;
    case InputModeState::Disabled: return kDisabledGlyph;
    default: return kUnknownGlyph;
    }
}
}

HICON CreateInputModeIcon(InputModeState state, int width, int height) noexcept {
    if (width <= 0 || height <= 0 || width > kMaximumModeIconSizePx || height > kMaximumModeIconSizePx) { return nullptr; }
    const HDC dc = CreateCompatibleDC(nullptr);
    const HBITMAP mask = CreateBitmap(width, height * 2, 1, 1, nullptr);
    const HFONT font = CreateFontW(-height * kModeGlyphHeightNumerator / kModeGlyphHeightDenominator,
        0, 0, 0, FW_SEMIBOLD, FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS, NONANTIALIASED_QUALITY, DEFAULT_PITCH, kModeGlyphFont);
    HICON result = nullptr;
    if (dc != nullptr && mask != nullptr && font != nullptr) {
        const HGDIOBJ old_bitmap = SelectObject(dc, mask), old_font = SelectObject(dc, font);
        // A monochrome icon stores AND above XOR: background (1,0) is transparent,
        // the glyph (0,1) is white, subsequently tinted by TF_LBI_STYLE_TEXTCOLORICON.
        RECT and_rect{0, 0, width, height}, xor_rect{0, height, width, height * 2};
        const bool cleared = PatBlt(dc, 0, 0, width, height, WHITENESS)
            && PatBlt(dc, 0, height, width, height, BLACKNESS);
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, RGB(0, 0, 0));
        const bool and_drawn = DrawTextW(dc, Glyph(state), -1, &and_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX) != 0;
        SetTextColor(dc, RGB(255, 255, 255));
        const bool xor_drawn = DrawTextW(dc, Glyph(state), -1, &xor_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX) != 0;
        SelectObject(dc, old_font); SelectObject(dc, old_bitmap);
        if (cleared && and_drawn && xor_drawn) {
            ICONINFO info{}; info.fIcon = TRUE; info.hbmMask = mask;
            result = CreateIconIndirect(&info);
        }
    }
    if (font != nullptr) { DeleteObject(font); }
    if (mask != nullptr) { DeleteObject(mask); }
    if (dc != nullptr) { DeleteDC(dc); }
    return result;
}

InputModeButton::InputModeButton(HINSTANCE module, volatile LONG* live_objects,
    ToggleCallback callback, void* owner) noexcept
    : module_(module), live_objects_(live_objects), callback_(callback), owner_(owner) {
    InterlockedIncrement(live_objects_);
}
InputModeButton::~InputModeButton() noexcept { InterlockedDecrement(live_objects_); }
STDMETHODIMP InputModeButton::QueryInterface(REFIID iid, void** object) noexcept {
    if (object == nullptr) { return E_POINTER; }
    *object = nullptr;
    if (iid == IID_IUnknown || iid == IID_ITfLangBarItem || iid == IID_ITfLangBarItemButton) {
        *object = static_cast<ITfLangBarItemButton*>(this);
    } else if (iid == IID_ITfSource) { *object = static_cast<ITfSource*>(this); }
    else { return E_NOINTERFACE; }
    AddRef(); return S_OK;
}
STDMETHODIMP_(ULONG) InputModeButton::AddRef() noexcept { return static_cast<ULONG>(InterlockedIncrement(&references_)); }
STDMETHODIMP_(ULONG) InputModeButton::Release() noexcept {
    const LONG count = InterlockedDecrement(&references_);
    if (count == 0) { delete this; return 0; }
    return static_cast<ULONG>(count);
}
void InputModeButton::Update(InputModeState state, bool can_toggle, bool visible) noexcept {
    if (state_ == state && can_toggle_ == can_toggle && visible_ == visible && !notification_pending_) { return; }
    state_ = state; can_toggle_ = can_toggle; visible_ = visible;
    // Pin both participants: OnUpdate may synchronously unadvise or deactivate.
    Microsoft::WRL::ComPtr<ITfLangBarItemButton> self(this);
    const auto sink = sink_;
    notification_pending_ = sink != nullptr && FAILED(sink->OnUpdate(TF_LBI_BTNALL | TF_LBI_STATUS));
}
void InputModeButton::Detach() noexcept {
    callback_ = nullptr; owner_ = nullptr;
    Update(InputModeState::Unknown, false, false);
    sink_.Reset(); cookie_ = TF_INVALID_COOKIE;
}
STDMETHODIMP InputModeButton::GetInfo(TF_LANGBARITEMINFO* info) noexcept {
    if (info == nullptr) { return E_POINTER; }
    *info = {}; info->clsidService = kTextServiceClsid; info->guidItem = kInputModeItemGuid;
    info->dwStyle = TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY | TF_LBI_STYLE_TEXTCOLORICON;
    BSTR description = nullptr;
    const HRESULT result = CopyText(InputModeText::Description, &description);
    if (FAILED(result)) { return result; }
    const errno_t copied = wcscpy_s(info->szDescription, description);
    SysFreeString(description);
    return copied == 0 ? S_OK : E_FAIL;
}
STDMETHODIMP InputModeButton::GetStatus(DWORD* status) noexcept {
    if (status == nullptr) { return E_POINTER; }
    *status = (visible_ ? 0 : TF_LBI_STATUS_HIDDEN) | (can_toggle_ ? 0 : TF_LBI_STATUS_DISABLED);
    return S_OK;
}
STDMETHODIMP InputModeButton::Show(BOOL visible) noexcept { Update(state_, can_toggle_, visible != FALSE); return S_OK; }
HRESULT InputModeButton::CopyText(InputModeText key, BSTR* text) const noexcept {
    if (text == nullptr) { return E_POINTER; }
    *text = nullptr;
    wchar_t buffer[kMaximumModeTextCharacters]{};
    if (LoadStringW(module_, static_cast<UINT>(key), buffer, ARRAYSIZE(buffer)) == 0) { return E_FAIL; }
    *text = SysAllocString(buffer);
    return *text != nullptr ? S_OK : E_OUTOFMEMORY;
}
STDMETHODIMP InputModeButton::GetTooltipString(BSTR* text) noexcept {
    return CopyText(state_ == InputModeState::Chinese ? InputModeText::Chinese
        : state_ == InputModeState::English ? InputModeText::English : InputModeText::Unavailable, text);
}
STDMETHODIMP InputModeButton::GetText(BSTR* text) noexcept {
    if (text == nullptr) { return E_POINTER; }
    *text = SysAllocString(Glyph(state_)); return *text != nullptr ? S_OK : E_OUTOFMEMORY;
}
STDMETHODIMP InputModeButton::GetIcon(HICON* icon) noexcept {
    if (icon == nullptr) { return E_POINTER; }
    *icon = CreateInputModeIcon(state_, GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON));
    return *icon != nullptr ? S_OK : E_FAIL;
}
STDMETHODIMP InputModeButton::OnClick(TfLBIClick click, POINT, const RECT*) noexcept {
    if (click != TF_LBI_CLK_LEFT || !can_toggle_ || callback_ == nullptr) { return S_FALSE; }
    callback_(owner_); return S_OK;
}
STDMETHODIMP InputModeButton::InitMenu(ITfMenu*) noexcept { return E_NOTIMPL; }
STDMETHODIMP InputModeButton::OnMenuSelect(UINT) noexcept { return E_NOTIMPL; }
STDMETHODIMP InputModeButton::AdviseSink(REFIID iid, IUnknown* sink, DWORD* cookie) noexcept {
    if (cookie == nullptr) { return E_POINTER; }
    *cookie = TF_INVALID_COOKIE;
    if (iid != IID_ITfLangBarItemSink) { return CONNECT_E_CANNOTCONNECT; }
    if (sink == nullptr) { return E_INVALIDARG; }
    if (sink_ != nullptr) { return CONNECT_E_ADVISELIMIT; }
    if (next_cookie_ == TF_INVALID_COOKIE) { return E_FAIL; }
    const HRESULT result = sink->QueryInterface(IID_PPV_ARGS(sink_.GetAddressOf()));
    if (FAILED(result)) { return result; }
    cookie_ = next_cookie_++; *cookie = cookie_; return S_OK;
}
STDMETHODIMP InputModeButton::UnadviseSink(DWORD cookie) noexcept {
    if (sink_ == nullptr || cookie != cookie_) { return CONNECT_E_NOCONNECTION; }
    sink_.Reset(); cookie_ = TF_INVALID_COOKIE; return S_OK;
}
}
