#pragma once

#include "mo_broker_client.h"

namespace mo::windows_tip {

// Small thread-affine native frontend. Row numbers are display ordinals, not
// schema selection-key promises. All commits still go through TSF edit locks.
class CandidateWindow final {
public:
    using ActionCallback = void (*)(void*, std::uint64_t, CandidateAction, std::uint32_t) noexcept;

    CandidateWindow() noexcept = default;
    ~CandidateWindow() noexcept;
    CandidateWindow(const CandidateWindow&) = delete;
    CandidateWindow& operator=(const CandidateWindow&) = delete;

    bool Update(HINSTANCE module, HWND owner, const RECT& anchor,
        const BrokerSnapshot& snapshot, CandidateTheme theme, bool show_comments,
        ActionCallback callback, void* context) noexcept;
    void Hide() noexcept;
    void Destroy() noexcept;

private:
    static LRESULT CALLBACK WindowProc(HWND, UINT, WPARAM, LPARAM) noexcept;
    void Paint() noexcept;
    void Render(HDC dc) noexcept;
    void RefreshShape() noexcept;
    void RefreshAccessibilityState() noexcept;
    int HitTest(LPARAM position) const noexcept;
    void ResetPress() noexcept;

    HINSTANCE module_ = nullptr;
    HWND window_ = nullptr;
    HFONT font_ = nullptr;
    HFONT body_font_ = nullptr;
    HFONT caption_font_ = nullptr;
    struct Row final {
        std::wstring label;
        std::wstring text;
        std::wstring comment;
    };
    std::wstring preedit_;
    std::vector<Row> rows_;
    std::uint64_t revision_ = 0;
    ActionCallback callback_ = nullptr;
    void* context_ = nullptr;
    int row_height_ = 38;
    int header_height_ = 44;
    int footer_height_ = 36;
    int padding_ = 12;
    int gap_ = 4;
    int radius_ = 12;
    int font_dpi_ = 0;
    int visible_rows_ = 0;
    int first_row_ = 0;
    int pressed_item_ = -1;
    std::uint64_t pressed_revision_ = 0;
    CandidateTheme theme_ = CandidateTheme::System;
    bool high_contrast_ = true;
};

}  // namespace mo::windows_tip
