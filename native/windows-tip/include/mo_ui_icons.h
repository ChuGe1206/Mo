#pragma once
#include <windows.h>
namespace mo::windows_tip {
// Exact Figma SVGs rasterized by WPF at seven supported DPI sizes.
enum class UiIcon : int { Previous = 300, Next, Expand, Collapse, Close, Smile };
bool DrawUiIcon(HINSTANCE module, HDC dc, UiIcon icon, RECT slot, COLORREF tint) noexcept;
}
