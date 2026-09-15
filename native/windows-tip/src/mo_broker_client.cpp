#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include "mo_broker_client.h"

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <limits>
#include <new>
#include <utility>
#include <vector>

namespace {

using Byte = std::uint8_t;

constexpr wchar_t kPipeName[] = L"\\\\.\\pipe\\LOCAL\\Mo.Input.Broker.v1";
constexpr std::array<Byte, 4> kMagic = {'M', 'O', 'I', 'P'};
constexpr std::size_t kHeaderLength = 48;
constexpr std::size_t kMaximumPayloadLength = 64 * 1024;
// Maximum encoded Snapshot size from mo-ipc. A smaller negotiated limit could
// make a protocol-valid response impossible to deliver.
constexpr std::uint32_t kMinimumNegotiatedPayloadLength = 24724;
constexpr std::uint16_t kProtocolMajor = 1;
constexpr std::uint16_t kProtocolMinor = 0;
constexpr std::uint32_t kResponseFlag = 1U;
constexpr std::uint32_t kErrorFlag = 2U;
constexpr std::uint64_t kKeyEventsFeature = 1ULL;
constexpr DWORD kClientPipeAccess = 0x0012019bUL;
constexpr std::size_t kMaximumWindowsPathLength = 32768;

class ScopedHandle final {
public:
    explicit ScopedHandle(HANDLE handle = nullptr) noexcept : handle_(handle) {}
    ~ScopedHandle() noexcept {
        if (handle_ != nullptr && handle_ != INVALID_HANDLE_VALUE) {
            CloseHandle(handle_);
        }
    }

    ScopedHandle(const ScopedHandle&) = delete;
    ScopedHandle& operator=(const ScopedHandle&) = delete;

