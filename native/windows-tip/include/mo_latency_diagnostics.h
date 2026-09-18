#pragma once
#ifdef MO_LATENCY_TRACE
#include <windows.h>
#include <unknwn.h>
#include <cstdint>

namespace mo::windows_tip {
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
};
// The caller-owned structure grew: reject the old development IID rather
// than overwrite an old probe's smaller buffer. Not a shipping interface.
MIDL_INTERFACE("B37A59F4-8F18-4D58-90D2-37A516D04197")
IBrokerDiagnostics : public IUnknown {
    virtual HRESULT STDMETHODCALLTYPE ReadLastTiming(BrokerTiming* timing) = 0;
};
}  // namespace mo::windows_tip
#endif
