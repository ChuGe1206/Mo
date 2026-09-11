#pragma once

#include <windows.h>

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace mo::windows_tip {

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
    BrokerClient() noexcept = default;
    ~BrokerClient() noexcept;

    BrokerClient(const BrokerClient&) = delete;
    BrokerClient& operator=(const BrokerClient&) = delete;

    bool ConnectAndOpen(DWORD timeout_ms) noexcept;
    bool SendKey(
        UINT virtual_key,
        UINT scan_code,
        std::uint16_t modifiers,
        bool key_down,
        bool repeat,
        BrokerSnapshot* snapshot,
        DWORD timeout_ms) noexcept;
    void Close(DWORD timeout_ms) noexcept;

    bool connected() const noexcept { return pipe_ != INVALID_HANDLE_VALUE; }
    std::uint64_t generation() const noexcept { return generation_; }
    std::uint64_t session_token() const noexcept { return session_token_; }

private:
    void Reset() noexcept;

    HANDLE pipe_ = INVALID_HANDLE_VALUE;
    std::uint64_t generation_ = 0;
    std::uint64_t session_token_ = 0;
    std::uint64_t next_request_id_ = 1;
};

}  // namespace mo::windows_tip
