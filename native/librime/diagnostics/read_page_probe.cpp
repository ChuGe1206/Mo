// SPDX-License-Identifier: Apache-2.0
#define WIN32_LEAN_AND_MEAN
#ifndef NOMINMAX
#define NOMINMAX
#endif
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0602
#endif
#include <windows.h>
#include "mo_read_diagnostic.h"
#include "mo_prefetch_diagnostic.h"
static_assert(sizeof(mo_diagnostic::PrefetchRange) == sizeof(WIN32_MEMORY_RANGE_ENTRY));
static_assert(offsetof(mo_diagnostic::PrefetchRange, NumberOfBytes) == offsetof(WIN32_MEMORY_RANGE_ENTRY, NumberOfBytes));
#include <cstdio>
#include <cstring>
int main(int argc, char** argv) {
    if (argc != 2 || (std::strcmp(argv[1], "--trace-on") && std::strcmp(argv[1], "--trace-off") && std::strcmp(argv[1], "--prefetch-on"))) {
        std::fprintf(stderr, "MO_READ_PROBE invalid_arguments\n");
        return 2;
    }
    const bool enabled = !std::strcmp(argv[1], "--trace-on");
    const bool prefetch = !std::strcmp(argv[1], "--prefetch-on");
    if (!SetEnvironmentVariableA("MO_DIAG_PREFETCH_RANGES", prefetch ? "1" : "0")) return 11;
    if (!SetEnvironmentVariableA("MO_DIAG_READ_PAGES", enabled ? "1" : "0")) return 3;
    SYSTEM_INFO system{};
    GetSystemInfo(&system);
    auto* private_page = static_cast<float*>(VirtualAlloc(nullptr, system.dwPageSize,
        MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE));
    HANDLE mapping = CreateFileMappingW(INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE,
        0, system.dwPageSize, nullptr);
    auto* mapped_page = mapping ? static_cast<float*>(MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0)) : nullptr;
    if (!private_page || !mapped_page) return 4;
    *private_page = 2.5f;
    const auto private_before = mo_diagnostic::QueryReadPage(private_page);
    const auto mapped_before = mo_diagnostic::QueryReadPage(mapped_page);
    if (!private_before.valid || !private_before.resident || private_before.mapped
        || !mapped_before.valid || mapped_before.resident || !mapped_before.mapped) return 5;
    if (prefetch) {
        mo_diagnostic::PrefetchMapping(mapped_page, system.dwPageSize);
        const auto hinted = mo_diagnostic::QueryReadPage(mapped_page);
        if (!hinted.valid || hinted.resident) return 12;
    }
    {
        mo_diagnostic::Scope scope("read_probe", "synthetic", 0);
        if (mo_diagnostic::ReadScalar(private_page, "probe_private") != 2.5f
            || mo_diagnostic::ReadScalar(mapped_page, "probe_mapped") != 0.0f) return 6;
        for (unsigned i = 0; i != 520; ++i) {
            if (mo_diagnostic::ReadScalar(private_page, "probe_limit") != 2.5f) return 7;
        }
    }
    const auto mapped_after = mo_diagnostic::QueryReadPage(mapped_page);
    if (!mapped_after.valid || !mapped_after.resident) return 8;
    if (mo_diagnostic::read_sequence.load() != (enabled ? 522ULL : 0ULL)) return 9;
    if (!UnmapViewOfFile(mapped_page) || !CloseHandle(mapping)
        || !VirtualFree(private_page, 0, MEM_RELEASE)) return 10;
    std::puts(prefetch ? "MO_READ_PROBE prefetch_on_pass" : (enabled ? "MO_READ_PROBE trace_on_pass" : "MO_READ_PROBE trace_off_pass"));
    return 0;
}
