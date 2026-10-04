// SPDX-License-Identifier: Apache-2.0
#pragma once
// Isolated input-free prefetch experiment; no page touches or residency promise.
#include "mo_diagnostic.h"
#include <array>
#include <cstdint>
#include <cstring>
namespace mo_diagnostic {
struct PrefetchRange {
    PVOID VirtualAddress;
    SIZE_T NumberOfBytes;
};
using PrefetchFunction = BOOL (WINAPI*)(HANDLE, ULONG_PTR, PrefetchRange*, ULONG);
inline bool PrefetchEnabled() noexcept {
    static const bool enabled = [] {
        char value[2]{};
        return GetEnvironmentVariableA("MO_DIAG_PREFETCH_RANGES", value, sizeof(value)) == 1
            && value[0] == '1';
    }();
    return enabled;
}
inline void PrefetchRanges(PrefetchRange* ranges, size_t count, const char* target) noexcept {
    if (!PrefetchEnabled()) return;
    const auto module = GetModuleHandleW(L"kernel32.dll");
    // Copy pointer bytes instead of a warning-prone function-pointer cast.
    PrefetchFunction function = nullptr;
    const auto address = module ? GetProcAddress(module, "PrefetchVirtualMemory") : nullptr;
    static_assert(sizeof(function) == sizeof(address));
    std::memcpy(&function, &address, sizeof(function));
    size_t bytes = 0;
    for (size_t i = 0; i < count; ++i) bytes += ranges[i].NumberOfBytes;
    const auto started = std::chrono::steady_clock::now();
    const BOOL result = function && count ? function(GetCurrentProcess(), count, ranges, 0) : FALSE;
    const DWORD error = result ? ERROR_SUCCESS : (function ? GetLastError() : ERROR_PROC_NOT_FOUND);
    const auto elapsed = std::chrono::duration_cast<std::chrono::microseconds>(
        std::chrono::steady_clock::now() - started).count();
    std::fprintf(stderr, "MO_PREFETCH target=%s ranges=%zu bytes=%zu result=%d error=%lu wall_us=%lld\n",
        target, count, bytes, result ? 1 : 0, error, static_cast<long long>(elapsed));
}
inline void PrefetchMapping(void* address, size_t size) noexcept {
    if (!PrefetchEnabled()) return;
    PrefetchRange range{address, size};
    PrefetchRanges(&range, 1, "mapped");
}
inline void PrefetchImage() noexcept {
    if (!PrefetchEnabled()) return;
    static int module_address;
    HMODULE module = nullptr;
    if (!GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        reinterpret_cast<LPCSTR>(&module_address), &module)) return;
    MODULEINFO image{};
    if (!GetModuleInformation(GetCurrentProcess(), module, &image, sizeof(image))) return;
    const auto begin = reinterpret_cast<std::uintptr_t>(image.lpBaseOfDll);
    const auto end = begin + image.SizeOfImage;
    if (end <= begin) return;
    std::array<PrefetchRange, 64> ranges{};
    size_t count = 0;
    for (auto cursor = begin; cursor < end;) {
        MEMORY_BASIC_INFORMATION region{};
        if (!VirtualQuery(reinterpret_cast<LPCVOID>(cursor), &region, sizeof(region))) return;
        const auto region_end = reinterpret_cast<std::uintptr_t>(region.BaseAddress) + region.RegionSize;
        if (region_end <= cursor) return;
        const auto limit = region_end < end ? region_end : end;
        const auto protection = region.Protect & 0xffu;
        const bool readable = protection == PAGE_READONLY || protection == PAGE_READWRITE || protection == PAGE_WRITECOPY ||
            protection == PAGE_EXECUTE_READ || protection == PAGE_EXECUTE_READWRITE || protection == PAGE_EXECUTE_WRITECOPY;
        if (region.State == MEM_COMMIT && region.Type == MEM_IMAGE && region.AllocationBase == module &&
            !(region.Protect & (PAGE_GUARD | PAGE_NOACCESS)) && readable) {
            if (count == ranges.size()) return;
            ranges[count++] = {reinterpret_cast<PVOID>(cursor), limit - cursor};
        }
        cursor = limit;
    }
    PrefetchRanges(ranges.data(), count, "image");
}
}
