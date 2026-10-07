// SPDX-License-Identifier: Apache-2.0
#pragma once
#include <cstdio>
#include <windows.h>
namespace mo_diagnostic {
// Only the current owned synthetic thread; no hardware counters or ETW session.
struct ThreadDispatchSnapshot {
    bool requested = false;
    HANDLE handle = nullptr;
    DWORD enable_error = ERROR_NOT_READY, first_error = ERROR_NOT_READY;
    DWORD last_error = ERROR_NOT_READY, disable_error = ERROR_NOT_READY;
    PERFORMANCE_DATA before{}, after{};
    void Begin() noexcept {
        char value[2]{};
        requested = GetEnvironmentVariableA("MO_DIAG_THREAD_DISPATCH", value, sizeof(value)) == 1
            && value[0] == '1';
        if (!requested) return;
        BOOLEAN active = FALSE;
        enable_error = QueryThreadProfiling(GetCurrentThread(), &active);
        if (enable_error != ERROR_SUCCESS) return;
        if (active) { enable_error = ERROR_ALREADY_EXISTS; return; }
        enable_error = EnableThreadProfiling(GetCurrentThread(), THREAD_PROFILING_FLAG_DISPATCH, 0, &handle);
        if (enable_error != ERROR_SUCCESS) { handle = nullptr; return; }
        before.Size = sizeof(before); before.Version = PERFORMANCE_DATA_VERSION;
        first_error = ReadThreadProfilingData(handle, READ_THREAD_PROFILING_FLAG_DISPATCHING, &before);
    }
    void End() noexcept {
        if (!handle) return;
        after.Size = sizeof(after); after.Version = PERFORMANCE_DATA_VERSION;
        last_error = ReadThreadProfilingData(handle, READ_THREAD_PROFILING_FLAG_DISPATCHING, &after);
        disable_error = DisableThreadProfiling(handle);
        handle = nullptr;
    }
    void Print() const noexcept {
        if (!requested) return;
        const bool valid = !enable_error && !first_error && !last_error && !disable_error;
        std::fprintf(stderr, "MO_DEFER_THREAD enable_error=%lu first_error=%lu last_error=%lu disable_error=%lu valid=%d context_switches=%lu wait_bitmap=%llu cycles=%llu\n",
            enable_error, first_error, last_error, disable_error, valid ? 1 : 0,
            valid ? after.ContextSwitchCount - before.ContextSwitchCount : 0,
            valid ? static_cast<unsigned long long>(after.WaitReasonBitMap) : 0,
            valid ? static_cast<unsigned long long>(after.CycleTime - before.CycleTime) : 0);
    }
};
}
