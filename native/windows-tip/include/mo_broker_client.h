#pragma once

#include <windows.h>

#include <cstdint>
#include <optional>
#include <string>
#include <vector>
#include "mo_latency_diagnostics.h"
#include "mo_deadline.h"
#include "mo_preedit_positions.h"

namespace mo::windows_tip {

enum class CandidateAction : std::uint8_t {
    Select = 0,
    PreviousPage = 1,
    NextPage = 2,
    InsertSymbol = 3,
    ToggleEmoji = 4,
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
    std::uint8_t skin = 0;
    std::uint32_t background = 0xffffff;
    std::uint32_t accent = 0x176d5e;
    std::uint32_t text = 0x25352f;
    std::uint8_t radius = 12;
    std::uint8_t font_size = 16;
    bool vertical = false;
};

enum class SettingsRefreshResult : std::uint8_t {
    Updated,
    Unavailable,
    Unsupported,
    Disconnected,
};

enum class SessionReplaceResult : std::uint8_t {
    Replaced,
    RejectedKeepOld,
    Disconnected,
};

struct CandidatePage final {
    std::uint32_t number = 0;
    bool has_previous = false;
    bool has_next = false;
    std::optional<std::uint8_t> highlighted;
};
struct BrokerSnapshot final {
    // Absent on old negotiated protocols; never infer mode from a key or preedit.
    std::optional<bool> ascii_mode;
    bool engine_disabled = false;
    std::uint64_t revision = 0;
    bool handled = false;
    std::optional<CandidatePage> page;
    bool symbols_supported = false;
    bool quick_settings_supported = false;
    std::string composition;
    std::optional<CompositionPositions> positions;
    std::optional<std::string> commit;
    std::vector<std::string> candidates;
    std::vector<std::optional<std::string>> candidate_comments;
    std::vector<std::optional<std::string>> candidate_labels;
};

struct BrokerInputMode final {
    std::uint64_t revision;
    bool ascii_mode;
    bool disabled;
    bool composing;
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
    bool input_mode_supported() const noexcept { return input_mode_supported_; }
    const std::optional<BrokerInputMode>& input_mode() const noexcept { return input_mode_; }
    bool SetInputMode(bool ascii_mode, std::uint64_t expected_revision, DWORD timeout_ms) noexcept;
    bool SetEmoji(std::uint64_t candidate_revision, bool value, DWORD timeout_ms) noexcept;
    SettingsRefreshResult RefreshSettings(DWORD timeout_ms) noexcept;
    // Open the next configured session before retiring the old one. A stable
    // Broker rejection keeps the old session alive; ambiguous transport drops
    // the whole connection so no token can be replayed.
    SessionReplaceResult ReplaceSession(DWORD timeout_ms) noexcept;
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
    bool RequestInputMode(std::optional<bool> ascii_mode, std::uint64_t revision, Deadline deadline) noexcept;
    void RememberInputMode(const BrokerSnapshot& snapshot) noexcept;

    std::wstring expected_broker_path_;
    HANDLE pipe_ = INVALID_HANDLE_VALUE;
    std::uint64_t generation_ = 0;
    std::uint64_t session_token_ = 0;
    std::uint64_t next_request_id_ = 1;
    bool candidate_actions_supported_ = false;
    bool candidate_details_supported_ = false;
    bool candidate_presentation_supported_ = false;
    bool composition_positions_supported_ = false;
    bool input_mode_supported_ = false;
    std::optional<BrokerInputMode> input_mode_;
    bool symbols_supported_ = false;
    bool presentation_settings_supported_ = false;
    bool quick_settings_supported_ = false;
    bool settings_supported_ = false;
    bool session_settings_ack_supported_ = false;
    BrokerSettings settings_;
    DWORD last_connect_error_ = ERROR_SUCCESS;
#ifdef MO_LATENCY_TRACE
    BrokerTiming last_timing_;
#endif
};

}  // namespace mo::windows_tip
