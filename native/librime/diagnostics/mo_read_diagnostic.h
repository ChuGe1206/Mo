// SPDX-License-Identifier: Apache-2.0
#pragma once
// Isolated synthetic diagnostics only. No addresses, values or input contents.
#include "mo_diagnostic.h"
#include <cstdint>
#include <type_traits>
namespace mo_diagnostic {
inline std::atomic<unsigned long long> read_sequence{0};
inline bool ReadPagesEnabled() noexcept {
    static const bool enabled = [] {
        char value[2]{};
        return GetEnvironmentVariableA("MO_DIAG_READ_PAGES", value, sizeof(value)) == 1
            && value[0] == '1';
    }();
    return enabled;
}
struct ReadPage {
    PSAPI_WORKING_SET_EX_INFORMATION page{};
    bool valid = false;
    bool resident = false;
    bool mapped = false;
    long long query_us = 0;
};
inline ReadPage QueryReadPage(const void* field) noexcept {
    ReadPage result;
    const auto started = std::chrono::steady_clock::now();
    result.page.VirtualAddress = const_cast<void*>(field);
    result.valid = !!QueryWorkingSetEx(GetCurrentProcess(), &result.page, sizeof(result.page));
    result.resident = result.valid && result.page.VirtualAttributes.Valid;
    MEMORY_BASIC_INFORMATION memory{};
    result.mapped = VirtualQuery(field, &memory, sizeof(memory)) == sizeof(memory)
        && memory.State == MEM_COMMIT && memory.Type == MEM_MAPPED;
    result.query_us = std::chrono::duration_cast<std::chrono::microseconds>(
        std::chrono::steady_clock::now() - started).count();
    return result;
}
template <class T>
inline T ReadScalar(const T* field, const char* label) {
    static_assert(std::is_arithmetic_v<T> && sizeof(T) <= 8);
    // Call sites use naturally aligned fields that fit in one Windows page.
    if (deferred_state.active) {
        const auto sample = read_sequence.fetch_add(1, std::memory_order_relaxed);
        if (sample >= 512) { ++deferred_state.read_skipped; return *field; }
        const auto started = std::chrono::steady_clock::now();
        const T value = *static_cast<const volatile T*>(field);
        const auto read_ns = std::chrono::duration_cast<std::chrono::nanoseconds>(
            std::chrono::steady_clock::now() - started).count();
        DeferRead(sample, current ? current->id : 0, label, started, read_ns);
        return value;
    }
    if (!ReadPagesEnabled()) return *field;
    const auto sample = read_sequence.fetch_add(1, std::memory_order_relaxed);
    if (sample >= 512) return *field;
    const auto before = QueryReadPage(field);
    DWORD faults_before = 0, faults_after = 0;
    unsigned long long cycles_before = 0, cycles_after = 0;
    const bool faults_start_valid = Scope::ReadFaults(faults_before);
    const bool cycles_start_valid = Scope::ReadCycles(cycles_before);
    const auto started = std::chrono::steady_clock::now();
    // Force the measured load; do not cache or change the stored value.
    const T value = *static_cast<const volatile T*>(field);
    const auto wall_us = std::chrono::duration_cast<std::chrono::microseconds>(
        std::chrono::steady_clock::now() - started).count();
    const bool cycles_end_valid = Scope::ReadCycles(cycles_after);
    const bool faults_end_valid = Scope::ReadFaults(faults_after);
    const auto after = QueryReadPage(field);
    const bool cycles_valid = cycles_start_valid && cycles_end_valid;
    const bool faults_valid = faults_start_valid && faults_end_valid;
    std::fprintf(stderr,
        "MO_READ sample=%llu parent=%llu label=%s mapped=%d before_valid=%d before_resident=%d after_valid=%d after_resident=%d read_us=%lld query_us=%lld thread_cycles=%llu cycles_valid=%d process_faults=%lu faults_valid=%d\n",
        sample, current ? current->id : 0, label, before.mapped ? 1 : 0,
        before.valid ? 1 : 0, before.resident ? 1 : 0,
        after.valid ? 1 : 0, after.resident ? 1 : 0,
        static_cast<long long>(wall_us), before.query_us + after.query_us,
        cycles_valid ? cycles_after - cycles_before : 0, cycles_valid ? 1 : 0,
        faults_valid ? faults_after - faults_before : 0, faults_valid ? 1 : 0);
    return value;
}
}
