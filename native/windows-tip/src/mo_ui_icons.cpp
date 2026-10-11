#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <algorithm>
#include <cstdint>
#include "mo_ui_icons.h"

namespace mo::windows_tip {
bool DrawUiIcon(HINSTANCE module, HDC dc, UiIcon icon, RECT slot, COLORREF tint) noexcept {
    const HRSRC resource = FindResourceW(module, MAKEINTRESOURCEW(static_cast<int>(icon)), RT_RCDATA);
    const HGLOBAL loaded = resource != nullptr ? LoadResource(module, resource) : nullptr;
    const auto* bytes = loaded != nullptr ? static_cast<const BYTE*>(LockResource(loaded)) : nullptr;
    const std::size_t length = resource != nullptr ? SizeofResource(module, resource) : 0;
    if (bytes == nullptr || length < 1 || slot.right <= slot.left || slot.bottom <= slot.top) { return false; }
    std::size_t offset = 1;
    const BYTE* alpha = nullptr;
    int size = 0;
    for (BYTE i = 0; i < bytes[0]; ++i) {
        if (offset + 6 > length) { return false; }
        const int candidate_size = bytes[offset] | (bytes[offset + 1] << 8);
        const std::uint32_t count = bytes[offset + 2] | (bytes[offset + 3] << 8)
            | (bytes[offset + 4] << 16) | (static_cast<std::uint32_t>(bytes[offset + 5]) << 24);
        offset += 6;
        if (candidate_size <= 0 || candidate_size > 80
            || count != static_cast<std::uint32_t>(candidate_size * candidate_size)
            || count > length - offset) { return false; }
        alpha = bytes + offset; size = candidate_size;
        offset += count;
        if (size >= slot.right - slot.left) { break; }
    }
    if (alpha == nullptr) { return false; }
    BITMAPINFO info{};
    info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    info.bmiHeader.biWidth = size; info.bmiHeader.biHeight = -size;
    info.bmiHeader.biPlanes = 1; info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    void* pixels = nullptr;
    const HDC source = CreateCompatibleDC(dc);
    const HBITMAP bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &pixels, nullptr, 0);
    if (source == nullptr || bitmap == nullptr || pixels == nullptr) {
        if (source != nullptr) { DeleteDC(source); }
        if (bitmap != nullptr) { DeleteObject(bitmap); } return false;
    }
    auto* output = static_cast<BYTE*>(pixels);
    for (int i = 0; i < size * size; ++i) {
        output[i * 4] = static_cast<BYTE>(GetBValue(tint) * alpha[i] / 255);
        output[i * 4 + 1] = static_cast<BYTE>(GetGValue(tint) * alpha[i] / 255);
        output[i * 4 + 2] = static_cast<BYTE>(GetRValue(tint) * alpha[i] / 255);
        output[i * 4 + 3] = alpha[i];
    }
    const HGDIOBJ old = SelectObject(source, bitmap);
    const BLENDFUNCTION blend{AC_SRC_OVER, 0, 255, AC_SRC_ALPHA};
    const bool result = AlphaBlend(dc, slot.left, slot.top, slot.right - slot.left,
        slot.bottom - slot.top, source, 0, 0, size, size, blend) != FALSE;
    SelectObject(source, old); DeleteObject(bitmap); DeleteDC(source);
    return result;
}
}
