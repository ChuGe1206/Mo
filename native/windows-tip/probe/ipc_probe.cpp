#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <algorithm>
#include <array>
#include <iostream>
#include <memory>
#include <string>
#include <utility>

#include "mo_broker_client.h"

namespace {

bool ProbeRejectsUnexpectedServerImage() {
    std::wstring own_path(32768, L'\0');
    const DWORD length = GetModuleFileNameW(
        nullptr,
        own_path.data(),
        static_cast<DWORD>(own_path.size()));
    if (length == 0 || length >= own_path.size()) {
        std::wcerr << L"GetModuleFileNameW failed while preparing identity probe\n";
        return false;
    }
    own_path.resize(length);

    mo::windows_tip::BrokerClient wrong_identity(std::move(own_path));
    if (wrong_identity.ConnectAndOpen(500)) {
        std::wcerr << L"BrokerClient accepted an unexpected server image\n";
        return false;
    }
    return true;
}

bool ProbeExpiredKeyBudget(const wchar_t* expected_broker) {
    mo::windows_tip::BrokerClient broker(expected_broker);
    if (broker.ConnectAndOpen(0) || broker.connected() || !broker.ConnectAndOpen(2000)) { return false; }
    mo::windows_tip::BrokerSnapshot untouched;
    untouched.revision = 987;
    untouched.composition = "sentinel";
    untouched.commit = "sentinel";
    if (broker.SendKey(VK_SPACE, 0, 0, true, false, &untouched, 0) || broker.connected()
        || untouched.revision != 987 || untouched.composition != "sentinel" || untouched.commit != "sentinel") {
        std::wcerr << L"Expired key budget accepted a reply or published a snapshot\n"; return false;
    }
    // The following frontend session is independent, with no old composition.
    const auto spent_deadline = mo::windows_tip::DeadlineClock::now();
    if (!broker.ConnectAndOpenUntil(mo::windows_tip::DeadlineFromNow(2000))
        || broker.SendKeyUntil(VK_SPACE, 0, 0, true, false, &untouched, spent_deadline)
        || broker.connected() || untouched.composition != "sentinel" || untouched.commit != "sentinel") {
        std::wcerr << L"Absolute key deadline was restarted after connecting\n"; return false;
    }
    const auto shared_deadline = mo::windows_tip::DeadlineFromNow(2000);
    if (!broker.ConnectAndOpenUntil(shared_deadline)
        || !broker.SendKeyUntil('Z', 0, 0, true, false, &untouched, shared_deadline)
        || untouched.composition != "z" || untouched.commit.has_value()) { return false; }
    broker.Close(500);
    return true;
}

bool ProbePool(const wchar_t* expected_broker, bool real_rime) {
    constexpr std::size_t count = 16;
    std::vector<std::unique_ptr<mo::windows_tip::BrokerClient>> peers;
    std::vector<std::uint64_t> revisions;
    std::vector<std::string> selected_texts;
    std::vector<std::uint32_t> selected_ordinals;
    const std::array<std::string, 4> inputs = {"NIHAO", "ZHONGGUO", "SHIJIE", "SHURU"};
    const std::array<std::string, 4> words = {u8"你好", u8"中国", u8"世界", u8"输入"};
    std::uint64_t last_revision = 0;
    for (std::size_t index = 0; index < count; ++index) {
        auto peer = std::make_unique<mo::windows_tip::BrokerClient>(expected_broker);
        if (!peer->ConnectAndOpen(2000)) {
            std::wcerr << L"Could not open pool client " << index << L'\n'; return false;
        }
        // All earlier connections stay open; a single-instance Broker fails
        // here before any candidate assertions can pass.
        mo::windows_tip::BrokerSnapshot snapshot;
        const std::string input = real_rime ? inputs[index % inputs.size()]
            : std::string(1, static_cast<char>('A' + index));
        for (const unsigned char key : input) {
            if (!peer->SendKey(key, 0, 0, true, false, &snapshot, 2000)) { return false; }
        }
        if (!snapshot.handled || snapshot.commit.has_value() || snapshot.revision <= last_revision) { return false; }
        const std::string expected = real_rime ? words[index % words.size()] : input;
        const auto candidate = std::find(snapshot.candidates.begin(), snapshot.candidates.end(), expected);
        if (candidate == snapshot.candidates.end()) {
            std::cerr << "Missing pool candidate: " << expected << '\n'; return false;
        }
        last_revision = snapshot.revision;
        revisions.push_back(snapshot.revision);
        selected_texts.push_back(expected);
        selected_ordinals.push_back(static_cast<std::uint32_t>(candidate - snapshot.candidates.begin()));
        peers.push_back(std::move(peer));
    }
    mo::windows_tip::BrokerClient overflow(expected_broker);
    const ULONGLONG started = GetTickCount64();
    if (overflow.ConnectAndOpen(40) || overflow.connected() || GetTickCount64() - started > 1000) {
        std::wcerr << L"Saturated pool did not fail within a bounded deadline\n"; return false;
    }
    // Free a middle slot while all other clients remain live. The new session
    // must start empty, not inherit that client's preedit.
    peers[7]->Close(500);
    auto replacement = std::make_unique<mo::windows_tip::BrokerClient>(expected_broker);
    mo::windows_tip::BrokerSnapshot fresh;
    if (!replacement->ConnectAndOpen(2000)
        || !replacement->SendKey('Z', 0, 0, true, false, &fresh, 2000)
        || fresh.composition != "z" || fresh.commit.has_value() || fresh.revision <= last_revision) {
        std::wcerr << L"Pool slot did not reset/reuse correctly\n"; return false;
    }
    last_revision = fresh.revision;
    for (std::size_t index = 0; index < peers.size(); ++index) {
        if (index == 7) { continue; }
        mo::windows_tip::BrokerSnapshot selected;
        if (!peers[index]->SendCandidateAction(revisions[index], mo::windows_tip::CandidateAction::Select,
                selected_ordinals[index], &selected, 2000)
            || selected.commit != selected_texts[index] || selected.revision <= last_revision) {
            std::wcerr << L"Concurrent candidate page was not isolated for client " << index << L'\n'; return false;
        }
        last_revision = selected.revision;
    }
    for (auto& peer : peers) { peer->Close(500); }
    replacement->Close(500);
    // Refill every slot several times, not just the middle slot. Verify fresh
    // compositions and exactly one commit after prior saturated connections.
    for (std::size_t round = 0; round < 3; ++round) {
        peers.clear();
        for (std::size_t index = 0; index < count; ++index) {
            auto peer = std::make_unique<mo::windows_tip::BrokerClient>(expected_broker);
            if (!peer->ConnectAndOpen(2000)) { return false; }
            mo::windows_tip::BrokerSnapshot snapshot;
            const std::string input = real_rime ? "NIHAO" : "M";
            bool first_key = true;
            for (const unsigned char key : input) {
                if (!peer->SendKey(key, 0, 0, true, false, &snapshot, 2000)) { return false; }
                if (first_key && (snapshot.composition != (real_rime ? "n" : "m") || snapshot.commit.has_value())) {
                    std::cerr << "Recovered session inherited preedit: " << snapshot.composition << '\n'; return false;
                }
                first_key = false;
            }
            if (snapshot.commit.has_value() || snapshot.revision <= last_revision
                || snapshot.composition.empty()) {
                std::wcerr << L"Recovered composition failed at round/client " << round << L'/' << index << L'\n'; return false;
            }
            if (!peer->SendKey(VK_SPACE, 0, 0, true, false, &snapshot, 2000)
                || snapshot.commit != (real_rime ? u8"你好" : "m")
                || !snapshot.composition.empty() || snapshot.revision <= last_revision) {
                std::wcerr << L"Recovered commit failed at round/client " << round << L'/' << index << L'\n'; return false;
            }
            last_revision = snapshot.revision;
            if (!peer->SendKey(VK_SPACE, 0, 0, true, false, &snapshot, 2000)
                || snapshot.commit == (real_rime ? u8"你好" : "m")
                || !snapshot.composition.empty() || snapshot.revision <= last_revision) {
                std::wcerr << L"Recovered session replayed commit at round/client " << round << L'/' << index << L'\n'; return false;
            }
            last_revision = snapshot.revision;
            peers.push_back(std::move(peer));
        }
        if (overflow.ConnectAndOpen(40) || overflow.connected()) { return false; }
        for (auto& peer : peers) { peer->Close(500); }
    }
    std::wcout << L"16 live clients, bounded saturation, retained slot reuse, isolated candidate commits and three full-pool recovery cycles passed.\n";
    return true;
}

bool ProbeFake(mo::windows_tip::BrokerClient* broker) {
    mo::windows_tip::BrokerSnapshot snapshot;
    if (!broker->SendKey('M', 0, 0, true, false, &snapshot, 500)) {
        std::wcerr << L"BrokerClient::SendKey failed\n";
        return false;
    }
    if (!snapshot.handled
        || snapshot.composition != "m"
        || snapshot.commit.has_value()
        || snapshot.candidates.size() != 2
        || snapshot.candidates[0] != "m"
        || snapshot.candidates[1] != "M") {
        std::wcerr << L"Unexpected diagnostic Broker snapshot\n";
        return false;
    }
    const auto original_revision = snapshot.revision;
    if (!broker->candidate_actions_supported()
        || !broker->SendCandidateAction(snapshot.revision, mo::windows_tip::CandidateAction::NextPage, 0, &snapshot, 500)
        || snapshot.candidates != std::vector<std::string>{"m#2", "M#2"}
        || !broker->SendCandidateAction(snapshot.revision, mo::windows_tip::CandidateAction::PreviousPage, 0, &snapshot, 500)
        || snapshot.candidates != std::vector<std::string>{"m", "M"}
        || !broker->SendCandidateAction(snapshot.revision, mo::windows_tip::CandidateAction::Select, 1, &snapshot, 500)
        || snapshot.commit != "M") {
        std::wcerr << L"Fake candidate actions did not page and select\n"; return false;
    }
    if (broker->SendCandidateAction(original_revision, mo::windows_tip::CandidateAction::Select, 0, &snapshot, 500)
        || broker->connected()) {
        std::wcerr << L"Stale candidate action was not rejected/reset\n"; return false;
    }
    return true;
}

bool ProbeRimeIce(mo::windows_tip::BrokerClient* broker) {
    mo::windows_tip::BrokerSnapshot snapshot;
    for (const char key : std::string("NIHAO")) {
        if (!broker->SendKey(
                static_cast<std::uint32_t>(key), 0, 0, true, false, &snapshot, 2000)) {
            std::wcerr << L"BrokerClient::SendKey failed during rime-ice input\n";
            return false;
        }
    }

    const std::string expected = u8"你好";
    if (!snapshot.handled
        || snapshot.composition.empty()
        || std::find(snapshot.candidates.begin(), snapshot.candidates.end(), expected)
            == snapshot.candidates.end()) {
        std::cerr << "rime-ice did not produce the expected nihao candidate; composition="
                  << snapshot.composition << ", candidates=[";
        for (std::size_t index = 0; index < snapshot.candidates.size(); ++index) {
            if (index != 0) {
                std::cerr << ", ";
            }
            std::cerr << snapshot.candidates[index];
        }
        std::cerr << "]\n";
        return false;
    }
    if (!broker->SendKey(VK_SPACE, 0, 0, true, false, &snapshot, 2000)
        || !snapshot.handled
        || snapshot.commit != expected) {
        std::wcerr << L"rime-ice did not commit the expected candidate\n";
        return false;
    }

    for (const char key : std::string("NI")) {
        if (!broker->SendKey(static_cast<std::uint32_t>(key), 0, 0, true, false, &snapshot, 2000)) {
            std::wcerr << L"rime-ice candidate navigation input failed\n";
            return false;
        }
    }
    if (snapshot.candidates.size() < 2) {
        std::wcerr << L"rime-ice ni did not produce two selectable candidates\n";
        return false;
    }
    const auto original_page = snapshot.candidates;
    const auto second_candidate = snapshot.candidates[1];
    if (!broker->SendKey(VK_NEXT, 0, 0, true, false, &snapshot, 2000)
        || !snapshot.handled
        || snapshot.commit.has_value()
        || snapshot.candidates.empty()
        || snapshot.candidates == original_page) {
        std::wcerr << L"rime-ice PageDown did not move to another candidate page\n";
        return false;
    }
    if (!broker->SendKey(VK_PRIOR, 0, 0, true, false, &snapshot, 2000)
        || !snapshot.handled
        || snapshot.commit.has_value()
        || snapshot.candidates != original_page) {
        std::wcerr << L"rime-ice PageUp did not restore the original candidate page\n";
        return false;
    }
    if (!broker->SendKey('2', 0, 0, true, false, &snapshot, 2000)
        || !snapshot.handled
        || snapshot.commit != second_candidate) {
        std::wcerr << L"rime-ice numeric selection did not commit the second candidate\n";
        return false;
    }
    for (const char key : std::string("NI")) {
        if (!broker->SendKey(static_cast<std::uint32_t>(key), 0, 0, true, false, &snapshot, 2000)) { return false; }
    }
    const auto action_first_page = snapshot.candidates;
    if (action_first_page.size() < 2) { return false; }
    const auto action_second_candidate = action_first_page[1];
    const auto original_revision = snapshot.revision;
    if (!broker->candidate_actions_supported()
        || !broker->SendCandidateAction(snapshot.revision, mo::windows_tip::CandidateAction::NextPage, 0, &snapshot, 2000)
        || snapshot.commit.has_value() || snapshot.candidates.empty() || snapshot.candidates == action_first_page
        || !broker->SendCandidateAction(snapshot.revision, mo::windows_tip::CandidateAction::PreviousPage, 0, &snapshot, 2000)
        || snapshot.commit.has_value() || snapshot.candidates != action_first_page
        || !broker->SendCandidateAction(snapshot.revision, mo::windows_tip::CandidateAction::Select, 1, &snapshot, 2000)
        || snapshot.commit != action_second_candidate) {
        std::wcerr << L"Real candidate action IPC paging/selection failed\n"; return false;
    }
    if (broker->SendCandidateAction(original_revision, mo::windows_tip::CandidateAction::Select, 0, &snapshot, 2000)
        || broker->connected()) {
        std::wcerr << L"Real stale candidate action was not rejected/reset\n"; return false;
    }
    return true;
}

}  // namespace

