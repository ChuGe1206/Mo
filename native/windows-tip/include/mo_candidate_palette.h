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

inline COLORREF RgbValue(std::uint32_t value) noexcept {
    return RGB((value >> 16) & 255, (value >> 8) & 255, value & 255);
}
inline CandidatePalette ResolveSkinPalette(const BrokerSettings& settings, CandidateTheme theme,
    const CandidatePalette& system, bool high_contrast) noexcept {
    if (high_contrast || settings.skin == 0) { return ResolveCandidatePalette(theme, system, high_contrast); }
    if (settings.skin == 1) { return ResolveCandidatePalette(CandidateTheme::Light, system, false); }
    if (settings.skin == 2) { return ResolveCandidatePalette(CandidateTheme::Dark, system, false); }
    if (settings.skin == 3) {
        return {RGB(240,248,242), RGB(33,61,44), RGB(33,61,44), RGB(40,103,72),
            RGB(255,255,255), RGB(240,248,242), RGB(82,106,89), RGB(199,222,207)};
    }
    if (settings.skin == 4) {
        return {RGB(255,244,239), RGB(74,48,41), RGB(74,48,41), RGB(152,80,63),
            RGB(255,255,255), RGB(255,244,239), RGB(121,92,80), RGB(232,210,199)};
    }
    if (settings.skin == 5) {
        return {RGB(244,244,234), RGB(48,60,42), RGB(48,60,42), RGB(82,97,67),
            RGB(255,255,255), RGB(244,244,234), RGB(96,104,79), RGB(212,218,197)};
    }
    const COLORREF background = RgbValue(settings.background), text = RgbValue(settings.text), accent = RgbValue(settings.accent);
    const int luminance = GetRValue(accent) * 299 + GetGValue(accent) * 587 + GetBValue(accent) * 114;
    return {background, text, text, accent, luminance > 150000 ? RGB(0,0,0) : RGB(255,255,255),
        background, text, text};
}

inline bool IsCandidateAppearanceMessage(UINT message) noexcept {
    return message == WM_SYSCOLORCHANGE || message == WM_SETTINGCHANGE || message == WM_THEMECHANGED;
}

}  // namespace mo::windows_tip
