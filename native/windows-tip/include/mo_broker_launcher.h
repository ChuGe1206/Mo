#pragma once

#include <windows.h>

#include <string>

namespace mo::windows_tip {

struct BrokerLocation final {
    std::wstring path;
    // Only the exact Program Files/Mo product layout is allowed to create a
    // process. Development and relocated probe layouts remain connect-only.
    bool auto_start = false;
};

BrokerLocation ResolveBrokerLocation(HMODULE tip_module) noexcept;
BrokerLocation ResolveBrokerLocationFromPath(const std::wstring& tip_path) noexcept;

// Starts the exact image with no arguments, shell, inherited handles or
// console window. The caller decides whether the resolved layout authorizes
// startup; this lower-level primitive remains independently probeable.
bool StartBrokerProcess(const std::wstring& broker_path, DWORD* process_id = nullptr) noexcept;

}  // namespace mo::windows_tip
