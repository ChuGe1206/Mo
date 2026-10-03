// SPDX-License-Identifier: Apache-2.0
#pragma once
// Mo isolated diagnosis only; Apache-2.0. No key, context, or memory protection changes.
#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <vector>
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <psapi.h>
#pragma comment(lib, "psapi.lib")
namespace mo_diagnostic {
inline bool PrefaultOwnImage() {
    char enabled[2]{};
    if (GetEnvironmentVariableA("MO_DIAG_PREFAULT_IMAGE", enabled, sizeof(enabled)) != 1 || enabled[0] != '1') return true;
    static int module_address;
    HMODULE module = nullptr;
    if (!GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                                                  reinterpret_cast<LPCSTR>(&module_address), &module)) return false;
    MODULEINFO image{};
    if (!GetModuleInformation(GetCurrentProcess(), module, &image, sizeof(image))) return false;
    SYSTEM_INFO system{};
    GetSystemInfo(&system);
    if (!system.dwPageSize || !image.SizeOfImage) return false;
    const auto begin = reinterpret_cast<std::uintptr_t>(image.lpBaseOfDll);
    const auto end = begin + image.SizeOfImage;
    if (end <= begin) return false;
    std::vector<PSAPI_WORKING_SET_EX_INFORMATION> pages;
    size_t readable_bytes = 0, executable_pages = 0;
    for (auto cursor = begin; cursor < end;) {
        MEMORY_BASIC_INFORMATION region{};
        if (!VirtualQuery(reinterpret_cast<LPCVOID>(cursor), &region, sizeof(region))) return false;
        const auto region_begin = reinterpret_cast<std::uintptr_t>(region.BaseAddress);
        const auto region_end = region_begin + region.RegionSize;
        if (region_end <= cursor) return false;
        const auto limit = (std::min)(region_end, end);
        const auto protection = region.Protect & 0xffu;
        const bool readable = protection == PAGE_READONLY || protection == PAGE_READWRITE || protection == PAGE_WRITECOPY ||
            protection == PAGE_EXECUTE_READ || protection == PAGE_EXECUTE_READWRITE || protection == PAGE_EXECUTE_WRITECOPY;
        if (region.State == MEM_COMMIT && region.Type == MEM_IMAGE && region.AllocationBase == module &&
                !(region.Protect & (PAGE_GUARD | PAGE_NOACCESS)) && readable) {
            readable_bytes += limit - cursor;
            for (auto address = cursor; address < limit; address += system.dwPageSize) {
                PSAPI_WORKING_SET_EX_INFORMATION page{};
                page.VirtualAddress = reinterpret_cast<PVOID>(address);
                pages.push_back(page);
                if (protection == PAGE_EXECUTE_READ || protection == PAGE_EXECUTE_READWRITE || protection == PAGE_EXECUTE_WRITECOPY) ++executable_pages;
            }
        }
        cursor = limit;
    }
    if (pages.empty() || pages.size() > MAXDWORD / sizeof(pages[0])) return false;
    const auto buffer_bytes = static_cast<DWORD>(pages.size() * sizeof(pages[0]));
    const bool before_valid = !!QueryWorkingSetEx(GetCurrentProcess(), pages.data(), buffer_bytes);
    size_t resident_before = 0;
    if (before_valid) for (const auto& page : pages) resident_before += page.VirtualAttributes.Valid ? 1 : 0;
    const auto started = std::chrono::steady_clock::now();
    unsigned char checksum = 0;
    for (const auto& page : pages) checksum ^= *static_cast<const volatile unsigned char*>(page.VirtualAddress);
    (void)checksum;
    const auto elapsed = std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::steady_clock::now() - started).count();
    const bool after_valid = !!QueryWorkingSetEx(GetCurrentProcess(), pages.data(), buffer_bytes);
    size_t resident_after = 0;
    if (after_valid) for (const auto& page : pages) resident_after += page.VirtualAttributes.Valid ? 1 : 0;
    std::fprintf(stderr, "MO_DIAG_IMAGE readable_bytes=%zu pages=%zu executable_pages=%zu resident_before=%zu before_valid=%d resident_after=%zu after_valid=%d touch_us=%lld\n",
                              readable_bytes, pages.size(), executable_pages, resident_before, before_valid ? 1 : 0,
                              resident_after, after_valid ? 1 : 0, static_cast<long long>(elapsed));
    return true;
}
}