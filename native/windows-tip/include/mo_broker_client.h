#pragma once

#include <windows.h>

#include <cstdint>
#include <optional>
#include <string>
#include <vector>
#include "mo_latency_diagnostics.h"
#include "mo_deadline.h"

namespace mo::windows_tip {

enum class CandidateAction : std::uint8_t {
    Select = 0,
    PreviousPage = 1,
    NextPage = 2,
};

enum class InputScheme : std::uint8_t {
    FullPinyin = 0,
    DoublePinyinNatural = 1,
    DoublePinyinFlypy = 2,
    DoublePinyinMicrosoft = 3,
    DoublePinyinSogou = 4,
};

enum class CharacterSet : std::uint8_t { Simplified = 0, Traditional = 1 };
enum class CandidateTheme : std::uint8_t { System = 0, Light = 1, Dark = 2 };

struct BrokerSettings final {
    std::uint64_t revision = 1;
    bool stored = false;
    InputScheme input_scheme = InputScheme::FullPinyin;
    CharacterSet character_set = CharacterSet::Simplified;
    std::uint8_t candidate_page_size = 5;
    CandidateTheme theme = CandidateTheme::System;
    bool show_comments = true;
    bool emoji = true;
    bool local_learning = true;
    bool privacy_mode = false;
    bool effective_learning = true;
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
    // Compose reconnect + key under the SAME absolute budget; never round and
    // start a fresh per-stage timeout on the input hot path.
    bool ConnectAndOpenUntil(Deadline deadline, bool retry_missing_endpoint = false) noexcept;
    bool SendKey(
        UINT virtual_key,
        UINT scan_code,
        std::uint16_t modifiers,
        bool key_down,
        bool repeat,
        BrokerSnapshot* snapshot,
        DWORD timeout_ms) noexcept;
    bool SendKeyUntil(
        UINT virtual_key, UINT scan_code, std::uint16_t modifiers,
        bool key_down, bool repeat, BrokerSnapshot* snapshot, Deadline deadline) noexcept;
    void Close(DWORD timeout_ms) noexcept;
    bool SendCandidateAction(
        std::uint64_t expected_revision,
        CandidateAction action,
        std::uint32_t index,
        BrokerSnapshot* snapshot,
        DWORD timeout_ms) noexcept;
    bool candidate_actions_supported() const noexcept { return candidate_actions_supported_; }
    bool RefreshSettings(DWORD timeout_ms) noexcept;
    const BrokerSettings& settings() const noexcept { return settings_; }

    bool connected() const noexcept { return pipe_ != INVALID_HANDLE_VALUE; }
    DWORD last_connect_error() const noexcept { return last_connect_error_; }
    std::uint64_t generation() const noexcept { return generation_; }
    std::uint64_t session_token() const noexcept { return session_token_; }
#ifdef MO_LATENCY_TRACE
    BrokerTiming last_timing() const noexcept { return last_timing_; }
#endif

private:
    void Reset() noexcept;

    std::wstring expected_broker_path_;
    HANDLE pipe_ = INVALID_HANDLE_VALUE;
    std::uint64_t generation_ = 0;
    std::uint64_t session_token_ = 0;
    std::uint64_t next_request_id_ = 1;
    bool candidate_actions_supported_ = false;
    bool settings_supported_ = false;
    BrokerSettings settings_;
    DWORD last_connect_error_ = ERROR_SUCCESS;
#ifdef MO_LATENCY_TRACE
    BrokerTiming last_timing_;
#endif
};

}  // namespace mo::windows_tip
