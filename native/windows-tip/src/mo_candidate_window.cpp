#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <windowsx.h>
#include <commctrl.h>

#include <algorithm>
#include <limits>
#include <utility>

#include "mo_candidate_window.h"
#include "mo_candidate_palette.h"
#include "mo_ui_icons.h"
#include "mo_symbols.h"

namespace {
constexpr wchar_t kWindowClass[] = L"Mo.CandidateWindow.v1";
constexpr int kPreviousPage = 32;
constexpr int kNextPage = 33;
constexpr int kExpand = 34;
constexpr int kSmile = 35;
constexpr int kClose = 36;

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

mo::windows_tip::CandidatePalette SystemPalette() noexcept {
    return {GetSysColor(COLOR_WINDOW), GetSysColor(COLOR_GRAYTEXT),
        GetSysColor(COLOR_WINDOWTEXT), GetSysColor(COLOR_HIGHLIGHT),
        GetSysColor(COLOR_HIGHLIGHTTEXT), GetSysColor(COLOR_BTNFACE),
        GetSysColor(COLOR_BTNTEXT), GetSysColor(COLOR_WINDOWFRAME)};
}
}  // namespace

namespace mo::windows_tip {

CandidateWindow::~CandidateWindow() noexcept { Destroy(); }

bool CandidateWindow::Update(HINSTANCE module, HWND owner, const RECT& anchor,
    const BrokerSnapshot& snapshot, CandidateTheme theme, bool show_comments,
    ActionCallback callback, void* context, const BrokerSettings* settings, InputScheme active_scheme) noexcept {
    try {
        Hide(false);
        if (snapshot.composition.empty() || snapshot.candidates.empty()
            || snapshot.candidates.size() > 32 || snapshot.revision == 0
            || !IsWindow(owner) || anchor.bottom <= anchor.top || anchor.right < anchor.left
            || callback == nullptr) { return false; }
        std::wstring preedit;
        if (!DisplayText(snapshot.composition, &preedit)) { return false; }
        std::vector<Row> rows;
        for (std::size_t index = 0; index < snapshot.candidates.size(); ++index) {
            std::wstring text;
            if (snapshot.candidates[index].size() > 512
                || !DisplayText(snapshot.candidates[index], &text)) { return false; }
            std::wstring label = std::to_wstring(index + 1);
            if (index < snapshot.candidate_labels.size()
                && snapshot.candidate_labels[index].has_value()
                && !snapshot.candidate_labels[index]->empty()
                && !DisplayText(*snapshot.candidate_labels[index], &label)) { return false; }
            Row row{std::move(label), std::move(text), {}};
            if (show_comments && index < snapshot.candidate_comments.size()
                && snapshot.candidate_comments[index].has_value()
                && !snapshot.candidate_comments[index]->empty()) {
                std::wstring comment;
                if (!DisplayText(*snapshot.candidate_comments[index], &comment)) { return false; }
                row.comment = std::move(comment);
            }
            rows.push_back(std::move(row));
        }
        if (window_ == nullptr) {
            WNDCLASSEXW definition{};
            definition.cbSize = sizeof(definition);
            definition.hInstance = module;
            definition.lpfnWndProc = WindowProc;
            definition.lpszClassName = kWindowClass;
            definition.hCursor = LoadCursorW(nullptr, IDC_ARROW);
            definition.style = CS_DROPSHADOW;
            if (!RegisterClassExW(&definition)) {
                WNDCLASSEXW existing{};
                existing.cbSize = sizeof(existing);
                if (GetLastError() != ERROR_CLASS_ALREADY_EXISTS
                    || !GetClassInfoExW(module, kWindowClass, &existing)
                    || existing.lpfnWndProc != WindowProc) { return false; }
            }
            module_ = module;
            window_ = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                kWindowClass, L"Mo 墨 · 候选", WS_POPUP,
                0, 0, 0, 0, owner, nullptr, module, this);
            if (window_ == nullptr) { UnregisterClassW(kWindowClass, module); return false; }
            RefreshAccessibilityState();
            const INITCOMMONCONTROLSEX controls{sizeof(INITCOMMONCONTROLSEX), ICC_WIN95_CLASSES};
            InitCommonControlsEx(&controls);
            tooltip_ = CreateWindowExW(WS_EX_TOPMOST | WS_EX_NOACTIVATE, TOOLTIPS_CLASSW, nullptr,
                WS_POPUP | TTS_ALWAYSTIP | TTS_NOPREFIX, 0, 0, 0, 0, window_, nullptr, module_, nullptr);
            if (tooltip_ != nullptr) {
                const wchar_t* labels[] = {L"上一页", L"下一页", L"展开或收起候选", L"打开表情面板",
                    L"联想表情：保存偏好，当前输入结束后生效"};
                for (int i = 0; i < 5; ++i) {
                    TOOLINFOW tool{}; tool.cbSize = sizeof(tool); tool.uFlags = TTF_SUBCLASS;
                    tool.hwnd = window_; tool.uId = static_cast<UINT_PTR>(i + 1);
                    tool.lpszText = const_cast<wchar_t*>(labels[i]);
                    SendMessageW(tooltip_, TTM_ADDTOOLW, 0, reinterpret_cast<LPARAM>(&tool));
                }
            }
        }
        SetWindowLongPtrW(window_, GWLP_HWNDPARENT, reinterpret_cast<LONG_PTR>(owner));
        const int old_font_size = appearance_.skin == 6 ? appearance_.font_size : 16;
        appearance_ = settings != nullptr ? *settings : BrokerSettings{};
        active_scheme_ = active_scheme;
        const UINT dpi = GetDpiForWindow(owner);
        const int effective_dpi = dpi == 0 ? 96 : static_cast<int>(dpi);
        row_height_ = MulDiv(38, effective_dpi, 96);
        header_height_ = MulDiv(44, effective_dpi, 96);
        footer_height_ = MulDiv(36, effective_dpi, 96);
        padding_ = MulDiv(12, effective_dpi, 96);
        gap_ = std::max(1, MulDiv(4, effective_dpi, 96));
        radius_ = MulDiv(appearance_.skin == 6 ? appearance_.radius : 12, effective_dpi, 96);
        const auto replace_font = [effective_dpi](HFONT& current, int pixels) {
            const HFONT font = CreateFontW(-MulDiv(pixels, effective_dpi, 96),
                0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                DEFAULT_PITCH, L"Microsoft YaHei UI");
            if (font != nullptr) {
                if (current != nullptr) { DeleteObject(current); }
                current = font;
            }
        };
        if (font_dpi_ != effective_dpi || old_font_size != (appearance_.skin == 6 ? appearance_.font_size : 16)
            || font_ == nullptr
            || body_font_ == nullptr || caption_font_ == nullptr) {
            replace_font(font_, appearance_.skin == 6 ? appearance_.font_size : 16);
            replace_font(body_font_, 14);
            replace_font(caption_font_, 12);
            const HFONT emoji = CreateFontW(-MulDiv(26, effective_dpi, 96), 0, 0, 0, FW_NORMAL,
                FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY, DEFAULT_PITCH, L"Segoe UI Emoji");
            if (emoji != nullptr) {
                if (emoji_font_ != nullptr) { DeleteObject(emoji_font_); }
                emoji_font_ = emoji;
            }
            font_dpi_ = effective_dpi;
        }

        MONITORINFO monitor{};
        monitor.cbSize = sizeof(monitor);
        if (!GetMonitorInfoW(MonitorFromPoint({anchor.left, anchor.top}, MONITOR_DEFAULTTONEAREST), &monitor)) {
            return false;
        }
        work_ = monitor.rcWork;
        anchor_ = anchor;
        width_ = std::min(Scale(560), static_cast<int>(work_.right - work_.left));
        if (preedit != preedit_) { view_ = 0; first_row_ = 0; }
        symbols_supported_ = snapshot.symbols_supported;
        quick_settings_supported_ = snapshot.quick_settings_supported;
        if (!symbols_supported_ && view_ == 2) { view_ = 0; }
        page_ = snapshot.page;
        preference_error_ = false;
        preedit_ = std::move(preedit);
        rows_ = std::move(rows);
        revision_ = snapshot.revision;
        first_row_ = std::clamp(first_row_, 0, std::max(0, static_cast<int>(rows_.size()) - 1));
        callback_ = callback;
        context_ = context;
        theme_ = theme;
        if (!Reflow()) { Hide(); return false; }
        return true;
    } catch (...) { Hide(); return false; }
}