    HANDLE get() const noexcept { return handle_; }

private:
    HANDLE handle_;
};

enum class MessageKind : std::uint16_t {
    Hello = 1,
    HelloAck = 2,
    OpenSession = 3,
    OpenSessionAck = 4,
    KeyEvent = 5,
    Snapshot = 6,
    CloseSession = 7,
    CloseSessionAck = 8,
};

struct Frame final {
    MessageKind kind = MessageKind::Hello;
    std::uint32_t flags = 0;
    std::uint64_t generation = 0;
    std::uint64_t session_token = 0;
    std::uint64_t request_id = 0;
    std::vector<Byte> payload;
};

ULONGLONG DeadlineFromNow(DWORD timeout_ms) noexcept {
    const ULONGLONG now = GetTickCount64();
    const ULONGLONG deadline = now + timeout_ms;
    return deadline < now ? std::numeric_limits<ULONGLONG>::max() : deadline;
}

DWORD RemainingMilliseconds(ULONGLONG deadline) noexcept {
    const ULONGLONG now = GetTickCount64();
    if (now >= deadline) {
        return 0;
    }
    const ULONGLONG remaining = deadline - now;
    return remaining > MAXDWORD ? MAXDWORD : static_cast<DWORD>(remaining);
}

void PutU16(Byte* output, std::uint16_t value) noexcept {
    output[0] = static_cast<Byte>(value);
    output[1] = static_cast<Byte>(value >> 8U);
}

void PutU32(Byte* output, std::uint32_t value) noexcept {
    for (std::size_t index = 0; index < 4; ++index) {
        output[index] = static_cast<Byte>(value >> (index * 8U));
    }
}

void PutU64(Byte* output, std::uint64_t value) noexcept {
    for (std::size_t index = 0; index < 8; ++index) {
        output[index] = static_cast<Byte>(value >> (index * 8U));
    }
}

std::uint16_t GetU16(const Byte* input) noexcept {
    return static_cast<std::uint16_t>(input[0])
        | static_cast<std::uint16_t>(static_cast<std::uint16_t>(input[1]) << 8U);
}

std::uint32_t GetU32(const Byte* input) noexcept {
    std::uint32_t value = 0;
    for (std::size_t index = 0; index < 4; ++index) {
        value |= static_cast<std::uint32_t>(input[index]) << (index * 8U);
    }
    return value;
}

std::uint64_t GetU64(const Byte* input) noexcept {
    std::uint64_t value = 0;
    for (std::size_t index = 0; index < 8; ++index) {
        value |= static_cast<std::uint64_t>(input[index]) << (index * 8U);
    }
    return value;
}

std::uint32_t Crc32(const std::vector<Byte>& bytes) noexcept {
    std::uint32_t crc = 0xffffffffU;
    for (const Byte byte : bytes) {
        crc ^= byte;
        for (unsigned int bit = 0; bit < 8; ++bit) {
            const std::uint32_t mask = 0U - (crc & 1U);
            crc = (crc >> 1U) ^ (0xedb88320U & mask);
        }
    }
    return ~crc;
}

bool TransferExact(
    HANDLE pipe,
    Byte* buffer,
    std::size_t length,
    bool write,
    ULONGLONG deadline) noexcept {
    HANDLE event_handle = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    if (event_handle == nullptr) {
        return false;
    }

    std::size_t offset = 0;
    bool success = true;
    while (offset < length) {
        const std::size_t remaining = length - offset;
        const DWORD chunk = remaining > MAXDWORD ? MAXDWORD : static_cast<DWORD>(remaining);
        OVERLAPPED operation{};
        operation.hEvent = event_handle;
        ResetEvent(event_handle);

        DWORD transferred = 0;
        const BOOL started = write
            ? WriteFile(pipe, buffer + offset, chunk, &transferred, &operation)
            : ReadFile(pipe, buffer + offset, chunk, &transferred, &operation);
        if (started == FALSE) {
            const DWORD error = GetLastError();
            if (error != ERROR_IO_PENDING) {
                success = false;
                break;
            }
            const DWORD wait = WaitForSingleObject(event_handle, RemainingMilliseconds(deadline));
            if (wait != WAIT_OBJECT_0) {
                CancelIoEx(pipe, &operation);
                WaitForSingleObject(event_handle, INFINITE);
                success = false;
                break;
            }
            if (GetOverlappedResult(pipe, &operation, &transferred, FALSE) == FALSE) {
                success = false;
                break;
            }
        }
        if (transferred == 0) {
            success = false;
            break;
        }
        offset += transferred;
    }

    CloseHandle(event_handle);
    return success;
}

bool WriteFrame(HANDLE pipe, const Frame& frame, ULONGLONG deadline) {
    if (frame.payload.size() > kMaximumPayloadLength) {
        return false;
    }
    std::vector<Byte> encoded(kHeaderLength + frame.payload.size(), 0);
    std::memcpy(encoded.data(), kMagic.data(), kMagic.size());
    PutU16(encoded.data() + 4, static_cast<std::uint16_t>(kHeaderLength));
    PutU16(encoded.data() + 6, kProtocolMajor);
    PutU16(encoded.data() + 8, kProtocolMinor);
    PutU16(encoded.data() + 10, static_cast<std::uint16_t>(frame.kind));
    PutU32(encoded.data() + 12, frame.flags);
    PutU32(encoded.data() + 16, static_cast<std::uint32_t>(frame.payload.size()));
    PutU32(encoded.data() + 20, Crc32(frame.payload));
    PutU64(encoded.data() + 24, frame.generation);
    PutU64(encoded.data() + 32, frame.session_token);
    PutU64(encoded.data() + 40, frame.request_id);
    if (!frame.payload.empty()) {
        std::memcpy(
            encoded.data() + kHeaderLength,
            frame.payload.data(),
            frame.payload.size());
    }
    return TransferExact(pipe, encoded.data(), encoded.size(), true, deadline);
}

bool ReadFrame(HANDLE pipe, Frame* frame, ULONGLONG deadline) {
    if (frame == nullptr) {
        return false;
    }
    std::array<Byte, kHeaderLength> header{};
    if (!TransferExact(pipe, header.data(), header.size(), false, deadline)) {
        return false;
    }
    if (!std::equal(kMagic.begin(), kMagic.end(), header.begin())
        || GetU16(header.data() + 4) != kHeaderLength
        || GetU16(header.data() + 6) != kProtocolMajor
        || GetU16(header.data() + 8) != kProtocolMinor) {
        return false;
    }
    const std::uint32_t flags = GetU32(header.data() + 12);
    if ((flags & ~(kResponseFlag | kErrorFlag)) != 0) {
        return false;
    }
    const std::size_t payload_length = GetU32(header.data() + 16);
    if (payload_length > kMaximumPayloadLength) {
        return false;
    }
    std::vector<Byte> payload(payload_length);
    if (!payload.empty()
        && !TransferExact(pipe, payload.data(), payload.size(), false, deadline)) {
        return false;
    }
    if (Crc32(payload) != GetU32(header.data() + 20)) {
        return false;
    }
    frame->kind = static_cast<MessageKind>(GetU16(header.data() + 10));
    frame->flags = flags;
    frame->generation = GetU64(header.data() + 24);
    frame->session_token = GetU64(header.data() + 32);
    frame->request_id = GetU64(header.data() + 40);
    frame->payload = std::move(payload);
    return true;
}

bool Exchange(
    HANDLE pipe,
    const Frame& request,
    MessageKind response_kind,
    Frame* response,
    ULONGLONG deadline) {
    Frame received;
    if (!WriteFrame(pipe, request, deadline) || !ReadFrame(pipe, &received, deadline)) {
        return false;
    }
    if (received.kind != response_kind
        || received.flags != kResponseFlag
        || received.request_id != request.request_id) {
        return false;
    }
    *response = std::move(received);
    return true;
}

bool IsDriveAbsolutePath(const std::wstring& path) noexcept {
    return path.size() >= 3
        && ((path[0] >= L'A' && path[0] <= L'Z')
            || (path[0] >= L'a' && path[0] <= L'z'))
        && path[1] == L':'
        && (path[2] == L'\\' || path[2] == L'/');
}

bool QueryTokenLogonSid(HANDLE token, std::vector<Byte>* storage, PSID* sid) {
    DWORD required = 0;
    if (GetTokenInformation(token, TokenGroups, nullptr, 0, &required) != FALSE
        || GetLastError() != ERROR_INSUFFICIENT_BUFFER
        || required == 0) {
        return false;
    }
    storage->resize(required);
    if (GetTokenInformation(
            token,
            TokenGroups,
            storage->data(),
            required,
            &required)
        == FALSE) {
        return false;
    }
    const auto* token_groups = reinterpret_cast<const TOKEN_GROUPS*>(storage->data());
    for (DWORD index = 0; index < token_groups->GroupCount; ++index) {
        const SID_AND_ATTRIBUTES& group = token_groups->Groups[index];
        if ((group.Attributes & SE_GROUP_LOGON_ID) == SE_GROUP_LOGON_ID
            && group.Sid != nullptr
            && IsValidSid(group.Sid) != FALSE) {
            *sid = group.Sid;
            return true;
        }
    }
    return false;
}

bool IsSameWindowsLogon(HANDLE process) {
    HANDLE server_token_raw = nullptr;
    HANDLE client_token_raw = nullptr;
    if (OpenProcessToken(process, TOKEN_QUERY, &server_token_raw) == FALSE) {
        return false;
    }
    ScopedHandle server_token(server_token_raw);
    if (OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &client_token_raw) == FALSE) {
        return false;
    }
    ScopedHandle client_token(client_token_raw);

