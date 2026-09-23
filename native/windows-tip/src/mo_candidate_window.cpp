#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <windowsx.h>

#include <algorithm>
#include <limits>
#include <utility>

#include "mo_candidate_window.h"

namespace {
constexpr wchar_t kWindowClass[] = L"Mo.CandidateWindow.v1";
constexpr int kPreviousPage = 32;
constexpr int kNextPage = 33;

bool DisplayText(const std::string& utf8, std::wstring* wide) {
    if (utf8.empty()) { wide->clear(); return true; }
    if (utf8.size() > 4096) { return false; }
    const int length = static_cast<int>(utf8.size());
    const int required = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS,
        utf8.data(), length, nullptr, 0);
    if (required <= 0) { return false; }
    wide->resize(static_cast<std::size_t>(required));
    if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS,
            utf8.data(), length, wide->data(), required) != required) { return false; }
    for (auto& character : *wide) {
        if (character < L' ' || character == 0x7f) { character = L' '; }
    }
    return true;
}

void Fill(HDC dc, const RECT& rectangle, COLORREF color) noexcept {
    const HBRUSH brush = CreateSolidBrush(color);
    if (brush != nullptr) { FillRect(dc, &rectangle, brush); DeleteObject(brush); }
}

void Draw(HDC dc, const std::wstring& text, RECT rectangle, COLORREF color) noexcept {
    SetTextColor(dc, color);
    DrawTextW(dc, text.data(), static_cast<int>(text.size()), &rectangle,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS);
}

void Draw(HDC dc, const wchar_t* text, RECT rectangle, COLORREF color) noexcept {
    SetTextColor(dc, color);
    DrawTextW(dc, text, -1, &rectangle,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS);
}

struct Palette final {
    COLORREF background;
    COLORREF preedit;
    COLORREF text;
    COLORREF pressed;
    COLORREF pressed_text;
    COLORREF footer;
    COLORREF footer_text;
};

Palette ResolvePalette(mo::windows_tip::CandidateTheme theme) noexcept {
    if (theme == mo::windows_tip::CandidateTheme::Dark) {
        return {RGB(32, 32, 34), RGB(190, 190, 194), RGB(245, 245, 247),
            RGB(54, 72, 92), RGB(245, 245, 247), RGB(43, 43, 46), RGB(214, 214, 218)};
    }
    if (theme == mo::windows_tip::CandidateTheme::Light) {
        return {RGB(250, 250, 248), RGB(80, 80, 80), RGB(25, 25, 25),
            RGB(220, 232, 245), RGB(25, 25, 25), RGB(238, 238, 235), RGB(70, 70, 70)};
    }
    return {GetSysColor(COLOR_WINDOW), GetSysColor(COLOR_GRAYTEXT),
        GetSysColor(COLOR_WINDOWTEXT), GetSysColor(COLOR_HIGHLIGHT),
        GetSysColor(COLOR_HIGHLIGHTTEXT), GetSysColor(COLOR_BTNFACE),
        GetSysColor(COLOR_BTNTEXT)};
}
}  // namespace