void CandidateWindow::ResetPress() noexcept {
    pressed_item_ = -1;
    pressed_revision_ = 0;
    if (window_ != nullptr && GetCapture() == window_) { ReleaseCapture(); }
}

void CandidateWindow::Hide(bool reset_view) noexcept {
    if (reset_view) { view_ = 0; first_row_ = 0; }
    ResetPress();
    revision_ = 0;
    callback_ = nullptr;
    context_ = nullptr;
    if (window_ != nullptr) { ShowWindow(window_, SW_HIDE); }
}

void CandidateWindow::Destroy() noexcept {
    Hide();
    if (tooltip_ != nullptr) { DestroyWindow(tooltip_); tooltip_ = nullptr; }
    if (window_ != nullptr) { DestroyWindow(window_); window_ = nullptr; }
    if (font_ != nullptr) { DeleteObject(font_); font_ = nullptr; }
    if (module_ != nullptr) { UnregisterClassW(kWindowClass, module_); module_ = nullptr; }
    if (body_font_ != nullptr) { DeleteObject(body_font_); body_font_ = nullptr; }
    if (caption_font_ != nullptr) { DeleteObject(caption_font_); caption_font_ = nullptr; }
    if (emoji_font_ != nullptr) { DeleteObject(emoji_font_); emoji_font_ = nullptr; }
    font_dpi_ = 0;
    rows_.clear();
    preedit_.clear();
}

