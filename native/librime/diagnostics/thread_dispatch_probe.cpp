// SPDX-License-Identifier: Apache-2.0
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <cstdio>
#include <cstring>
int main(int argc, char** argv) {
    if (argc != 2 || std::strcmp(argv[1], "--probe")) {
        std::fprintf(stderr, "MO_THREAD_PROBE invalid_arguments\n"); return 2;
    }
    BOOLEAN profiling = FALSE;
    const DWORD query = QueryThreadProfiling(GetCurrentThread(), &profiling);
    if (query != ERROR_SUCCESS || profiling) {
        std::fprintf(stderr, "MO_THREAD_PROBE preflight_error=%lu already_active=%d\n", query, profiling ? 1 : 0);
        return 4;
    }
    HANDLE data = nullptr;
    const DWORD enable = EnableThreadProfiling(GetCurrentThread(), THREAD_PROFILING_FLAG_DISPATCH, 0, &data);
    std::fprintf(stderr, "MO_THREAD_PROBE enable_error=%lu hardware_counters=0\n", enable);
    if (enable != ERROR_SUCCESS) return 3;
    PERFORMANCE_DATA before{}, after{};
    before.Size = sizeof(before); before.Version = PERFORMANCE_DATA_VERSION;
    after.Size = sizeof(after); after.Version = PERFORMANCE_DATA_VERSION;
    const DWORD first = ReadThreadProfilingData(data, READ_THREAD_PROFILING_FLAG_DISPATCHING, &before);
    Sleep(10); // Owned synthetic wait, no input or system-wide trace.
    const DWORD last = ReadThreadProfilingData(data, READ_THREAD_PROFILING_FLAG_DISPATCHING, &after);
    const DWORD disable = DisableThreadProfiling(data);
    profiling = TRUE;
    const DWORD ended = QueryThreadProfiling(GetCurrentThread(), &profiling);
    std::fprintf(stderr, "MO_THREAD_PROBE first_error=%lu last_error=%lu disable_error=%lu final_query_error=%lu final_active=%d context_switches=%lu wait_bitmap=%llu cycles=%llu\n",
        first, last, disable, ended, profiling ? 1 : 0,
        after.ContextSwitchCount - before.ContextSwitchCount,
        static_cast<unsigned long long>(after.WaitReasonBitMap),
        static_cast<unsigned long long>(after.CycleTime - before.CycleTime));
    if (first || last || disable || ended || profiling) return 5;
    if (after.ContextSwitchCount <= before.ContextSwitchCount) return 6;
    std::puts("MO_THREAD_PROBE dispatch_pass");
    return 0;
}
