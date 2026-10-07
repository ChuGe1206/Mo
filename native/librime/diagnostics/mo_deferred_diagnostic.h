// SPDX-License-Identifier: Apache-2.0
#pragma once
// Synthetic diagnostics only. Fixed storage; no I/O or page queries while armed.
#include <array>
#include <chrono>
#include <cstdio>
#include <cstring>
#include <windows.h>
#include "mo_thread_dispatch_diagnostic.h"
namespace mo_diagnostic {
inline bool DeferredEnabled() noexcept {
    static const bool enabled = [] {
        char value[2]{};
        return GetEnvironmentVariableA("MO_DIAG_DEFER_LOGS", value, sizeof(value)) == 1
            && value[0] == '1';
    }();
    return enabled;
}
struct DeferredScopeRecord {
    unsigned long long id = 0, parent = 0;
    long long start_us = 0, wall_us = 0, self_us = 0;
    char category[48]{}, label[96]{};
    bool truncated = false;
};
struct DeferredReadRecord {
    unsigned long long sample = 0, parent = 0;
    long long start_ns = 0, read_ns = 0;
    char label[48]{};
    bool truncated = false;
};
struct DeferredState {
    bool active = false;
    std::chrono::steady_clock::time_point started{};
    size_t scopes = 0, reads = 0;
    unsigned long long scope_dropped = 0, read_skipped = 0;
    std::array<DeferredScopeRecord, 2048> scope_records{};
    std::array<DeferredReadRecord, 512> read_records{};
};
inline thread_local DeferredState deferred_state;
template <size_t N>
inline bool CopyDiagnosticLabel(char (&target)[N], const char* source) noexcept {
    const auto size = std::strlen(source);
    const auto count = size < N ? size : N - 1;
    std::memcpy(target, source, count);
    target[count] = '\0';
    return size >= N;
}
inline void DeferScope(unsigned long long id, unsigned long long parent,
    const char* category, const char* label, std::chrono::steady_clock::time_point start,
    long long wall_us, long long self_us) noexcept {
    auto& state = deferred_state;
    if (state.scopes == state.scope_records.size()) { ++state.scope_dropped; return; }
    auto& record = state.scope_records[state.scopes++];
    record.id = id; record.parent = parent;
    record.start_us = std::chrono::duration_cast<std::chrono::microseconds>(start - state.started).count();
    record.wall_us = wall_us; record.self_us = self_us;
    record.truncated = CopyDiagnosticLabel(record.category, category);
    record.truncated = CopyDiagnosticLabel(record.label, label) || record.truncated;
}
inline void DeferRead(unsigned long long sample, unsigned long long parent, const char* label,
    std::chrono::steady_clock::time_point start, long long read_ns) noexcept {
    auto& state = deferred_state;
    if (state.reads == state.read_records.size()) { ++state.read_skipped; return; }
    auto& record = state.read_records[state.reads++];
    record.sample = sample; record.parent = parent;
    record.start_ns = std::chrono::duration_cast<std::chrono::nanoseconds>(start - state.started).count();
    record.read_ns = read_ns;
    record.truncated = CopyDiagnosticLabel(record.label, label);
}
struct DeferredCapture {
    bool owner = false;
    ThreadDispatchSnapshot thread;
    long long arm_us = 0;
    DeferredCapture() noexcept {
        if (!DeferredEnabled() || deferred_state.active) return;
        const auto arm_started = std::chrono::steady_clock::now();
        auto& state = deferred_state;
        state.scopes = 0; state.reads = 0;
        state.scope_dropped = 0; state.read_skipped = 0;
        state.active = true; owner = true;
        thread.Begin();
        state.started = std::chrono::steady_clock::now();
        arm_us = std::chrono::duration_cast<std::chrono::microseconds>(state.started - arm_started).count();
    }
    DeferredCapture(const DeferredCapture&) = delete;
    DeferredCapture& operator=(const DeferredCapture&) = delete;
    ~DeferredCapture() noexcept {
        if (!owner) return;
        auto& state = deferred_state;
        const auto ended = std::chrono::steady_clock::now();
        const auto capture_us = std::chrono::duration_cast<std::chrono::microseconds>(ended - state.started).count();
        state.active = false;
        thread.End();
        for (size_t i = 0; i != state.scopes; ++i) {
            const auto& r = state.scope_records[i];
            std::fprintf(stderr, "MO_DEFER_SCOPE id=%llu parent=%llu category=%s label=%s start_us=%lld wall_us=%lld self_us=%lld truncated=%d\n",
                r.id, r.parent, r.category, r.label, r.start_us, r.wall_us, r.self_us, r.truncated ? 1 : 0);
        }
        for (size_t i = 0; i != state.reads; ++i) {
            const auto& r = state.read_records[i];
            std::fprintf(stderr, "MO_DEFER_READ sample=%llu parent=%llu label=%s start_ns=%lld read_ns=%lld truncated=%d\n",
                r.sample, r.parent, r.label, r.start_ns, r.read_ns, r.truncated ? 1 : 0);
        }
        thread.Print();
        const auto flush_us = std::chrono::duration_cast<std::chrono::microseconds>(
            std::chrono::steady_clock::now() - ended).count();
        std::fprintf(stderr, "MO_DEFER_FLUSH scopes=%zu reads=%zu scope_dropped=%llu read_skipped=%llu arm_us=%lld capture_us=%lld flush_us=%lld\n",
            state.scopes, state.reads, state.scope_dropped, state.read_skipped, arm_us, capture_us, flush_us);
    }
};
}