    std::vector<Byte> server_storage;
    std::vector<Byte> client_storage;
    PSID server_sid = nullptr;
    PSID client_sid = nullptr;
    return QueryTokenLogonSid(server_token.get(), &server_storage, &server_sid)
        && QueryTokenLogonSid(client_token.get(), &client_storage, &client_sid)
        && EqualSid(server_sid, client_sid) != FALSE;
}

bool QueryProcessImagePath(HANDLE process, std::wstring* path) {
    std::vector<wchar_t> buffer(kMaximumWindowsPathLength);
    DWORD length = static_cast<DWORD>(buffer.size());
    if (QueryFullProcessImageNameW(process, 0, buffer.data(), &length) == FALSE
        || length == 0
        || length >= buffer.size()) {
        return false;
    }
    path->assign(buffer.data(), length);
    return true;
}

bool IsSameFile(const std::wstring& first_path, const std::wstring& second_path) noexcept {
    constexpr DWORD share_mode = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
    ScopedHandle first(CreateFileW(
        first_path.c_str(),
        FILE_READ_ATTRIBUTES,
        share_mode,
        nullptr,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        nullptr));
    if (first.get() == INVALID_HANDLE_VALUE) {
        return false;
    }
    ScopedHandle second(CreateFileW(
        second_path.c_str(),
        FILE_READ_ATTRIBUTES,
        share_mode,
        nullptr,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        nullptr));
    if (second.get() == INVALID_HANDLE_VALUE) {
        return false;
    }

    BY_HANDLE_FILE_INFORMATION first_info{};
    BY_HANDLE_FILE_INFORMATION second_info{};
    return GetFileInformationByHandle(first.get(), &first_info) != FALSE
        && GetFileInformationByHandle(second.get(), &second_info) != FALSE
        && (first_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) == 0
        && (second_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) == 0
        && first_info.dwVolumeSerialNumber == second_info.dwVolumeSerialNumber
        && first_info.nFileIndexHigh == second_info.nFileIndexHigh
        && first_info.nFileIndexLow == second_info.nFileIndexLow;
}