namespace mo::windows_tip {

CandidateWindow::~CandidateWindow() noexcept { Destroy(); }

bool CandidateWindow::Update(HINSTANCE module, HWND owner, const RECT& anchor,
    const BrokerSnapshot& snapshot, CandidateTheme theme,
    ActionCallback callback, void* context) noexcept {
    try {
        Hide();
        if (snapshot.composition.empty() || snapshot.candidates.empty()
            || snapshot.candidates.size() > 32 || snapshot.revision == 0
            || !IsWindow(owner) || anchor.bottom <= anchor.top || anchor.right < anchor.left
            || callback == nullptr) { return false; }
        std::wstring preedit;
        if (!DisplayText(snapshot.composition, &preedit)) { return false; }
        std::vector<std::wstring> rows;
        for (std::size_t index = 0; index < snapshot.candidates.size(); ++index) {
            std::wstring text;
            if (snapshot.candidates[index].size() > 512
                || !DisplayText(snapshot.candidates[index], &text)) { return false; }
            rows.push_back(std::to_wstring(index + 1) + L".  " + text);
        }
        if (window_ == nullptr) {
            WNDCLASSEXW definition{};
            definition.cbSize = sizeof(definition);
            definition.hInstance = module;
            definition.lpfnWndProc = WindowProc;
            definition.lpszClassName = kWindowClass;
            definition.hCursor = LoadCursorW(nullptr, IDC_ARROW);
            if (!RegisterClassExW(&definition)) {
                WNDCLASSEXW existing{};
                existing.cbSize = sizeof(existing);
                if (GetLastError() != ERROR_CLASS_ALREADY_EXISTS
                    || !GetClassInfoExW(module, kWindowClass, &existing)
                    || existing.lpfnWndProc != WindowProc) { return false; }
            }
            module_ = module;
            window_ = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                kWindowClass, L"Mo 墨 · 候选", WS_POPUP | WS_BORDER,
                0, 0, 0, 0, owner, nullptr, module, this);
            if (window_ == nullptr) { UnregisterClassW(kWindowClass, module); return false; }
        }
        SetWindowLongPtrW(window_, GWLP_HWNDPARENT, reinterpret_cast<LONG_PTR>(owner));
        const UINT dpi = GetDpiForWindow(owner);
        const int effective_dpi = dpi == 0 ? 96 : static_cast<int>(dpi);
        row_height_ = MulDiv(32, effective_dpi, 96);
        header_height_ = MulDiv(40, effective_dpi, 96);
        footer_height_ = MulDiv(32, effective_dpi, 96);
        padding_ = MulDiv(12, effective_dpi, 96);
        const HFONT font = CreateFontW(-MulDiv(11, effective_dpi, 72),
            0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
            DEFAULT_PITCH, L"Microsoft YaHei UI");
        if (font != nullptr) { if (font_ != nullptr) { DeleteObject(font_); } font_ = font; }

        MONITORINFO monitor{};
        monitor.cbSize = sizeof(monitor);
        if (!GetMonitorInfoW(MonitorFromPoint({anchor.left, anchor.top}, MONITOR_DEFAULTTONEAREST), &monitor)) {
            return false;
        }
        const RECT work = monitor.rcWork;
        const int available_height = work.bottom - work.top;
        visible_rows_ = std::min(static_cast<int>(rows.size()),
            std::max(0, (available_height - header_height_ - footer_height_ - 2) / row_height_));
        if (visible_rows_ == 0) { return false; }
        const int width = std::min(MulDiv(360, effective_dpi, 96),
            static_cast<int>(work.right - work.left));
        const int height = header_height_ + visible_rows_ * row_height_ + footer_height_ + 2;
        const int x = std::clamp(static_cast<int>(anchor.left),
            static_cast<int>(work.left), static_cast<int>(work.right) - width);
        int y = static_cast<int>(anchor.bottom);
        if (y + height > work.bottom) { y = static_cast<int>(anchor.top) - height; }
        y = std::clamp(y, static_cast<int>(work.top), static_cast<int>(work.bottom) - height);
        preedit_ = std::move(preedit);
        rows_ = std::move(rows);
        revision_ = snapshot.revision;
        first_row_ = 0;
        callback_ = callback;
        context_ = context;
        theme_ = theme;
        if (!SetWindowPos(window_, HWND_TOPMOST, x, y, width, height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW)) { Hide(); return false; }
        InvalidateRect(window_, nullptr, FALSE);
        return true;
    } catch (...) { Hide(); return false; }
}

void CandidateWindow::ResetPress() noexcept {
    pressed_item_ = -1;
    pressed_revision_ = 0;
    if (window_ != nullptr && GetCapture() == window_) { ReleaseCapture(); }
}

void CandidateWindow::Hide() noexcept {
    ResetPress();
    revision_ = 0;
    callback_ = nullptr;
    context_ = nullptr;
    if (window_ != nullptr) { ShowWindow(window_, SW_HIDE); }
}

void CandidateWindow::Destroy() noexcept {
    Hide();
    if (window_ != nullptr) { DestroyWindow(window_); window_ = nullptr; }
    if (font_ != nullptr) { DeleteObject(font_); font_ = nullptr; }
    if (module_ != nullptr) { UnregisterClassW(kWindowClass, module_); module_ = nullptr; }
    rows_.clear();
    preedit_.clear();
}

int CandidateWindow::HitTest(LPARAM position) const noexcept {
    RECT client{};
    GetClientRect(window_, &client);
    const int x = GET_X_LPARAM(position);
    const int y = GET_Y_LPARAM(position);
    if (x < 0 || x >= client.right || y < header_height_ || y >= client.bottom) { return -1; }
    const int row = (y - header_height_) / row_height_;
    if (row < visible_rows_) { return first_row_ + row; }
    return x < client.right / 2 ? kPreviousPage : kNextPage;
}

