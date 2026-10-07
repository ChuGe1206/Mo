// SPDX-License-Identifier: Apache-2.0
#pragma once
#include <atomic>
#include <chrono>
#include <cstdio>
#include <string>
#include <utility>
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <psapi.h>
// Keep the Windows service macro from renaming rime::Service::StartService.
#ifdef StartService
#undef StartService
#endif
#pragma comment(lib, "psapi.lib")
#include "mo_deferred_diagnostic.h"
namespace mo_diagnostic {
struct Scope;
inline thread_local Scope* current = nullptr;
inline std::atomic<unsigned long long> sequence{0};
struct Scope {
    const char* category;
    std::string label;
    Scope* parent;
    unsigned long long id;
    std::chrono::steady_clock::time_point start;
    long long children = 0;
    long long minimum_us = 500;
    unsigned long long cpu_start = 0;
    DWORD faults_start = 0;
    unsigned long long cycles_start = 0;
    bool cycles_valid = false;
    bool deferred = false;
    static bool ReadCycles(unsigned long long& value) {
        ULONG64 cycles = 0;
        if (!QueryThreadCycleTime(GetCurrentThread(), &cycles)) return false;
        value = cycles;
        return true;
    }
    bool cpu_valid = false;
    bool faults_valid = false;
    static bool ReadCpu(unsigned long long& value) {
        FILETIME creation{}, exit{}, kernel{}, user{};
        if (!GetThreadTimes(GetCurrentThread(), &creation, &exit, &kernel, &user)) return false;
        value = ((static_cast<unsigned long long>(kernel.dwHighDateTime) << 32) | kernel.dwLowDateTime) +
                        ((static_cast<unsigned long long>(user.dwHighDateTime) << 32) | user.dwLowDateTime);
        return true;
    }
    static bool ReadFaults(DWORD& value) {
        PROCESS_MEMORY_COUNTERS counters{};
        counters.cb = sizeof(counters);
        if (!GetProcessMemoryInfo(GetCurrentProcess(), &counters, sizeof(counters))) return false;
        value = counters.PageFaultCount;
        return true;
    }
    Scope(const char* c, std::string l, long long minimum = 500) : category(c), label(std::move(l)),
        parent(current), id(++sequence), start(std::chrono::steady_clock::now()) {
        minimum_us = minimum;
        deferred = deferred_state.active;
        if (!deferred) {
            cpu_valid = ReadCpu(cpu_start);
            cycles_valid = ReadCycles(cycles_start);
            faults_valid = ReadFaults(faults_start);
        }
        current = this;
    }
    ~Scope() {
        auto elapsed = std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::steady_clock::now()-start).count();
        if (deferred) {
            current = parent;
            if (parent) parent->children += elapsed;
            if (elapsed >= minimum_us) DeferScope(id, parent ? parent->id : 0,
                category, label.c_str(), start, elapsed, elapsed - children);
            return;
        }
        unsigned long long cpu_end = 0;
        DWORD faults_end = 0;
        unsigned long long cycles_end = 0;
        cpu_valid = ReadCpu(cpu_end) && cpu_valid;
        cycles_valid = ReadCycles(cycles_end) && cycles_valid;
        faults_valid = ReadFaults(faults_end) && faults_valid;
        current = parent;
        if (parent) parent->children += elapsed;
        if (elapsed >= minimum_us) std::fprintf(stderr,
            "MO_COMPONENT id=%llu parent=%llu category=%s label=%s wall_us=%lld self_us=%lld thread_cpu_us=%llu cpu_valid=%d process_faults=%lu faults_valid=%d thread_cycles=%llu cycles_valid=%d\n",
            id, parent ? parent->id : 0, category, label.c_str(), static_cast<long long>(elapsed),
            static_cast<long long>(elapsed-children),
            cpu_valid ? (cpu_end-cpu_start)/10 : 0, cpu_valid ? 1 : 0,
            faults_valid ? faults_end-faults_start : 0, faults_valid ? 1 : 0,
            cycles_valid ? cycles_end-cycles_start : 0, cycles_valid ? 1 : 0);
    }
};
}
