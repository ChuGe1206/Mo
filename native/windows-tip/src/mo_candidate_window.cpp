#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <windowsx.h>

#include <algorithm>
#include <limits>
#include <utility>

#include "mo_candidate_window.h"
#include "mo_candidate_palette.h"

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
    ActionCallback callback, void* context) noexcept {
    try {
        Hide();
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
        }
        SetWindowLongPtrW(window_, GWLP_HWNDPARENT, reinterpret_cast<LONG_PTR>(owner));
        const UINT dpi = GetDpiForWindow(owner);
        const int effective_dpi = dpi == 0 ? 96 : static_cast<int>(dpi);
        row_height_ = MulDiv(38, effective_dpi, 96);
        header_height_ = MulDiv(44, effective_dpi, 96);
        footer_height_ = MulDiv(36, effective_dpi, 96);
        padding_ = MulDiv(12, effective_dpi, 96);
        gap_ = std::max(1, MulDiv(4, effective_dpi, 96));
        radius_ = MulDiv(12, effective_dpi, 96);
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
        if (font_dpi_ != effective_dpi || font_ == nullptr
            || body_font_ == nullptr || caption_font_ == nullptr) {
            replace_font(font_, 16);
            replace_font(body_font_, 14);
            replace_font(caption_font_, 12);
            font_dpi_ = effective_dpi;
        }

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
        const int width = std::min(MulDiv(340, effective_dpi, 96),
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
        RefreshShape();
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
    if (body_font_ != nullptr) { DeleteObject(body_font_); body_font_ = nullptr; }
    if (caption_font_ != nullptr) { DeleteObject(caption_font_); caption_font_ = nullptr; }
    font_dpi_ = 0;
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
    if (row < visible_rows_) {
        if (x < padding_ || x >= client.right - padding_
            || (y - header_height_) % row_height_ >= row_height_ - gap_) { return -1; }
        return first_row_ + row;
    }
    return x < client.right / 2 ? kPreviousPage : kNextPage;
}

void CandidateWindow::RefreshAccessibilityState() noexcept {
    HIGHCONTRASTW state{};
    state.cbSize = sizeof(state);
    const bool queried = SystemParametersInfoW(SPI_GETHIGHCONTRAST,
        sizeof(state), &state, 0) != FALSE;
    high_contrast_ = UseHighContrastPalette(queried, state.dwFlags);
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
    RECT client{};
    GetClientRect(window_, &client);
    const CandidatePalette palette = ResolveCandidatePalette(theme_, SystemPalette(), high_contrast_);
    Fill(dc, client, palette.background);
    const HGDIOBJ previous_font = SelectObject(dc,
        body_font_ != nullptr ? body_font_ : GetStockObject(DEFAULT_GUI_FONT));
    const int previous_mode = SetBkMode(dc, TRANSPARENT);
    const COLORREF previous_color = GetTextColor(dc);
    const int badge_width = padding_ * 5;
    Draw(dc, preedit_, {padding_, padding_, client.right - padding_ - badge_width,
        header_height_ - gap_ * 2}, palette.preedit);
    SelectObject(dc, caption_font_ != nullptr ? caption_font_ : GetStockObject(DEFAULT_GUI_FONT));
    Draw(dc, L"中 · Mo", {client.right - padding_ - badge_width, padding_,
        client.right - padding_, header_height_ - gap_ * 2}, palette.footer_text);
    for (int index = 0; index < visible_rows_; ++index) {
        const int row_index = first_row_ + index;
        const Row& row = rows_[static_cast<std::size_t>(row_index)];
        RECT rectangle{padding_, header_height_ + index * row_height_, client.right - padding_,
            header_height_ + (index + 1) * row_height_ - gap_};
        // Primary suggestion styling is not an engine selection-state claim.
        const bool primary = pressed_item_ >= 0 ? row_index == pressed_item_ : row_index == 0;
        if (primary) {
            const HBRUSH brush = CreateSolidBrush(palette.pressed);
            const HGDIOBJ old_brush = brush != nullptr ? SelectObject(dc, brush) : nullptr;
            const HGDIOBJ old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
            if (brush != nullptr) {
                if (high_contrast_) { FillRect(dc, &rectangle, brush); }
                else { RoundRect(dc, rectangle.left, rectangle.top, rectangle.right,
                    rectangle.bottom, gap_ * 4, gap_ * 4); }
                SelectObject(dc, old_brush);
                DeleteObject(brush);
            }
            SelectObject(dc, old_pen);
        }
        const COLORREF text_color = primary ? palette.pressed_text : palette.text;
        const COLORREF secondary = primary ? palette.pressed_text : palette.footer_text;
        rectangle.left += padding_;
        rectangle.right -= gap_ * 2;
        SelectObject(dc, caption_font_ != nullptr ? caption_font_ : GetStockObject(DEFAULT_GUI_FONT));
        SIZE label_size{};
        GetTextExtentPoint32W(dc, row.label.data(), static_cast<int>(row.label.size()), &label_size);
        const int label_width = std::min(std::max(static_cast<int>(label_size.cx), gap_ * 2),
            padding_ * 4);
        Draw(dc, row.label, {rectangle.left, rectangle.top, rectangle.left + label_width,
            rectangle.bottom}, secondary);
        rectangle.left += label_width + gap_ * 2;
        SelectObject(dc, font_ != nullptr ? font_ : GetStockObject(DEFAULT_GUI_FONT));
        SIZE text_size{};
        GetTextExtentPoint32W(dc, row.text.data(), static_cast<int>(row.text.size()), &text_size);
        Draw(dc, row.text, rectangle, text_color);
        const int comment_left = rectangle.left + static_cast<int>(text_size.cx) + gap_ * 2;
        if (!row.comment.empty() && comment_left < rectangle.right) {
            SelectObject(dc, caption_font_ != nullptr ? caption_font_ : GetStockObject(DEFAULT_GUI_FONT));
            Draw(dc, row.comment, {comment_left, rectangle.top, rectangle.right,
                rectangle.bottom}, secondary);
        }
    }
    const int footer_top = header_height_ + visible_rows_ * row_height_;
    SelectObject(dc, caption_font_ != nullptr ? caption_font_ : GetStockObject(DEFAULT_GUI_FONT));
    Draw(dc, L"‹ 上一页", {padding_, footer_top, client.right / 2, client.bottom - gap_ * 2},
        palette.footer_text);
    Draw(dc, L"下一页 ›", {client.right / 2 + padding_, footer_top,
        client.right - padding_, client.bottom - gap_ * 2}, palette.footer_text);
    const HPEN pen = CreatePen(PS_SOLID, 1, palette.border);
    if (pen != nullptr) {
        const HGDIOBJ old_pen = SelectObject(dc, pen);
        const HGDIOBJ old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
        if (high_contrast_) { Rectangle(dc, 0, 0, client.right, client.bottom); }
        else { RoundRect(dc, 0, 0, client.right, client.bottom, radius_ * 2, radius_ * 2); }
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(pen);
    }
    SetTextColor(dc, previous_color);
    SetBkMode(dc, previous_mode);
    SelectObject(dc, previous_font);
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
            const auto action = item == kPreviousPage ? CandidateAction::PreviousPage
                : item == kNextPage ? CandidateAction::NextPage : CandidateAction::Select;
            // The callback may release the service and destroy this window.
            // Do not touch self after it returns.
            callback(context, revision, action,
                action == CandidateAction::Select ? static_cast<std::uint32_t>(item) : 0);
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
