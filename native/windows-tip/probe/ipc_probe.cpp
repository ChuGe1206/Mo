#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <algorithm>
#include <iostream>
#include <string>

#include "mo_broker_client.h"

namespace {

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
    return true;
}

}  // namespace

int wmain(int argc, wchar_t** argv) {
    const bool rime_ice = argc == 2 && std::wstring(argv[1]) == L"--rime-ice";
    if (argc != 1 && !rime_ice) {
        std::wcerr << L"usage: mo_tip_ipc_probe.exe [--rime-ice]\n";
        return 2;
    }

    mo::windows_tip::BrokerClient broker;
    if (!broker.ConnectAndOpen(2000)) {
        std::wcerr << L"BrokerClient::ConnectAndOpen failed\n";
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
