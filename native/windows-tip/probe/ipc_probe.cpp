#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <algorithm>
#include <iostream>
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
    const bool rime_ice = argc == 3 && std::wstring(argv[2]) == L"--rime-ice";
    const bool reject_unexpected =
        argc == 3 && std::wstring(argv[2]) == L"--reject-unexpected";
    if ((argc != 2 && !rime_ice && !reject_unexpected) || argv[1][0] == L'\0') {
        std::wcerr << L"usage: mo_tip_ipc_probe.exe <absolute-broker-path> "
                      L"[--rime-ice|--reject-unexpected]\n";
        return 2;
    }

    if (reject_unexpected) {
        if (!ProbeRejectsUnexpectedServerImage()) {
            return 1;
        }
        std::wcout << L"Mo TIP client rejected an unexpected Broker server image.\n";
        return 0;
    }

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

    if (!(rime_ice ? ProbeRimeIce(&broker) : ProbeFake(&broker))) {
        return 1;
    }

    broker.Close(500);
    std::wcout << (rime_ice
        ? L"Mo TIP C++ client -> Broker -> librime/rime-ice probe passed.\n"
        : L"Mo TIP C++ client -> Rust named-pipe Broker probe passed.\n");
    return 0;
}
