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
};
MIDL_INTERFACE("1DE6A239-4965-487B-A886-212C375F3708")
IBrokerDiagnostics : public IUnknown {
    virtual HRESULT STDMETHODCALLTYPE ReadLastTiming(BrokerTiming* timing) = 0;
};
}  // namespace mo::windows_tip
#endif
