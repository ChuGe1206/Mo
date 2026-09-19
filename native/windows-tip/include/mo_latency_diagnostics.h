#pragma once
#ifdef MO_LATENCY_TRACE
#include <windows.h>
#include <unknwn.h>
#include <cstdint>

namespace mo::windows_tip {
constexpr DWORD kTerminationFrameCapacity = 24;
struct TerminationFrame final {
    // 0=unknown, 1=Mo TIP, 2=msctf, 3=user32, 4=ntdll, 5=combase,
    // 6=imm32, 7=win32u, 8=kernelbase, 9=kernel32, 10=probe,
    // 11=textinputframework, 12=msctfmonitor, 13=msutb, 14=ole32,
    // 15=rpcrt4, 16=ucrtbase, 17=vcruntime140, 18=vcruntime140_1.
    DWORD module = 0;
    std::uint64_t rva = 0;
};
// Development-only, read-only metadata. No text, key values or session tokens.
struct BrokerTiming final {
    std::uint64_t total_us = 0, write_us = 0, header_us = 0, payload_us = 0, cancel_us = 0;
    std::uint64_t request_id = 0;
    DWORD kind = 0, phase = 0, error = 0;
    DWORD candidate_stage = 0, candidate_count = 0, candidate_focus = 0;
    HRESULT candidate_result = S_OK;
    DWORD candidate_snapshot = 0, candidate_reset = 0;
    std::uint64_t candidate_reset_count = 0;
    HRESULT edit_request = S_OK, edit_session = S_OK;
    DWORD termination_owner_active = 0, termination_sent = 0, termination_owner_foreground = 0;
    std::uint64_t dispatch_total_us = 0, dispatch_pre_send_us = 0, dispatch_connect_us = 0, dispatch_modifiers_us = 0;
    std::uint64_t termination_notification_count = 0;
    DWORD termination_frame_count = 0;
    TerminationFrame termination_frames[kTerminationFrameCapacity]{};
};
// The caller-owned structure grew: reject the old development IID rather
// than overwrite an old probe's smaller buffer. Not a shipping interface.
MIDL_INTERFACE("4EAD6830-8CB1-4C04-A703-8F1E90990822")
IBrokerDiagnostics : public IUnknown {
    virtual HRESULT STDMETHODCALLTYPE ReadLastTiming(BrokerTiming* timing) = 0;
};
}  // namespace mo::windows_tip
#endif
