#pragma once

#include <windows.h>

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace mo::windows_tip {

enum class CandidateAction : std::uint8_t {
    Select = 0,
    PreviousPage = 1,
    NextPage = 2,
};

struct BrokerSnapshot final {
    std::uint64_t revision = 0;
    bool handled = false;
    std::string composition;
    std::optional<std::string> commit;
    std::vector<std::string> candidates;
};

// Thread-affine client for the versioned Mo broker protocol. Every public
// operation has a caller-supplied hard deadline and resets the connection on
// ambiguous I/O or protocol state.
class BrokerClient final {
public:
    explicit BrokerClient(std::wstring expected_broker_path) noexcept;
    ~BrokerClient() noexcept;

    BrokerClient(const BrokerClient&) = delete;
    BrokerClient& operator=(const BrokerClient&) = delete;

    bool ConnectAndOpen(DWORD timeout_ms, bool retry_missing_endpoint = false) noexcept;
    bool SendKey(
        UINT virtual_key,
        UINT scan_code,
        std::uint16_t modifiers,
        bool key_down,
        bool repeat,
        BrokerSnapshot* snapshot,
        DWORD timeout_ms) noexcept;
    void Close(DWORD timeout_ms) noexcept;
    bool SendCandidateAction(
        std::uint64_t expected_revision,
        CandidateAction action,
        std::uint32_t index,
        BrokerSnapshot* snapshot,
        DWORD timeout_ms) noexcept;
    bool candidate_actions_supported() const noexcept { return candidate_actions_supported_; }

    bool connected() const noexcept { return pipe_ != INVALID_HANDLE_VALUE; }
    std::uint64_t generation() const noexcept { return generation_; }
    std::uint64_t session_token() const noexcept { return session_token_; }

private:
    void Reset() noexcept;

    std::wstring expected_broker_path_;
    HANDLE pipe_ = INVALID_HANDLE_VALUE;
    std::uint64_t generation_ = 0;
    std::uint64_t session_token_ = 0;
    std::uint64_t next_request_id_ = 1;
    bool candidate_actions_supported_ = false;
};

}  // namespace mo::windows_tip