void CandidateWindow::Paint() noexcept {
    PAINTSTRUCT paint{};
    const HDC dc = BeginPaint(window_, &paint);
    if (dc == nullptr) { return; }
    RECT client{};
    GetClientRect(window_, &client);
    const Palette palette = ResolvePalette(theme_);
    Fill(dc, client, palette.background);
    const HGDIOBJ previous_font = SelectObject(dc,
        font_ != nullptr ? font_ : GetStockObject(DEFAULT_GUI_FONT));
    SetBkMode(dc, TRANSPARENT);
    Draw(dc, preedit_, {padding_, 0, client.right - padding_, header_height_}, palette.preedit);
    for (int index = 0; index < visible_rows_; ++index) {
        const int row = first_row_ + index;
        RECT rectangle{0, header_height_ + index * row_height_, client.right,
            header_height_ + (index + 1) * row_height_};
        if (row == pressed_item_) { Fill(dc, rectangle, palette.pressed); }
        rectangle.left = padding_;
        rectangle.right -= padding_;
        Draw(dc, rows_[static_cast<std::size_t>(row)], rectangle,
            row == pressed_item_ ? palette.pressed_text : palette.text);
    }
    const int footer_top = header_height_ + visible_rows_ * row_height_;
    Fill(dc, {0, footer_top, client.right, client.bottom}, palette.footer);
    Draw(dc, L"‹ 上一页", {padding_, footer_top, client.right / 2, client.bottom}, palette.footer_text);
    Draw(dc, L"下一页 ›", {client.right / 2 + padding_, footer_top,
        client.right - padding_, client.bottom}, palette.footer_text);
    SelectObject(dc, previous_font);
    EndPaint(window_, &paint);
}

LRESULT CALLBACK CandidateWindow::WindowProc(HWND window, UINT message,
    WPARAM parameter, LPARAM data) noexcept {
    auto* self = reinterpret_cast<CandidateWindow*>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
        const auto* creation = reinterpret_cast<const CREATESTRUCTW*>(data);
        self = static_cast<CandidateWindow*>(creation->lpCreateParams);
        self->window_ = window;
        SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self));
    }
    if (self == nullptr) { return DefWindowProcW(window, message, parameter, data); }
    switch (message) {
    case WM_MOUSEACTIVATE: return MA_NOACTIVATE;
    case WM_ERASEBKGND: return 1;
    case WM_PAINT: self->Paint(); return 0;
    case WM_LBUTTONDOWN:
        if (self->revision_ != 0) {
            self->pressed_item_ = self->HitTest(data);
            self->pressed_revision_ = self->revision_;
            SetCapture(window);
            InvalidateRect(window, nullptr, FALSE);
        }
        return 0;
    case WM_LBUTTONUP: {
        const int item = self->HitTest(data);
        const auto revision = self->pressed_revision_;
        const auto callback = self->callback_;
        void* const context = self->context_;
        const bool accepted = item >= 0 && item == self->pressed_item_
            && revision != 0 && revision == self->revision_ && callback != nullptr;
        self->ResetPress();
        InvalidateRect(window, nullptr, FALSE);
        if (accepted) {
            const auto action = item == kPreviousPage ? CandidateAction::PreviousPage
                : item == kNextPage ? CandidateAction::NextPage : CandidateAction::Select;
            // The callback may release the service and destroy this window.
            // Do not touch self after it returns.
            callback(context, revision, action,
                action == CandidateAction::Select ? static_cast<std::uint32_t>(item) : 0);
        }
        return 0;
    }
    case WM_CAPTURECHANGED: self->pressed_item_ = -1; self->pressed_revision_ = 0; return 0;
    case WM_MOUSEWHEEL:
        self->ResetPress();
        if (self->rows_.size() > static_cast<std::size_t>(self->visible_rows_)) {
            self->first_row_ = std::clamp(self->first_row_
                + (GET_WHEEL_DELTA_WPARAM(parameter) < 0 ? 1 : -1), 0,
                static_cast<int>(self->rows_.size()) - self->visible_rows_);
            InvalidateRect(window, nullptr, FALSE);
        }
        return 0;
    case WM_NCDESTROY:
        self->window_ = nullptr;
        SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        break;
    default: break;
    }
    return DefWindowProcW(window, message, parameter, data);
}
}  // namespace mo::windows_tip
