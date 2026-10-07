// SPDX-License-Identifier: Apache-2.0
#define WIN32_LEAN_AND_MEAN
#include "mo_read_diagnostic.h"
#include <cstring>
int main(int argc, char** argv) {
    if (argc != 2 || (std::strcmp(argv[1], "--off") && std::strcmp(argv[1], "--on")
        && std::strcmp(argv[1], "--overflow") && std::strcmp(argv[1], "--dispatch") && std::strcmp(argv[1], "--preprofile"))) {
        std::fprintf(stderr, "MO_DEFER_PROBE invalid_arguments\n"); return 2;
    }
    const bool enabled = std::strcmp(argv[1], "--off") != 0;
    const bool preprofile = std::strcmp(argv[1], "--preprofile") == 0;
    const bool dispatch = std::strcmp(argv[1], "--dispatch") == 0;
    const bool overflow = std::strcmp(argv[1], "--overflow") == 0;
    if (!SetEnvironmentVariableA("MO_DIAG_THREAD_DISPATCH", (dispatch || preprofile) ? "1" : "0")) return 14;
    if (!SetEnvironmentVariableA("MO_DIAG_DEFER_LOGS", enabled ? "1" : "0")
        || !SetEnvironmentVariableA("MO_DIAG_READ_PAGES", "0")) return 3;
    HANDLE existing = nullptr;
    if (preprofile && EnableThreadProfiling(GetCurrentThread(), THREAD_PROFILING_FLAG_DISPATCH, 0, &existing) != ERROR_SUCCESS) return 16;
    SYSTEM_INFO system{}; GetSystemInfo(&system);
    auto* private_page = static_cast<float*>(VirtualAlloc(nullptr, system.dwPageSize,
        MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE));
    HANDLE mapping = CreateFileMappingW(INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE,
        0, system.dwPageSize, nullptr);
    auto* mapped_page = mapping ? static_cast<float*>(MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0)) : nullptr;
    if (!private_page || !mapped_page) return 4;
    *private_page = 2.5f;
    const auto before = mo_diagnostic::QueryReadPage(mapped_page);
    if (!before.valid || before.resident || !before.mapped) return 5;
    {
        mo_diagnostic::DeferredCapture capture;
        if (capture.owner != enabled) return 6;
        mo_diagnostic::Scope all("engine", "ProcessKey", 0);
        {
            mo_diagnostic::DeferredCapture nested;
            if (nested.owner) return 7;
            mo_diagnostic::Scope scope("probe", "nested", 0);
            if (dispatch) Sleep(10);
            if (mo_diagnostic::ReadScalar(private_page, "probe_private") != 2.5f
                || mo_diagnostic::ReadScalar(mapped_page, "probe_mapped") != 0.0f) return 8;
            for (unsigned i = 0; i != 520; ++i) {
                if (mo_diagnostic::ReadScalar(private_page, "probe_limit") != 2.5f) return 9;
            }
        }
        if (overflow) {
            {
                mo_diagnostic::Scope long_label("probe",
                    "synthetic_label_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 0);
            }
            for (unsigned i = 0; i != 2150; ++i) {
                mo_diagnostic::Scope scope("probe", "overflow", 0);
            }
        }
        std::fprintf(stderr, "MO_DEFER_PROBE armed_end\n");
    }
    // A second capture must not replay stale slots or reset the process read cap.
    {
        mo_diagnostic::DeferredCapture capture;
        mo_diagnostic::Scope all("engine", "ProcessKey", 0);
        if (mo_diagnostic::ReadScalar(private_page, "probe_second") != 2.5f) return 10;
        std::fprintf(stderr, "MO_DEFER_PROBE second_end\n");
    }
    BOOLEAN profiling = FALSE;
    if (preprofile) {
        if (QueryThreadProfiling(GetCurrentThread(), &profiling) != ERROR_SUCCESS || !profiling) return 17;
        if (DisableThreadProfiling(existing) != ERROR_SUCCESS) return 18;
    }
    profiling = TRUE;
    if ((dispatch || preprofile) && (QueryThreadProfiling(GetCurrentThread(), &profiling) != ERROR_SUCCESS || profiling)) return 15;
    const auto after = mo_diagnostic::QueryReadPage(mapped_page);
    if (!after.valid || !after.resident || mo_diagnostic::deferred_state.active) return 11;
    if (mo_diagnostic::read_sequence.load() != (enabled ? 523ULL : 0ULL)) return 12;
    if (!UnmapViewOfFile(mapped_page) || !CloseHandle(mapping)
        || !VirtualFree(private_page, 0, MEM_RELEASE)) return 13;
    std::puts(enabled ? "MO_DEFER_PROBE enabled_pass" : "MO_DEFER_PROBE disabled_pass");
    return 0;
}
