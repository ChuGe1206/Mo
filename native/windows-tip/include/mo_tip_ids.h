#pragma once

#include <guiddef.h>

namespace mo::windows_tip {

// These Phase 0 identifiers are Mo-owned and must remain stable once packages
// are published. They are intentionally unrelated to Rime/Weasel identifiers.
inline constexpr GUID kTextServiceClsid = {
    0xb4911146,
    0x2a27,
    0x47aa,
    {0x9d, 0x12, 0x10, 0x9b, 0x6a, 0xe1, 0x0a, 0x70},
};

inline constexpr GUID kSimplifiedChineseProfileGuid = {
    0x595a4275,
    0x4c0b,
    0x4d4f,
    {0x80, 0xc3, 0x7d, 0xd3, 0x22, 0xbc, 0x6f, 0x74},
};

inline constexpr wchar_t kProfileDescription[] = L"Mo (墨) 输入法 — Phase 0 shell";

}  // namespace mo::windows_tip