bool IsExpectedBrokerServer(HANDLE pipe, const std::wstring& expected_path) {
    if (!IsDriveAbsolutePath(expected_path)) {
        SetLastError(ERROR_BAD_PATHNAME);
        return false;
    }

    ULONG server_process_id = 0;
    if (GetNamedPipeServerProcessId(pipe, &server_process_id) == FALSE) {
        return false;
    }
    if (server_process_id == 0 || server_process_id == GetCurrentProcessId()) {
        SetLastError(ERROR_INVALID_OWNER);
        return false;
    }

    ScopedHandle process(OpenProcess(
        PROCESS_QUERY_LIMITED_INFORMATION,
        FALSE,
        server_process_id));
    if (process.get() == nullptr) {
        return false;
    }
    if (!IsSameWindowsLogon(process.get())) {
        SetLastError(ERROR_ACCESS_DENIED);
        return false;
    }

    std::wstring actual_path;
    if (!QueryProcessImagePath(process.get(), &actual_path)) {
        return false;
    }
    if (!IsSameFile(expected_path, actual_path)) {
        SetLastError(ERROR_INVALID_IMAGE_HASH);
        return false;
    }
    return true;
}

HANDLE ConnectPipe(
    const std::wstring& expected_broker_path,
    ULONGLONG deadline) noexcept {
    for (;;) {
        const HANDLE pipe = CreateFileW(
            kPipeName,
            kClientPipeAccess,
            0,
            nullptr,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT
                | SECURITY_IDENTIFICATION,
            nullptr);
        if (pipe != INVALID_HANDLE_VALUE) {
            try {
                if (!IsExpectedBrokerServer(pipe, expected_broker_path)) {
                    const DWORD identity_error = GetLastError();
                    CloseHandle(pipe);
                    SetLastError(identity_error);
                    return INVALID_HANDLE_VALUE;
                }
            } catch (...) {
                CloseHandle(pipe);
                SetLastError(ERROR_NOT_ENOUGH_MEMORY);
                return INVALID_HANDLE_VALUE;
            }
            return pipe;
        }
        const DWORD error = GetLastError();
        const DWORD remaining = RemainingMilliseconds(deadline);
        if (remaining == 0) {
            return INVALID_HANDLE_VALUE;
        }
        if (error == ERROR_PIPE_BUSY) {
            WaitNamedPipeW(kPipeName, remaining);
        } else if (error == ERROR_FILE_NOT_FOUND) {
            // The installed Broker is long-lived. If its endpoint does not
            // exist, return immediately so TIP activation stays fail-open;
            // the caller owns throttled reconnect attempts.
            return INVALID_HANDLE_VALUE;
        } else {
            return INVALID_HANDLE_VALUE;
        }
    }
}

void AppendU16(std::vector<Byte>* bytes, std::uint16_t value) {
    const std::size_t offset = bytes->size();
    bytes->resize(offset + 2);
    PutU16(bytes->data() + offset, value);
}

void AppendU32(std::vector<Byte>* bytes, std::uint32_t value) {
    const std::size_t offset = bytes->size();
    bytes->resize(offset + 4);
    PutU32(bytes->data() + offset, value);
}

