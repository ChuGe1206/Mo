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
    COLORREF border;
};

// If accessibility state cannot be read, prefer the user's system colors.
inline bool UseHighContrastPalette(bool query_succeeded, DWORD flags) noexcept {
    return !query_succeeded || (flags & HCF_HIGHCONTRASTON) != 0;
}

inline CandidatePalette ResolveCandidatePalette(CandidateTheme theme,
    const CandidatePalette& system, bool high_contrast) noexcept {
    if (high_contrast) {
        return {system.background, system.text, system.text,
            system.pressed, system.pressed_text, system.background, system.text, system.border};
    }
    if (theme == CandidateTheme::Dark) {
        return {RGB(36, 43, 41), RGB(239, 245, 241), RGB(239, 245, 241),
            RGB(154, 218, 197), RGB(21, 53, 44), RGB(36, 43, 41), RGB(176, 190, 181),
            RGB(76, 89, 82)};
    }
    if (theme == CandidateTheme::Light) {
        return {RGB(255, 255, 255), RGB(37, 53, 47), RGB(37, 53, 47),
            RGB(23, 109, 94), RGB(255, 255, 255), RGB(255, 255, 255), RGB(98, 111, 104),
            RGB(224, 230, 226)};
    }
    return system;
}

inline bool IsCandidateAppearanceMessage(UINT message) noexcept {
    return message == WM_SYSCOLORCHANGE || message == WM_SETTINGCHANGE || message == WM_THEMECHANGED;
}

}  // namespace mo::windows_tip
