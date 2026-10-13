#pragma once
#include <windows.h>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <string>

namespace mo::windows_tip {
struct CompositionPositions final {
    std::uint32_t cursor_utf8_bytes = 0;
    std::uint32_t selection_start_utf8_bytes = 0;
    std::uint32_t selection_end_utf8_bytes = 0;
};
struct Utf16CompositionPositions final {
    std::size_t cursor_units = 0;
    std::size_t selection_start_units = 0;
    std::size_t selection_end_units = 0;
};
// Prefix conversion rejects out-of-range positions and split UTF-8 scalars.
// Counts UTF-16 code units, including both halves of a supplementary scalar.
inline bool ToUtf16CompositionPositions(const std::string& text,
    const CompositionPositions& positions, Utf16CompositionPositions* output) noexcept {
    if (output == nullptr || text.empty() || text.size() > static_cast<std::size_t>(std::numeric_limits<int>::max())
        || positions.selection_start_utf8_bytes > positions.selection_end_utf8_bytes) { return false; }
    const auto convert = [&text](std::uint32_t offset, std::size_t* units) {
        if (offset > text.size()) { return false; }
        if (offset == 0) { *units = 0; return true; }
        const int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
            static_cast<int>(offset), nullptr, 0);
        if (count <= 0) { return false; }
        *units = static_cast<std::size_t>(count); return true;
    };
    Utf16CompositionPositions checked;
    if (!convert(positions.cursor_utf8_bytes, &checked.cursor_units)
        || !convert(positions.selection_start_utf8_bytes, &checked.selection_start_units)
        || !convert(positions.selection_end_utf8_bytes, &checked.selection_end_units)) { return false; }
    *output = checked; return true;
}
} // namespace mo::windows_tip