void AppendU64(std::vector<Byte>* bytes, std::uint64_t value) {
    const std::size_t offset = bytes->size();
    bytes->resize(offset + 8);
    PutU64(bytes->data() + offset, value);
}

bool IsValidUtf8(const Byte* bytes, int length) noexcept {
    if (length == 0) {
        return true;
    }
    return MultiByteToWideChar(
               CP_UTF8,
               MB_ERR_INVALID_CHARS,
               reinterpret_cast<const char*>(bytes),
               length,
               nullptr,
               0)
        > 0;
}

class PayloadReader final {
public:
    explicit PayloadReader(const std::vector<Byte>& bytes) noexcept : bytes_(bytes) {}

    bool U8(Byte* value) noexcept { return Take(value, 1); }
    bool U16(std::uint16_t* value) noexcept {
        std::array<Byte, 2> data{};
        if (!Take(data.data(), data.size())) {
            return false;
        }
        *value = GetU16(data.data());
        return true;
    }
    bool U32(std::uint32_t* value) noexcept {
        std::array<Byte, 4> data{};
        if (!Take(data.data(), data.size())) {
            return false;
        }
        *value = GetU32(data.data());
        return true;
    }
    bool U64(std::uint64_t* value) noexcept {
        std::array<Byte, 8> data{};
        if (!Take(data.data(), data.size())) {
            return false;
        }
        *value = GetU64(data.data());
        return true;
    }
    bool String(std::size_t maximum, std::string* value) {
        std::uint32_t length = 0;
        if (!U32(&length) || length > maximum || length > remaining()) {
            return false;
        }
        const Byte* start = bytes_.data() + offset_;
        if (length > static_cast<std::uint32_t>(std::numeric_limits<int>::max())
            || !IsValidUtf8(start, static_cast<int>(length))) {
            return false;
        }
        value->assign(reinterpret_cast<const char*>(start), length);
        offset_ += length;
        return true;
    }
    bool finished() const noexcept { return offset_ == bytes_.size(); }

private:
    bool Take(void* output, std::size_t count) noexcept {
        if (count > remaining()) {
            return false;
        }
        std::memcpy(output, bytes_.data() + offset_, count);
        offset_ += count;
        return true;
    }
    std::size_t remaining() const noexcept { return bytes_.size() - offset_; }

    const std::vector<Byte>& bytes_;
    std::size_t offset_ = 0;
};

bool DecodeSnapshot(const Frame& frame, mo::windows_tip::BrokerSnapshot* snapshot) {
    PayloadReader reader(frame.payload);
    std::uint64_t revision = 0;
    Byte handled = 0;
    Byte has_commit = 0;
    std::string composition;
    std::string commit;
    std::uint16_t candidate_count = 0;
    if (!reader.U64(&revision)
        || !reader.U8(&handled)
        || handled > 1
        || !reader.String(4 * 1024, &composition)
        || !reader.U8(&has_commit)
        || has_commit > 1
        || (has_commit != 0 && !reader.String(4 * 1024, &commit))
        || !reader.U16(&candidate_count)
        || candidate_count > 32) {
        return false;
    }
    std::vector<std::string> candidates;
    candidates.reserve(candidate_count);
    for (std::uint16_t index = 0; index < candidate_count; ++index) {
        std::string candidate;
        if (!reader.String(512, &candidate)) {
            return false;
        }
        candidates.push_back(std::move(candidate));
    }
    if (!reader.finished()) {
        return false;
    }
    snapshot->revision = revision;
    snapshot->handled = handled != 0;
    snapshot->composition = std::move(composition);
    snapshot->commit = has_commit != 0
        ? std::optional<std::string>(std::move(commit))
        : std::nullopt;
    snapshot->candidates = std::move(candidates);
    return true;
}

}  // namespace

