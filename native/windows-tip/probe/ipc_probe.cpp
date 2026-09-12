#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <iostream>

#include "mo_broker_client.h"

int wmain() {
    mo::windows_tip::BrokerClient broker;
    if (!broker.ConnectAndOpen(2000)) {
        std::wcerr << L"BrokerClient::ConnectAndOpen failed\n";
        return 1;
    }
    if (broker.generation() == 0 || broker.session_token() == 0) {
        std::wcerr << L"Broker handshake returned zero correlation values\n";
        return 1;
    }

    mo::windows_tip::BrokerSnapshot snapshot;
    if (!broker.SendKey('M', 0, 0, true, false, &snapshot, 500)) {
        std::wcerr << L"BrokerClient::SendKey failed\n";
        return 1;
    }
    if (!snapshot.handled
        || snapshot.composition != "m"
        || snapshot.commit.has_value()
        || snapshot.candidates.size() != 2
        || snapshot.candidates[0] != "m"
        || snapshot.candidates[1] != "M") {
        std::wcerr << L"Unexpected Broker snapshot\n";
        return 1;
    }

    broker.Close(500);
    std::wcout << L"Mo TIP C++ client -> Rust named-pipe Broker probe passed.\n";
    return 0;
}