bool CandidateWindow::panel_open() const noexcept {
    return view_ != 0 && window_ != nullptr && IsWindowVisible(window_) != FALSE && revision_ != 0;
}
bool CandidateWindow::ClosePanel() noexcept {
    if (!panel_open()) { return false; }
    ResetPress(); view_ = 0; return Reflow();
}
void CandidateWindow::ReportSettingsFailure() noexcept {
    preference_error_ = true;
    if (window_ != nullptr) { InvalidateRect(window_, nullptr, FALSE); }
}
RECT CandidateWindow::ToolbarRect(int item) const noexcept {
    const int x = width_ - padding_ - Scale(124) + item * Scale(32);
    const int top = view_ == 0 && appearance_.vertical ? 8 : 42;
    return {x, Scale(top), x + Scale(28), Scale(top + 28)};
}
bool CandidateWindow::ActionEnabled(int item) const noexcept {
    if (item == kPreviousPage) { return page_.has_value() && page_->has_previous; }
    if (item == kNextPage) { return page_.has_value() && page_->has_next; }
    if (item == kSmile) { return symbols_supported_; }
    if (item == 44) { return quick_settings_supported_; }
    return true;
}
bool CandidateWindow::Reflow() noexcept {
    width_ = std::min(Scale(view_ == 0 && appearance_.vertical ? 340 : 560),
        static_cast<int>(work_.right - work_.left));
    compact_rects_.fill(RECT{}); expanded_rects_.fill(RECT{}); symbol_rects_.fill(RECT{});
    compact_count_ = 0; symbol_count_ = 0;
    const HDC dc = GetDC(window_);
    if (dc == nullptr || width_ < Scale(224)) { if (dc != nullptr) { ReleaseDC(window_, dc); } return false; }
    int x = padding_;
    const int right = ToolbarRect(0).left - Scale(8);
    const auto old_font = SelectObject(dc, font_);
    for (std::size_t i = 0; !(view_ == 0 && appearance_.vertical) && i < rows_.size(); ++i) {
        SIZE text{}, label{};
        GetTextExtentPoint32W(dc, rows_[i].text.data(), static_cast<int>(rows_[i].text.size()), &text);
        SelectObject(dc, caption_font_);
        GetTextExtentPoint32W(dc, rows_[i].label.data(), static_cast<int>(rows_[i].label.size()), &label);
        SelectObject(dc, font_);
        const int desired = std::clamp(static_cast<int>(text.cx + label.cx) + Scale(28), Scale(60), Scale(180));
        const int width = std::min(desired, right - x);
        if (width < Scale(60)) { break; }
        compact_rects_[i] = {x, Scale(40), x + width, Scale(72)};
        ++compact_count_; x += width + gap_;
    }
    SelectObject(dc, old_font); ReleaseDC(window_, dc);
    int height = Scale(86);
    visible_rows_ = 0;
    if (view_ == 0 && appearance_.vertical) {
        const int available = std::max(1, std::min(8, (static_cast<int>(work_.bottom - work_.top) - Scale(56)) / Scale(38)));
        first_row_ = std::clamp(first_row_, 0, std::max(0, static_cast<int>(rows_.size()) - available));
        for (int i = first_row_; i < static_cast<int>(rows_.size()) && visible_rows_ < available; ++i) {
            const int top = Scale(44) + visible_rows_ * Scale(38);
            expanded_rects_[static_cast<std::size_t>(i)] = {padding_, top, width_ - padding_, top + Scale(34)};
            ++visible_rows_;
        }
        height = Scale(56) + visible_rows_ * Scale(38);
    } else     if (view_ == 1) {
        const int cell_width = (width_ - 2 * padding_ - 2 * Scale(8)) / 3;
        const int available = std::max(1, std::min(4, (static_cast<int>(work_.bottom - work_.top) - Scale(122)) / Scale(64)));
        first_row_ = std::clamp(first_row_, 0, std::max(0, static_cast<int>(rows_.size()) - available * 3));
        for (int i = first_row_; i < static_cast<int>(rows_.size()) && visible_rows_ < available * 3; ++i) {
            const int col = visible_rows_ % 3, row = visible_rows_ / 3;
            const int left = padding_ + col * (cell_width + Scale(8));
            const int top = Scale(112) + row * Scale(64);
            expanded_rects_[static_cast<std::size_t>(i)] = {left, top, left + cell_width, top + Scale(60)};
            ++visible_rows_;
        }
        height = Scale(122) + ((visible_rows_ + 2) / 3) * Scale(64);
    } else if (view_ == 2) {
        const int cell_width = (width_ - 2 * padding_ - 7 * Scale(8)) / 8;
        for (int i = 0; i < 28; ++i) {
            if ((category_ == 0 && i >= 24) || (category_ == 1 && kSymbols[i].category != 1)
                || (category_ == 2 && kSymbols[i].category != 2)
                || (category_ == 3 && kSymbols[i].category != 3)) { continue; }
            const int col = category_ == 3 ? 0 : symbol_count_ % 8;
            const int row = category_ == 3 ? symbol_count_ : symbol_count_ / 8;
            const int left = padding_ + col * (cell_width + Scale(8));
            const int top = Scale(154) + row * Scale(48);
            symbol_rects_[static_cast<std::size_t>(symbol_count_)] = {left, top,
                category_ == 3 ? width_ - padding_ : left + cell_width, top + Scale(44)};
            symbol_indices_[static_cast<std::size_t>(symbol_count_++)] = i;
        }
        const int panel_height = std::min(Scale(category_ == 3 ? 382 : 334),
            static_cast<int>(work_.bottom - work_.top));
        const int available = std::max(1, (panel_height - Scale(182)) / Scale(48));
        const int total = category_ == 3 ? symbol_count_ : (symbol_count_ + 7) / 8;
        first_row_ = std::clamp(first_row_, 0, std::max(0, total - available));
        for (int i = 0; i < symbol_count_; ++i) {
            OffsetRect(&symbol_rects_[static_cast<std::size_t>(i)], 0, -first_row_ * Scale(48));
        }
        height = Scale(category_ == 3 ? 382 : 334);
    }
    height = std::min(height, static_cast<int>(work_.bottom - work_.top));
    const int left = std::clamp(static_cast<int>(anchor_.left), static_cast<int>(work_.left),
        static_cast<int>(work_.right) - width_);
    int top = static_cast<int>(anchor_.bottom);
    if (top + height > work_.bottom) { top = static_cast<int>(anchor_.top) - height; }
    top = std::clamp(top, static_cast<int>(work_.top), static_cast<int>(work_.bottom) - height);
    if (!SetWindowPos(window_, HWND_TOPMOST, left, top, width_, height, SWP_NOACTIVATE | SWP_SHOWWINDOW)) { return false; }
    if (tooltip_ != nullptr) {
        for (int i = 0; i < 5; ++i) {
            TOOLINFOW tool{}; tool.cbSize = sizeof(tool); tool.hwnd = window_;
            tool.uId = static_cast<UINT_PTR>(i + 1);
            tool.rect = i < 4 ? ToolbarRect(i) : view_ == 2
                ? RECT{width_ - padding_ - Scale(120), Scale(120), width_ - padding_, Scale(148)} : RECT{};
            SendMessageW(tooltip_, TTM_NEWTOOLRECTW, 0, reinterpret_cast<LPARAM>(&tool));
        }
    }
    RefreshShape(); InvalidateRect(window_, nullptr, FALSE); return true;
}
int CandidateWindow::HitTest(LPARAM position) const noexcept {
    const POINT point{GET_X_LPARAM(position), GET_Y_LPARAM(position)};
    RECT client{}; GetClientRect(window_, &client);
    if (!PtInRect(&client, point)) { return -1; }
    for (int i = 0; i < 4; ++i) {
        const RECT box = ToolbarRect(i);
        if (PtInRect(&box, point)) { return ActionEnabled(kPreviousPage + i) ? kPreviousPage + i : -1; }
    }
    if (view_ == 2) {
        const RECT close{width_ - padding_ - Scale(28), Scale(90), width_ - padding_, Scale(118)};
        if (PtInRect(&close, point)) { return kClose; }
        for (int i = 0; i < 4; ++i) {
            const RECT tab{padding_ + i * Scale(64), Scale(120), padding_ + i * Scale(64) + Scale(60), Scale(148)};
            if (PtInRect(&tab, point)) { return 40 + i; }
        }
        const RECT toggle{width_ - padding_ - Scale(120), Scale(120), width_ - padding_, Scale(148)};
        if (PtInRect(&toggle, point)) { return ActionEnabled(44) ? 44 : -1; }
        for (int i = 0; i < symbol_count_; ++i) {
            const RECT box = symbol_rects_[static_cast<std::size_t>(i)];
            if (box.top >= Scale(154) && box.bottom <= client.bottom - Scale(28) && PtInRect(&box, point)) {
                return 100 + symbol_indices_[static_cast<std::size_t>(i)];
            }
        }
    }
    for (std::size_t i = 0; i < rows_.size(); ++i) {
        if (PtInRect(&compact_rects_[i], point)
            || ((view_ == 1 || (view_ == 0 && appearance_.vertical)) && PtInRect(&expanded_rects_[i], point))) { return static_cast<int>(i); }
    }
    return -1;
}