namespace mo::windows_tip {

BrokerClient::BrokerClient(std::wstring expected_broker_path) noexcept
    : expected_broker_path_(std::move(expected_broker_path)) {}

BrokerClient::~BrokerClient() noexcept {
    Close(20);
}

bool BrokerClient::ConnectAndOpen(DWORD timeout_ms) noexcept {
    try {
        Reset();
        const ULONGLONG deadline = DeadlineFromNow(timeout_ms);
        pipe_ = ConnectPipe(expected_broker_path_, deadline);
        if (pipe_ == INVALID_HANDLE_VALUE) {
            return false;
        }

        Frame hello;
        hello.kind = MessageKind::Hello;
        hello.request_id = next_request_id_++;
        AppendU16(&hello.payload, kProtocolMajor);
        AppendU16(&hello.payload, kProtocolMinor);
        AppendU16(&hello.payload, kProtocolMajor);
        AppendU16(&hello.payload, kProtocolMinor);
        AppendU64(&hello.payload, kKeyEventsFeature);
        AppendU32(&hello.payload, static_cast<std::uint32_t>(kMaximumPayloadLength));

        Frame hello_ack;
        if (!Exchange(pipe_, hello, MessageKind::HelloAck, &hello_ack, deadline)
            || hello_ack.generation == 0
            || hello_ack.session_token != 0
            || hello_ack.payload.size() != 16
            || GetU16(hello_ack.payload.data()) != kProtocolMajor
            || GetU16(hello_ack.payload.data() + 2) != kProtocolMinor
            || (GetU64(hello_ack.payload.data() + 4) & kKeyEventsFeature) == 0
            || GetU32(hello_ack.payload.data() + 12) < kMinimumNegotiatedPayloadLength
            || GetU32(hello_ack.payload.data() + 12) > kMaximumPayloadLength) {
            Reset();
            return false;
        }
        generation_ = hello_ack.generation;

        Frame open;
        open.kind = MessageKind::OpenSession;
        open.generation = generation_;
        open.request_id = next_request_id_++;
        Frame opened;
        if (!Exchange(pipe_, open, MessageKind::OpenSessionAck, &opened, deadline)
            || opened.generation != generation_
            || opened.session_token == 0
            || !opened.payload.empty()) {
            Reset();
            return false;
        }
        session_token_ = opened.session_token;
        return true;
    } catch (const std::bad_alloc&) {
        Reset();
        return false;
    } catch (...) {
        Reset();
        return false;
    }
}

bool BrokerClient::SendKey(
    UINT virtual_key,
    UINT scan_code,
    std::uint16_t modifiers,
    bool key_down,
    bool repeat,
    BrokerSnapshot* snapshot,
    DWORD timeout_ms) noexcept {
    if (!connected() || snapshot == nullptr) {
        return false;
    }
    try {
        const ULONGLONG deadline = DeadlineFromNow(timeout_ms);
        Frame request;
        request.kind = MessageKind::KeyEvent;
        request.generation = generation_;
        request.session_token = session_token_;
        request.request_id = next_request_id_++;
        AppendU32(&request.payload, virtual_key);
        AppendU32(&request.payload, scan_code);
        AppendU16(&request.payload, modifiers);
        request.payload.push_back(key_down ? 1 : 0);
        request.payload.push_back(repeat ? 1 : 0);

        Frame response;
        if (!Exchange(pipe_, request, MessageKind::Snapshot, &response, deadline)
            || response.generation != generation_
            || response.session_token != session_token_
            || !DecodeSnapshot(response, snapshot)) {
            Reset();
            return false;
        }
        return true;
    } catch (...) {
        Reset();
        return false;
    }
}

void BrokerClient::Close(DWORD timeout_ms) noexcept {
    if (!connected()) {
        Reset();
        return;
    }
    try {
        if (generation_ != 0 && session_token_ != 0) {
            const ULONGLONG deadline = DeadlineFromNow(timeout_ms);
            Frame request;
            request.kind = MessageKind::CloseSession;
            request.generation = generation_;
            request.session_token = session_token_;
            request.request_id = next_request_id_++;
            Frame response;
            Exchange(pipe_, request, MessageKind::CloseSessionAck, &response, deadline);
        }
    } catch (...) {
    }
    Reset();
}

void BrokerClient::Reset() noexcept {
    if (pipe_ != INVALID_HANDLE_VALUE) {
        CancelIoEx(pipe_, nullptr);
        CloseHandle(pipe_);
    }
    pipe_ = INVALID_HANDLE_VALUE;
    generation_ = 0;
    session_token_ = 0;
    next_request_id_ = 1;
}

}  // namespace mo::windows_tip
