#pragma once
#include <windows.h>
#include <chrono>

namespace mo::windows_tip {
// MSVC steady_clock uses QPC. No process/global timer-resolution changes.
using DeadlineClock = std::chrono::steady_clock;
using Deadline = DeadlineClock::time_point;
inline Deadline DeadlineFromNow(DWORD timeout_ms) noexcept {
    return DeadlineClock::now() + std::chrono::milliseconds(timeout_ms);
}
inline bool DeadlineExpired(Deadline deadline) noexcept { return DeadlineClock::now() >= deadline; }
inline DWORD RemainingMillisecondsAt(Deadline deadline, Deadline now) noexcept {
    if (now >= deadline) { return 0; }
    const auto remaining = deadline - now;
    // Round upward for the kernel wait, but never turn a finite budget into INFINITE.
    const auto whole = std::chrono::duration_cast<std::chrono::milliseconds>(remaining);
    const auto rounded = whole.count() + (whole < remaining ? 1 : 0);
    return rounded >= MAXDWORD ? MAXDWORD - 1 : static_cast<DWORD>(rounded);
}
inline DWORD RemainingMilliseconds(Deadline deadline) noexcept {
    return RemainingMillisecondsAt(deadline, DeadlineClock::now());
}
}  // namespace mo::windows_tip
