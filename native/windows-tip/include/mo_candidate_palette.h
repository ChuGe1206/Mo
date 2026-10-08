#pragma once

#include "mo_broker_client.h"

namespace mo::windows_tip {

struct CandidatePalette final {
    COLORREF background;
    COLORREF preedit;
    COLORREF text;
    COLORREF pressed;
    COLORREF pressed_text;
    COLORREF footer;
    COLORREF footer_text;
};

// If accessibility state cannot be read, prefer the user's system colors.
inline bool UseHighContrastPalette(bool query_succeeded, DWORD flags) noexcept {
    return !query_succeeded || (flags & HCF_HIGHCONTRASTON) != 0;
}

inline CandidatePalette ResolveCandidatePalette(CandidateTheme theme,
    const CandidatePalette& system, bool high_contrast) noexcept {
    if (high_contrast) {
        return {system.background, system.text, system.text,
            system.pressed, system.pressed_text, system.background, system.text};
    }
    if (theme == CandidateTheme::Dark) {
        return {RGB(32, 32, 34), RGB(190, 190, 194), RGB(245, 245, 247),
            RGB(54, 72, 92), RGB(245, 245, 247), RGB(43, 43, 46), RGB(214, 214, 218)};
    }
    if (theme == CandidateTheme::Light) {
        return {RGB(250, 250, 248), RGB(80, 80, 80), RGB(25, 25, 25),
            RGB(220, 232, 245), RGB(25, 25, 25), RGB(238, 238, 235), RGB(70, 70, 70)};
    }
    return system;
}

inline bool IsCandidateAppearanceMessage(UINT message) noexcept {
    return message == WM_SYSCOLORCHANGE || message == WM_SETTINGCHANGE || message == WM_THEMECHANGED;
}

}  // namespace mo::windows_tip