void CandidateWindow::RefreshAccessibilityState() noexcept {
    HIGHCONTRASTW state{};
    state.cbSize = sizeof(state);
    const bool queried = SystemParametersInfoW(SPI_GETHIGHCONTRAST,
        sizeof(state), &state, 0) != FALSE;
    high_contrast_ = UseHighContrastPalette(queried, state.dwFlags);
    DWORD value = 1, bytes = sizeof(value);
    system_dark_ = RegGetValueW(HKEY_CURRENT_USER,
        L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize", L"AppsUseLightTheme",
        RRF_RT_REG_DWORD, nullptr, &value, &bytes) == ERROR_SUCCESS && value == 0;
    // SPI_GETHIGHCONTRAST (Unicode) supplies a system-owned scheme pointer.
    // Only dwFlags is needed; never read, retain, or free the scheme name.
}

void CandidateWindow::RefreshShape() noexcept {
    if (window_ == nullptr) { return; }
    if (high_contrast_) { SetWindowRgn(window_, nullptr, TRUE); return; }
    RECT client{};
    GetClientRect(window_, &client);
    const HRGN region = CreateRoundRectRgn(0, 0, client.right + 1, client.bottom + 1,
        radius_ * 2, radius_ * 2);
    // Ownership transfers only after a successful SetWindowRgn.
    if (region != nullptr && SetWindowRgn(window_, region, TRUE) == 0) { DeleteObject(region); }
}

