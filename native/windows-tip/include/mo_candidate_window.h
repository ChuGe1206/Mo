#pragma once

#include "mo_broker_client.h"
#include <array>

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
        ActionCallback callback, void* context, const BrokerSettings* settings = nullptr,
        InputScheme active_scheme = InputScheme::FullPinyin) noexcept;
    void Hide(bool reset_view = true) noexcept;
    void Destroy() noexcept;
    bool panel_open() const noexcept;
    bool ClosePanel() noexcept;
    void ReportSettingsFailure() noexcept;

private:
    friend struct CandidateWindowFixture; // Synthetic renderer/geometry regression only.
    static LRESULT CALLBACK WindowProc(HWND, UINT, WPARAM, LPARAM) noexcept;
    void Paint() noexcept;
    void Render(HDC dc) noexcept;
    void RefreshShape() noexcept;
    void RefreshAccessibilityState() noexcept;
    int HitTest(LPARAM position) const noexcept;
    void ResetPress() noexcept;
    bool Reflow() noexcept;
    RECT ToolbarRect(int item) const noexcept;
    bool ActionEnabled(int item) const noexcept;

    HINSTANCE module_ = nullptr;
    HWND window_ = nullptr;
    HWND tooltip_ = nullptr;
    bool system_dark_ = false;
    bool preference_error_ = false;
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
    HFONT emoji_font_ = nullptr;
    RECT anchor_{};
    RECT work_{};
    int width_ = 560;
    int view_ = 0; // Compact, expanded candidates, symbols.
    int category_ = 0;
    int compact_count_ = 0;
    int compact_line_count_ = 0;
    int compact_scroll_px_ = 0;
    int compact_height_ = 0;
    int page_label_width_ = 0;
    RECT page_label_rect_{};
    std::array<RECT, 32> compact_label_rects_{};
    std::array<RECT, 32> compact_text_rects_{};
    std::array<RECT, 32> compact_comment_rects_{};
    int symbol_count_ = 0;
    bool symbols_supported_ = false;
    bool quick_settings_supported_ = false;
    BrokerSettings appearance_;
    InputScheme active_scheme_ = InputScheme::FullPinyin;
    std::optional<CandidatePage> page_;
    std::array<RECT, 32> compact_rects_{};
    std::array<RECT, 32> expanded_rects_{};
    std::array<RECT, 28> symbol_rects_{};
    std::array<int, 28> symbol_indices_{};
    int Scale(int value) const noexcept { return MulDiv(value, font_dpi_ > 0 ? font_dpi_ : 96, 96); }
};

}  // namespace mo::windows_tip