int wmain(int argc, wchar_t** argv) {
    const bool pool = argc == 3 && std::wstring(argv[2]) == L"--pool";
    const bool pool_rime = argc == 3 && std::wstring(argv[2]) == L"--pool-rime-ice";
    const bool rime_ice = argc == 3 && std::wstring(argv[2]) == L"--rime-ice";
    const bool reject_unexpected =
        argc == 3 && std::wstring(argv[2]) == L"--reject-unexpected";
    if ((argc != 2 && !rime_ice && !reject_unexpected && !pool && !pool_rime) || argv[1][0] == L'\0') {
        std::wcerr << L"usage: mo_tip_ipc_probe.exe <absolute-broker-path> "
                      L"[--rime-ice|--reject-unexpected|--pool|--pool-rime-ice]\n";
        return 2;
    }

    if (pool || pool_rime) { return ProbePool(argv[1], pool_rime) ? 0 : 1; }
    if (reject_unexpected) {
        if (!ProbeRejectsUnexpectedServerImage()) {
            return 1;
        }
        std::wcout << L"Mo TIP client rejected an unexpected Broker server image.\n";
        return 0;
    }

    if (!ProbeExpiredKeyBudget(argv[1])) { return 1; }

    mo::windows_tip::BrokerClient broker(argv[1]);
    if (!broker.ConnectAndOpen(2000)) {
        std::wcerr << L"BrokerClient::ConnectAndOpen failed with Win32 error "
                   << GetLastError() << L'\n';
        return 1;
    }
    if (broker.generation() == 0 || broker.session_token() == 0) {
        std::wcerr << L"Broker handshake returned zero correlation values\n";
        return 1;
    }
    const auto settings_revision = broker.settings().revision;
    if (settings_revision == 0 || broker.settings().stored
        || broker.settings().input_scheme != mo::windows_tip::InputScheme::FullPinyin
        || broker.settings().character_set != mo::windows_tip::CharacterSet::Simplified
        || broker.settings().candidate_page_size != 5
        || broker.settings().theme != mo::windows_tip::CandidateTheme::System
        || !broker.settings().show_comments || !broker.settings().emoji
        || !broker.settings().local_learning || broker.settings().privacy_mode
        || !broker.settings().effective_learning
        || !broker.RefreshSettings(500)
        || broker.settings().revision != settings_revision) {
        std::wcerr << L"Broker settings snapshot or explicit refresh is invalid\n";
        return 1;
    }

    if (!(rime_ice ? ProbeRimeIce(&broker) : ProbeFake(&broker))) {
        return 1;
    }

    broker.Close(500);
    std::wcout << (rime_ice
        ? L"Mo TIP C++ client -> Broker -> librime/rime-ice probe passed.\n"
        : L"Mo TIP C++ client -> Rust named-pipe Broker probe passed.\n");
    return 0;
}