void CandidateWindow::Paint() noexcept {
    PAINTSTRUCT paint{};
    const HDC dc = BeginPaint(window_, &paint);
    if (dc != nullptr) { Render(dc); }
    EndPaint(window_, &paint);
}

void CandidateWindow::Render(HDC dc) noexcept {
    RECT client{}; GetClientRect(window_, &client);
    const CandidatePalette palette = ResolveSkinPalette(appearance_, theme_ == CandidateTheme::System
        ? system_dark_ ? CandidateTheme::Dark : CandidateTheme::Light : theme_, SystemPalette(), high_contrast_);
    Fill(dc, client, palette.background);
    const HGDIOBJ previous_font = SelectObject(dc, body_font_ != nullptr ? body_font_ : GetStockObject(DEFAULT_GUI_FONT));
    const int previous_mode = SetBkMode(dc, TRANSPARENT);
    const COLORREF previous_color = GetTextColor(dc);
    const bool vertical = view_ == 0 && appearance_.vertical;
    Draw(dc, preedit_, {padding_, Scale(10), vertical ? ToolbarRect(0).left - gap_ : width_ - padding_ - Scale(100), Scale(34)}, palette.text);
    SelectObject(dc, caption_font_);
    const wchar_t* badge = active_scheme_ == InputScheme::FullPinyin ? L"中 · 全拼" : L"中 · 双拼";
    if (!vertical) { Draw(dc, badge, {width_ - padding_ - Scale(64), Scale(10), width_ - padding_, Scale(34)}, palette.footer_text); }
    const auto rounded = [this, dc](RECT box, COLORREF color) {
        const HBRUSH brush = CreateSolidBrush(color);
        if (brush == nullptr) { return; }
        const HGDIOBJ old_brush = SelectObject(dc, brush), old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
        if (high_contrast_) { FillRect(dc, &box, brush); }
        else { RoundRect(dc, box.left, box.top, box.right, box.bottom, Scale(16), Scale(16)); }
        SelectObject(dc, old_pen); SelectObject(dc, old_brush); DeleteObject(brush);
    };
    const auto row_draw = [&](int index, RECT box, bool expanded) {
        if (box.right <= box.left) { return; }
        const Row& row = rows_[static_cast<std::size_t>(index)];
        const bool primary = pressed_item_ >= 0 && pressed_item_ < 32
            ? pressed_item_ == index : page_.has_value() && page_->highlighted.has_value()
                ? *page_->highlighted == index : index == 0;
        if (primary) { rounded(box, palette.pressed); }
        const COLORREF color = primary ? palette.pressed_text : palette.text;
        const COLORREF secondary = primary ? palette.pressed_text : palette.footer_text;
        SelectObject(dc, caption_font_);
        RECT label{box.left + Scale(8), box.top, box.left + Scale(28), expanded && !vertical ? box.top + Scale(30) : box.bottom};
        Draw(dc, row.label, label, secondary);
        SelectObject(dc, font_);
        RECT text{box.left + Scale(32), box.top, box.right - Scale(8), expanded && !vertical ? box.top + Scale(30) : box.bottom};
        Draw(dc, row.text, text, color);
        if (expanded && !row.comment.empty()) {
            SelectObject(dc, caption_font_);
            if (vertical) {
                SelectObject(dc, font_);
                SIZE extent{}; GetTextExtentPoint32W(dc, row.text.data(), static_cast<int>(row.text.size()), &extent);
                SelectObject(dc, caption_font_);
                const int left = box.left + Scale(40) + static_cast<int>(extent.cx);
                if (left < box.right - Scale(8)) { Draw(dc, row.comment, {left, box.top, box.right - Scale(8), box.bottom}, secondary); }
            } else {
                Draw(dc, row.comment, {box.left + Scale(8), box.top + Scale(30), box.right - Scale(8), box.bottom}, secondary);
            }
        }
    };
    for (int i = 0; i < compact_count_; ++i) { row_draw(i, compact_rects_[static_cast<std::size_t>(i)], false); }
    if (vertical) {
        for (std::size_t i = 0; i < rows_.size(); ++i) { row_draw(static_cast<int>(i), expanded_rects_[i], true); }
    }
    const UiIcon icons[] = {UiIcon::Previous, UiIcon::Next, view_ == 1 ? UiIcon::Collapse : UiIcon::Expand, UiIcon::Smile};
    for (int i = 0; i < 4; ++i) {
        RECT box = ToolbarRect(i);
        if (pressed_item_ == kPreviousPage + i) { rounded(box, palette.footer); }
        InflateRect(&box, -Scale(4), -Scale(4));
        COLORREF color = palette.footer_text;
        if (!ActionEnabled(kPreviousPage + i)) {
            color = high_contrast_ ? GetSysColor(COLOR_GRAYTEXT)
                : RGB((GetRValue(color) * 35 + GetRValue(palette.background) * 65) / 100,
                    (GetGValue(color) * 35 + GetGValue(palette.background) * 65) / 100,
                    (GetBValue(color) * 35 + GetBValue(palette.background) * 65) / 100);
        }
        DrawUiIcon(module_, dc, icons[i], box, color);
    }
    if (view_ == 1) {
        SelectObject(dc, body_font_);
        Draw(dc, L"更多候选", {padding_, Scale(82), width_ / 2, Scale(106)}, palette.text);
        SelectObject(dc, caption_font_);
        const std::wstring summary = L"当前页 · " + std::to_wstring(rows_.size()) + L" 项"
            + (rows_.size() > static_cast<std::size_t>(visible_rows_) ? L" · 滚轮查看更多" : L"");
        Draw(dc, summary, {width_ / 2, Scale(82), width_ - padding_, Scale(106)}, palette.footer_text);
        for (std::size_t i = 0; i < rows_.size(); ++i) { row_draw(static_cast<int>(i), expanded_rects_[i], true); }
    } else if (view_ == 2) {
        SelectObject(dc, body_font_);
        Draw(dc, L"表情与颜文字", {padding_, Scale(90), width_ - Scale(60), Scale(118)}, palette.text);
        DrawUiIcon(module_, dc, UiIcon::Close, {width_ - padding_ - Scale(24), Scale(94),
            width_ - padding_ - Scale(4), Scale(114)}, palette.footer_text);
        const wchar_t* tabs[] = {L"常用", L"笑脸", L"手势", L"颜文字"};
        for (int i = 0; i < 4; ++i) {
            RECT tab{padding_ + i * Scale(64), Scale(120), padding_ + i * Scale(64) + Scale(60), Scale(148)};
            if (i == category_) { rounded(tab, palette.pressed); }
            SetTextColor(dc, i == category_ ? palette.pressed_text : palette.footer_text);
            DrawTextW(dc, tabs[i], -1, &tab, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
            SetTextColor(dc, palette.text);
        }
        SelectObject(dc, caption_font_);
        Draw(dc, L"联想表情", {width_ - padding_ - Scale(120), Scale(120), width_ - padding_ - Scale(36), Scale(148)}, palette.footer_text);
        RECT track{width_ - padding_ - Scale(32), Scale(125), width_ - padding_, Scale(143)};
        rounded(track, appearance_.emoji ? palette.pressed : palette.border);
        const int knob_x = appearance_.emoji ? track.right - Scale(16) : track.left + Scale(2);
        RECT knob{knob_x, track.top + Scale(2), knob_x + Scale(14), track.bottom - Scale(2)};
        rounded(knob, appearance_.emoji ? palette.pressed_text : palette.background);
        for (int i = 0; i < symbol_count_; ++i) {
            const int index = symbol_indices_[static_cast<std::size_t>(i)];
            RECT box = symbol_rects_[static_cast<std::size_t>(i)];
            if (box.top < Scale(154) || box.bottom > client.bottom - Scale(28)) { continue; }
            if (pressed_item_ == 100 + index) { rounded(box, palette.footer); }
            SelectObject(dc, category_ == 3 ? body_font_ : emoji_font_);
            SetTextColor(dc, palette.text);
            DrawTextW(dc, kSymbols[index].text, -1, &box, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        }
        SelectObject(dc, caption_font_);
        Draw(dc, preference_error_ ? L"联想设置保存失败，请在设置中重新读取" : L"点击插入表情 · Esc 返回候选", {padding_, client.bottom - Scale(28), width_ - padding_, client.bottom - Scale(8)}, palette.footer_text);
    }
    const HPEN pen = CreatePen(PS_SOLID, 1, palette.border);
    if (pen != nullptr) {
        const HGDIOBJ old_pen = SelectObject(dc, pen), old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
        if (high_contrast_) { Rectangle(dc, 0, 0, client.right, client.bottom); }
        else { RoundRect(dc, 0, 0, client.right, client.bottom, radius_ * 2, radius_ * 2); }
        SelectObject(dc, old_brush); SelectObject(dc, old_pen); DeleteObject(pen);
    }
    SetTextColor(dc, previous_color); SetBkMode(dc, previous_mode); SelectObject(dc, previous_font);
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
    if (IsCandidateAppearanceMessage(message)) {
        // Repaint only: keep candidate revision, press identity, focus and
        // engine/session state intact. Re-read OS accessibility state off the
        // key dispatch path, without changing the saved theme preference.
        self->RefreshAccessibilityState();
        self->RefreshShape();
        InvalidateRect(window, nullptr, FALSE);
        return 0;
    }
    switch (message) {
    case WM_MOUSEACTIVATE: return MA_NOACTIVATE;
    case WM_ERASEBKGND: return 1;
    case WM_PAINT: self->Paint(); return 0;
    case WM_PRINTCLIENT:
        if (parameter != 0) { self->Render(reinterpret_cast<HDC>(parameter)); }
        return 0;
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
            if (item == kExpand || item == kSmile || item == kClose || (item >= 40 && item < 44)) {
                if (item == kExpand) { self->view_ = self->view_ == 1 ? 0 : 1; }
                else if (item == kSmile) { self->view_ = self->view_ == 2 ? 0 : 2; }
                else if (item == kClose) { self->view_ = 0; }
                else { self->category_ = item - 40; }
                self->first_row_ = 0;
                self->Reflow(); return 0;
            }
            const auto action = item == kPreviousPage ? CandidateAction::PreviousPage
                : item == kNextPage ? CandidateAction::NextPage
                : item == 44 ? CandidateAction::ToggleEmoji
                : item >= 100 ? CandidateAction::InsertSymbol : CandidateAction::Select;
            // Callback may release the service and destroy this window.
            callback(context, revision, action, action == CandidateAction::Select
                ? static_cast<std::uint32_t>(item) : action == CandidateAction::InsertSymbol
                    ? static_cast<std::uint32_t>(item - 100) : 0);
        }
        return 0;
    }
    case WM_CAPTURECHANGED:
        self->pressed_item_ = -1;
        self->pressed_revision_ = 0;
        InvalidateRect(window, nullptr, FALSE);
        return 0;
    case WM_MOUSEWHEEL:
        self->ResetPress();
        if (self->view_ == 2) {
            self->first_row_ += GET_WHEEL_DELTA_WPARAM(parameter) < 0 ? 1 : -1;
            self->Reflow();
        } else if ((self->view_ == 1 || (self->view_ == 0 && self->appearance_.vertical)) && self->rows_.size() > static_cast<std::size_t>(self->visible_rows_)) {
            self->first_row_ += (GET_WHEEL_DELTA_WPARAM(parameter) < 0 ? 1 : -1) * (self->view_ == 1 ? 3 : 1);
            self->Reflow();
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
