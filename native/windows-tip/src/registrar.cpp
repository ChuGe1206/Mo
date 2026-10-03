#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#define _WIN32_WINNT 0x0A00
#include <windows.h>

#include <msctf.h>
#include <shlobj.h>

#include <algorithm>
#include <array>
#include <functional>
#include <iostream>
#include <iterator>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "mo_tip_ids.h"

namespace {

constexpr LANGID kLanguage = MAKELANGID(LANG_CHINESE, SUBLANG_CHINESE_SIMPLIFIED);
constexpr DWORD kIlotUninstall = 0x00000001;
constexpr wchar_t kMachineProfileKey[] =
    L"Software\\Microsoft\\CTF\\TIP\\{B4911146-2A27-47AA-9D12-109B6AE10A70}\\LanguageProfile\\0x00000804\\{595A4275-4C0B-4D4F-80C3-7DD322BC6F74}";
constexpr wchar_t kMachineCategoryByCategoryKey[] =
    L"Software\\Microsoft\\CTF\\TIP\\{B4911146-2A27-47AA-9D12-109B6AE10A70}\\Category\\Category\\{34745C63-B2F0-4784-8B67-5E12C8701A31}\\{B4911146-2A27-47AA-9D12-109B6AE10A70}";
constexpr wchar_t kMachineCategoryByItemKey[] =
    L"Software\\Microsoft\\CTF\\TIP\\{B4911146-2A27-47AA-9D12-109B6AE10A70}\\Category\\Item\\{B4911146-2A27-47AA-9D12-109B6AE10A70}\\{34745C63-B2F0-4784-8B67-5E12C8701A31}";
constexpr wchar_t kComKey[] =
    L"Software\\Classes\\CLSID\\{B4911146-2A27-47AA-9D12-109B6AE10A70}\\InprocServer32";
constexpr wchar_t kComClsidPath[] =
    L"Software\\Classes\\CLSID\\{B4911146-2A27-47AA-9D12-109B6AE10A70}";
constexpr wchar_t kComParentKey[] = L"Software\\Classes\\CLSID";
constexpr wchar_t kComClsidKey[] = L"{B4911146-2A27-47AA-9D12-109B6AE10A70}";
constexpr wchar_t kProbeComKey[] =
    L"Software\\Classes\\CLSID\\"
    L"{A51FCF97-6D9E-4C59-8905-C36A443AB7C2}\\InprocServer32";
constexpr wchar_t kProbeComClsidKey[] = L"{A51FCF97-6D9E-4C59-8905-C36A443AB7C2}";
constexpr wchar_t kFixedTipSuffix[] = L"\\Mo\\tip\\x64\\mo-tip.dll";
constexpr wchar_t kFixedTipX86Suffix[] = L"\\Mo\\tip\\x86\\mo-tip.dll";
constexpr wchar_t kInstallMarkerSuffix[] = L"\\Mo\\.machine-profile-install-rollback-v1";
constexpr wchar_t kRemoveMarkerSuffix[] = L"\\Mo\\.machine-profile-remove-rollback-v1";
// Software\Classes\Local Settings is deliberately machine-local for a roaming
// user profile. Bundle detection must never roam to a machine without Mo.
constexpr wchar_t kUserFinalizerKey[] =
    L"Software\\Classes\\Local Settings\\Software\\Mo\\InputMethod\\Setup";
constexpr wchar_t kUserFinalizerValue[] = L"UserFinalizer";
constexpr wchar_t kUserFinalizerMarker[] = L"mo-user-finalizer-v1";
constexpr wchar_t kUserFinalizerTransactionValue[] = L"UserFinalizerTransaction";
constexpr DWORD kMaximumRegistryStringBytes = 64 * 1024;
constexpr wchar_t kUserFinalizerMutex[] =
    L"Local\\Mo.InputMethod.UserFinalizer.{5B37FACA-6075-49FD-B29B-3E14BC779C72}";
constexpr wchar_t kProbeUserFinalizerKey[] =
    L"Software\\Classes\\Local Settings\\Software\\Mo\\InputMethod\\Tests\\"
    L"{24CD1DC4-1619-4A29-8E89-761D50F02A6D}";

template <typename Interface>
class ComPtr final {
public:
    ComPtr() = default;
    ~ComPtr() {
        if (value_ != nullptr) {
            value_->Release();
        }
    }
    ComPtr(const ComPtr&) = delete;
    ComPtr& operator=(const ComPtr&) = delete;
    Interface** put() noexcept { return &value_; }
    Interface* operator->() const noexcept { return value_; }

private:
    Interface* value_ = nullptr;
};

class RegistryKey final {
public:
    RegistryKey() = default;
    ~RegistryKey() {
        if (value_ != nullptr) {
            RegCloseKey(value_);
        }
    }
    RegistryKey(const RegistryKey&) = delete;
    RegistryKey& operator=(const RegistryKey&) = delete;
    HKEY* put() noexcept { return &value_; }
    HKEY get() const noexcept { return value_; }

private:
    HKEY value_ = nullptr;
};

class UserFinalizerLock final {
public:
    UserFinalizerLock() = default;
    ~UserFinalizerLock() {
        if (acquired_) {
            ReleaseMutex(value_);
        }
        if (value_ != nullptr) {
            CloseHandle(value_);
        }
    }
    UserFinalizerLock(const UserFinalizerLock&) = delete;
    UserFinalizerLock& operator=(const UserFinalizerLock&) = delete;

    HRESULT acquire() noexcept {
        value_ = CreateMutexW(nullptr, FALSE, kUserFinalizerMutex);
        if (value_ == nullptr) {
            return HRESULT_FROM_WIN32(GetLastError());
        }
        const DWORD waited = WaitForSingleObject(value_, 30'000);
        if (waited == WAIT_OBJECT_0 || waited == WAIT_ABANDONED) {
            acquired_ = true;
            return S_OK;
        }
        if (waited == WAIT_TIMEOUT) {
            return HRESULT_FROM_WIN32(ERROR_TIMEOUT);
        }
        if (waited == WAIT_FAILED) {
            return HRESULT_FROM_WIN32(GetLastError());
        }
        return E_UNEXPECTED;
    }

private:
    HANDLE value_ = nullptr;
    bool acquired_ = false;
};

HRESULT hresult_from_registry(LSTATUS status) noexcept {
    return status == ERROR_SUCCESS
        ? S_OK
        : HRESULT_FROM_WIN32(static_cast<unsigned long>(status));
}

bool is_absolute_local_path(const std::wstring& path) noexcept {
    const bool drive_path = path.size() >= 3
        && ((path[0] >= L'A' && path[0] <= L'Z') || (path[0] >= L'a' && path[0] <= L'z'))
        && path[1] == L':'
        && (path[2] == L'\\' || path[2] == L'/');
    if (!drive_path || path.find(L':', 2) != std::wstring::npos) {
        return false;
    }
    return true;
}

bool is_absolute_file(const std::wstring& path) noexcept {
    if (!is_absolute_local_path(path)) {
        return false;
    }
    const DWORD attributes = GetFileAttributesW(path.c_str());
    return attributes != INVALID_FILE_ATTRIBUTES && (attributes & FILE_ATTRIBUTE_DIRECTORY) == 0;
}

bool is_64_bit_windows() noexcept {
#if defined(_WIN64)
    return true;
#else
    BOOL wow64 = FALSE;
    return IsWow64Process(GetCurrentProcess(), &wow64) != FALSE && wow64 != FALSE;
#endif
}

HRESULT query_process_elevation(bool& elevated) noexcept {
    elevated = false;
    HANDLE token = nullptr;
    if (OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token) == FALSE) {
        return HRESULT_FROM_WIN32(GetLastError());
    }
    TOKEN_ELEVATION elevation{};
    DWORD bytes = 0;
    const BOOL queried = GetTokenInformation(
        token, TokenElevation, &elevation, sizeof(elevation), &bytes);
    const DWORD last_error = queried != FALSE ? ERROR_SUCCESS : GetLastError();
    CloseHandle(token);
    if (queried == FALSE) {
        return HRESULT_FROM_WIN32(last_error);
    }
    elevated = elevation.TokenIsElevated != 0;
    return S_OK;
}

HRESULT require_elevated_process() noexcept {
    bool elevated = false;
    const HRESULT result = query_process_elevation(elevated);
    if (FAILED(result)) {
        return result;
    }
    return elevated ? S_OK : HRESULT_FROM_WIN32(ERROR_ELEVATION_REQUIRED);
}

HRESULT validate_user_finalizer_process_context(
    bool elevated,
    DWORD session,
    bool app_container) noexcept {
    if (elevated) {
        return HRESULT_FROM_WIN32(ERROR_ACCESS_DENIED);
    }
    if (session == 0) {
        return HRESULT_FROM_WIN32(ERROR_NOT_LOGGED_ON);
    }
    return app_container ? HRESULT_FROM_WIN32(ERROR_ACCESS_DENIED) : S_OK;
}

HRESULT require_standard_current_user_process() noexcept {
    bool elevated = false;
    HRESULT result = query_process_elevation(elevated);
    if (FAILED(result)) {
        return result;
    }
    DWORD session = 0;
    if (ProcessIdToSessionId(GetCurrentProcessId(), &session) == FALSE) {
        return HRESULT_FROM_WIN32(GetLastError());
    }
    HANDLE token = nullptr;
    if (OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token) == FALSE) {
        return HRESULT_FROM_WIN32(GetLastError());
    }
    DWORD app_container = 0;
    DWORD bytes = 0;
    const BOOL queried = GetTokenInformation(
        token, TokenIsAppContainer, &app_container, sizeof(app_container), &bytes);
    const DWORD last_error = queried != FALSE ? ERROR_SUCCESS : GetLastError();
    CloseHandle(token);
    if (queried == FALSE) {
        return HRESULT_FROM_WIN32(last_error);
    }
    return validate_user_finalizer_process_context(
        elevated, session, app_container != 0);
}

HRESULT create_profile_manager(ComPtr<ITfInputProcessorProfileMgr>& manager) noexcept {
    return CoCreateInstance(
        CLSID_TF_InputProcessorProfiles,
        nullptr,
        CLSCTX_INPROC_SERVER,
        IID_ITfInputProcessorProfileMgr,
        reinterpret_cast<void**>(manager.put()));
}

HRESULT create_category_manager(ComPtr<ITfCategoryMgr>& manager) noexcept {
    return CoCreateInstance(
        CLSID_TF_CategoryMgr,
        nullptr,
        CLSCTX_INPROC_SERVER,
        IID_ITfCategoryMgr,
        reinterpret_cast<void**>(manager.put()));
}

struct MachineProfileState final {
    bool profile = false;
    bool category = false;
};

enum class TransactionKind {
    install,
    remove,
};

enum class UserFinalizerTransactionKind {
    none,
    install,
    repair,
    remove,
};

struct UserFinalizerTransaction final {
    UserFinalizerTransactionKind kind = UserFinalizerTransactionKind::none;
    bool previous_enabled = false;
};

HRESULT query_profile_exists(
    ITfInputProcessorProfileMgr* manager,
    bool& exists) noexcept {
    exists = false;
    ComPtr<IEnumTfInputProcessorProfiles> enumerator;
    HRESULT result = manager->EnumProfiles(kLanguage, enumerator.put());
    if (FAILED(result)) {
        return result;
    }
    for (;;) {
        TF_INPUTPROCESSORPROFILE profile{};
        ULONG fetched = 0;
        result = enumerator->Next(1, &profile, &fetched);
        if (result == S_FALSE || fetched == 0) {
            return S_OK;
        }
        if (FAILED(result)) {
            return result;
        }
        if (profile.dwProfileType == TF_PROFILETYPE_INPUTPROCESSOR
            && IsEqualGUID(profile.clsid, mo::windows_tip::kTextServiceClsid)
            && IsEqualGUID(profile.guidProfile, mo::windows_tip::kSimplifiedChineseProfileGuid)) {
            exists = true;
            return S_OK;
        }
    }
}

HRESULT query_registry_key_exists_at(
    HKEY root, const wchar_t* key_path, REGSAM view, bool& exists) noexcept;

HRESULT query_machine_profile_state(MachineProfileState& state) noexcept {
    // TSF category enumeration is scoped to the caller's context. In an MSI
    // deferred action running as SYSTEM it can omit a category that the same
    // TSF manager just registered under HKLM. Read the machine registration
    // that the transaction actually owns, in the native registry view.
    HRESULT result = query_registry_key_exists_at(
        HKEY_LOCAL_MACHINE, kMachineProfileKey, KEY_WOW64_64KEY, state.profile);
    if (FAILED(result)) {
        return result;
    }
    bool by_category = false;
    result = query_registry_key_exists_at(
        HKEY_LOCAL_MACHINE, kMachineCategoryByCategoryKey, KEY_WOW64_64KEY, by_category);
    if (FAILED(result)) {
        return result;
    }
    bool by_item = false;
    result = query_registry_key_exists_at(
        HKEY_LOCAL_MACHINE, kMachineCategoryByItemKey, KEY_WOW64_64KEY, by_item);
    if (FAILED(result)) {
        return result;
    }
    if (by_category != by_item) {
        return HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
    }
    state.category = by_category;
    return S_OK;
}

HRESULT register_profile(
    ITfInputProcessorProfileMgr* profiles,
    const std::wstring& icon_path) noexcept {
    constexpr ULONG description_length =
        static_cast<ULONG>((sizeof(mo::windows_tip::kProfileDescription) / sizeof(wchar_t)) - 1);
    return profiles->RegisterProfile(
        mo::windows_tip::kTextServiceClsid,
        kLanguage,
        mo::windows_tip::kSimplifiedChineseProfileGuid,
        mo::windows_tip::kProfileDescription,
        description_length,
        icon_path.c_str(),
        static_cast<ULONG>(icon_path.size()),
        0,
        nullptr,
        0,
        FALSE,
        0);
}

HRESULT apply_machine_profile_state(
    ITfInputProcessorProfileMgr* profiles,
    ITfCategoryMgr* categories,
    const std::wstring& icon_path,
    const MachineProfileState& target) noexcept {
    MachineProfileState current{};
    HRESULT result = query_machine_profile_state(current);
    if (FAILED(result)) {
        return result;
    }
    HRESULT first_failure = S_OK;
    if (target.profile) {
        const HRESULT operation = register_profile(profiles, icon_path);
        if (FAILED(operation)) {
            first_failure = operation;
        }
    }
    if (target.category) {
        const HRESULT operation = categories->RegisterCategory(
            mo::windows_tip::kTextServiceClsid,
            GUID_TFCAT_TIP_KEYBOARD,
            mo::windows_tip::kTextServiceClsid);
        if (FAILED(operation) && SUCCEEDED(first_failure)) {
            first_failure = operation;
        }
    }
    if (!target.category && current.category) {
        const HRESULT operation = categories->UnregisterCategory(
            mo::windows_tip::kTextServiceClsid,
            GUID_TFCAT_TIP_KEYBOARD,
            mo::windows_tip::kTextServiceClsid);
        if (FAILED(operation) && SUCCEEDED(first_failure)) {
            first_failure = operation;
        }
    }
    if (!target.profile && current.profile) {
        const HRESULT operation = profiles->UnregisterProfile(
            mo::windows_tip::kTextServiceClsid,
            kLanguage,
            mo::windows_tip::kSimplifiedChineseProfileGuid,
            0);
        if (FAILED(operation) && SUCCEEDED(first_failure)) {
            first_failure = operation;
        }
    }
    if (FAILED(first_failure)) {
        return first_failure;
    }
    MachineProfileState final_state{};
    result = query_machine_profile_state(final_state);
    if (FAILED(result)) {
        return result;
    }
    if (final_state.profile != target.profile || final_state.category != target.category) {
        std::wcerr << L"Machine profile readback mismatch: profile=" << final_state.profile
                   << L", category=" << final_state.category
                   << L", target.profile=" << target.profile
                   << L", target.category=" << target.category << L'\n';
        return E_UNEXPECTED;
    }
    return S_OK;
}

HRESULT get_program_files_x64(std::wstring& path) noexcept {
    path.clear();
    PWSTR value = nullptr;
    const HRESULT known_folder = SHGetKnownFolderPath(
        FOLDERID_ProgramFilesX64,
        KF_FLAG_DEFAULT,
        nullptr,
        &value);
    if (SUCCEEDED(known_folder) && value != nullptr) {
        try {
            path.assign(value);
        } catch (...) {
            CoTaskMemFree(value);
            return E_OUTOFMEMORY;
        }
        CoTaskMemFree(value);
        return S_OK;
    }
    if (value != nullptr) {
        CoTaskMemFree(value);
    }

    RegistryKey key;
    LSTATUS status = RegOpenKeyExW(
        HKEY_LOCAL_MACHINE,
        L"SOFTWARE\\Microsoft\\Windows\\CurrentVersion",
        0,
        KEY_QUERY_VALUE | KEY_WOW64_64KEY,
        key.put());
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }
    std::array<wchar_t, 32768> buffer{};
    DWORD bytes = static_cast<DWORD>(buffer.size() * sizeof(wchar_t));
    status = RegGetValueW(
        key.get(),
        nullptr,
        L"ProgramFilesDir",
        RRF_RT_REG_SZ | RRF_ZEROONFAILURE,
        nullptr,
        buffer.data(),
        &bytes);
    if (status != ERROR_SUCCESS || bytes < sizeof(wchar_t)) {
        return status == ERROR_SUCCESS
            ? HRESULT_FROM_WIN32(ERROR_INVALID_DATA)
            : hresult_from_registry(status);
    }
    try {
        path.assign(buffer.data());
    } catch (...) {
        return E_OUTOFMEMORY;
    }
    return S_OK;
}

HRESULT get_fixed_machine_paths(
    std::wstring& icon_path,
    std::wstring& install_marker,
    std::wstring& remove_marker) noexcept {
    std::wstring program_files;
    HRESULT result = get_program_files_x64(program_files);
    if (FAILED(result)) {
        return result;
    }
    try {
        while (!program_files.empty()
               && (program_files.back() == L'\\' || program_files.back() == L'/')) {
            program_files.pop_back();
        }
        icon_path = program_files + kFixedTipSuffix;
        install_marker = program_files + kInstallMarkerSuffix;
        remove_marker = program_files + kRemoveMarkerSuffix;
    } catch (...) {
        return E_OUTOFMEMORY;
    }
    return S_OK;
}

HRESULT get_fixed_tip_paths(
    std::wstring& x64_path,
    std::wstring& x86_path) noexcept {
    std::wstring program_files;
    HRESULT result = get_program_files_x64(program_files);
    if (FAILED(result)) {
        return result;
    }
    try {
        while (!program_files.empty()
               && (program_files.back() == L'\\' || program_files.back() == L'/')) {
            program_files.pop_back();
        }
        x64_path = program_files + kFixedTipSuffix;
        x86_path = program_files + kFixedTipX86Suffix;
    } catch (...) {
        return E_OUTOFMEMORY;
    }
    return S_OK;
}

std::string marker_contents(TransactionKind kind, const MachineProfileState& state) {
    std::string value = "mo-machine-profile-transaction-v1 ";
    value += kind == TransactionKind::install ? "install " : "remove ";
    value += state.profile ? '1' : '0';
    value += ' ';
    value += state.category ? '1' : '0';
    value += '\n';
    return value;
}

HRESULT create_transaction_marker(
    const std::wstring& path,
    TransactionKind kind,
    const MachineProfileState& state) noexcept {
    HANDLE file = CreateFileW(
        path.c_str(),
        GENERIC_WRITE,
        0,
        nullptr,
        CREATE_NEW,
        FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_NOT_CONTENT_INDEXED,
        nullptr);
    if (file == INVALID_HANDLE_VALUE) {
        return HRESULT_FROM_WIN32(GetLastError());
    }
    const std::string contents = marker_contents(kind, state);
    DWORD written = 0;
    const BOOL wrote = WriteFile(
        file,
        contents.data(),
        static_cast<DWORD>(contents.size()),
        &written,
        nullptr);
    DWORD error = ERROR_SUCCESS;
    if (wrote == FALSE) {
        error = GetLastError();
    } else if (written != contents.size()) {
        error = ERROR_WRITE_FAULT;
    } else if (FlushFileBuffers(file) == FALSE) {
        error = GetLastError();
    }
    CloseHandle(file);
    if (error != ERROR_SUCCESS) {
        DeleteFileW(path.c_str());
        return HRESULT_FROM_WIN32(error);
    }
    return S_OK;
}

HRESULT read_transaction_marker(
    const std::wstring& path,
    TransactionKind expected_kind,
    bool& exists,
    MachineProfileState& state) noexcept {
    exists = false;
    state = {};
    HANDLE file = CreateFileW(
        path.c_str(),
        GENERIC_READ,
        0,
        nullptr,
        OPEN_EXISTING,
        FILE_FLAG_OPEN_REPARSE_POINT,
        nullptr);
    if (file == INVALID_HANDLE_VALUE) {
        const DWORD error = GetLastError();
        return error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND
            ? S_OK
            : HRESULT_FROM_WIN32(error);
    }
    FILE_ATTRIBUTE_TAG_INFO tag{};
    if (GetFileInformationByHandleEx(file, FileAttributeTagInfo, &tag, sizeof(tag)) == FALSE) {
        const DWORD error = GetLastError();
        CloseHandle(file);
        return HRESULT_FROM_WIN32(error);
    }
    if ((tag.FileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)) != 0) {
        CloseHandle(file);
        return HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
    }
    std::array<char, 96> buffer{};
    DWORD bytes = 0;
    const BOOL read = ReadFile(
        file,
        buffer.data(),
        static_cast<DWORD>(buffer.size()),
        &bytes,
        nullptr);
    const DWORD read_error = read == FALSE ? GetLastError() : ERROR_SUCCESS;
    CloseHandle(file);
    if (read == FALSE) {
        return HRESULT_FROM_WIN32(read_error);
    }
    const std::string actual(buffer.data(), bytes);
    for (const bool profile : {false, true}) {
        for (const bool category : {false, true}) {
            const MachineProfileState candidate{profile, category};
            if (actual == marker_contents(expected_kind, candidate)) {
                state = candidate;
                exists = true;
                return S_OK;
            }
        }
    }
    return HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
}

HRESULT delete_transaction_marker(
    const std::wstring& path,
    TransactionKind expected_kind,
    bool missing_is_success) noexcept {
    bool exists = false;
    MachineProfileState ignored{};
    HRESULT result = read_transaction_marker(path, expected_kind, exists, ignored);
    if (FAILED(result)) {
        return result;
    }
    if (!exists) {
        return missing_is_success ? S_OK : HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND);
    }
    return DeleteFileW(path.c_str()) != FALSE
        ? S_OK
        : HRESULT_FROM_WIN32(GetLastError());
}

HRESULT query_registry_string_at(
    HKEY root,
    const wchar_t* key_path,
    REGSAM view,
    const wchar_t* value_name,
    std::wstring& value,
    bool& exists) noexcept {
    exists = false;
    value.clear();
    RegistryKey key;
    LSTATUS status = RegOpenKeyExW(
        root,
        key_path,
        0,
        KEY_QUERY_VALUE | view,
        key.put());
    if (status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND) {
        return S_OK;
    }
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }

    DWORD type = 0;
    DWORD bytes = 0;
    status = RegQueryValueExW(key.get(), value_name, nullptr, &type, nullptr, &bytes);
    if (status == ERROR_FILE_NOT_FOUND) {
        return S_OK;
    }
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }
    if (type != REG_SZ || bytes == 0 || bytes > kMaximumRegistryStringBytes
        || bytes % sizeof(wchar_t) != 0) {
        return HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
    }

    const DWORD expected_bytes = bytes;
    std::vector<wchar_t> buffer(bytes / sizeof(wchar_t), L'\0');
    status = RegQueryValueExW(
        key.get(),
        value_name,
        nullptr,
        &type,
        reinterpret_cast<BYTE*>(buffer.data()),
        &bytes);
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }
    if (type != REG_SZ || bytes != expected_bytes || bytes == 0
        || bytes % sizeof(wchar_t) != 0 || buffer.empty() || buffer.back() != L'\0'
        || std::find(buffer.begin(), buffer.end() - 1, L'\0') != buffer.end() - 1) {
        return HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
    }
    value.assign(buffer.begin(), buffer.end() - 1);
    exists = true;
    return S_OK;
}

HRESULT query_registry_key_exists_at(
    HKEY root,
    const wchar_t* key_path,
    REGSAM view,
    bool& exists) noexcept {
    exists = false;
    RegistryKey key;
    const LSTATUS status = RegOpenKeyExW(
        root, key_path, 0, KEY_READ | view, key.put());
    if (status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND) {
        return S_OK;
    }
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }
    exists = true;
    return S_OK;
}

HRESULT query_registry_string(
    const wchar_t* key_path,
    REGSAM view,
    const wchar_t* value_name,
    std::wstring& value,
    bool& exists) noexcept {
    return query_registry_string_at(
        HKEY_CURRENT_USER, key_path, view, value_name, value, exists);
}

HRESULT query_user_finalizer_marker_at(
    const wchar_t* key_path,
    bool& exists) noexcept {
    std::wstring value;
    HRESULT result = query_registry_string(
        key_path, KEY_WOW64_64KEY, kUserFinalizerValue, value, exists);
    if (FAILED(result) || !exists) {
        return result;
    }
    return value == kUserFinalizerMarker
        ? S_OK
        : HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
}

const wchar_t* user_finalizer_transaction_value(
    const UserFinalizerTransaction& transaction) noexcept {
    switch (transaction.kind) {
    case UserFinalizerTransactionKind::install:
        return transaction.previous_enabled
            ? L"mo-user-finalizer-install-v1-enabled"
            : L"mo-user-finalizer-install-v1-disabled";
    case UserFinalizerTransactionKind::repair:
        return transaction.previous_enabled
            ? L"mo-user-finalizer-repair-v1-enabled"
            : L"mo-user-finalizer-repair-v1-disabled";
    case UserFinalizerTransactionKind::remove:
        return transaction.previous_enabled
            ? L"mo-user-finalizer-remove-v1-enabled"
            : L"mo-user-finalizer-remove-v1-disabled";
    case UserFinalizerTransactionKind::none:
        return nullptr;
    }
    return nullptr;
}

HRESULT query_user_finalizer_transaction_at(
    const wchar_t* key_path,
    UserFinalizerTransaction& transaction) noexcept {
    transaction = {};
    std::wstring value;
    bool exists = false;
    const HRESULT result = query_registry_string(
        key_path, KEY_WOW64_64KEY, kUserFinalizerTransactionValue, value, exists);
    if (FAILED(result) || !exists) {
        return result;
    }
    for (const UserFinalizerTransaction candidate : std::array{
             UserFinalizerTransaction{UserFinalizerTransactionKind::install, false},
             UserFinalizerTransaction{UserFinalizerTransactionKind::install, true},
             UserFinalizerTransaction{UserFinalizerTransactionKind::repair, false},
             UserFinalizerTransaction{UserFinalizerTransactionKind::repair, true},
             UserFinalizerTransaction{UserFinalizerTransactionKind::remove, false},
             UserFinalizerTransaction{UserFinalizerTransactionKind::remove, true}}) {
        if (value == user_finalizer_transaction_value(candidate)) {
            transaction = candidate;
            return S_OK;
        }
    }
    return HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
}

HRESULT write_user_finalizer_value_at(
    const wchar_t* key_path,
    const wchar_t* value_name,
    const wchar_t* value,
    bool& written) noexcept {
    written = false;
    if (value == nullptr) {
        return E_INVALIDARG;
    }
    RegistryKey key;
    LSTATUS status = RegCreateKeyExW(
        HKEY_CURRENT_USER,
        key_path,
        0,
        nullptr,
        REG_OPTION_NON_VOLATILE,
        KEY_QUERY_VALUE | KEY_SET_VALUE | KEY_WOW64_64KEY,
        nullptr,
        key.put(),
        nullptr);
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }
    status = RegSetValueExW(
        key.get(),
        value_name,
        0,
        REG_SZ,
        reinterpret_cast<const BYTE*>(value),
        static_cast<DWORD>(
            (std::char_traits<wchar_t>::length(value) + 1) * sizeof(wchar_t)));
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }
    written = true;
    status = RegFlushKey(key.get());
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }
    std::wstring actual;
    bool exists = false;
    const HRESULT result = query_registry_string(
        key_path, KEY_WOW64_64KEY, value_name, actual, exists);
    return SUCCEEDED(result) && exists && actual == value
        ? S_OK
        : (FAILED(result) ? result : E_UNEXPECTED);
}

HRESULT delete_user_finalizer_value_at(
    const wchar_t* key_path,
    const wchar_t* value_name,
    const wchar_t* expected_value,
    bool missing_is_success,
    bool& deleted) noexcept {
    deleted = false;
    std::wstring actual;
    bool exists = false;
    HRESULT result = query_registry_string(
        key_path, KEY_WOW64_64KEY, value_name, actual, exists);
    if (FAILED(result)) {
        return result;
    }
    if (!exists) {
        return missing_is_success ? S_OK : HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND);
    }
    if (actual != expected_value) {
        return HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
    }
    RegistryKey key;
    LSTATUS status = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        key_path,
        0,
        KEY_QUERY_VALUE | KEY_SET_VALUE | KEY_WOW64_64KEY,
        key.put());
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }
    status = RegDeleteValueW(key.get(), value_name);
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }
    deleted = true;
    status = RegFlushKey(key.get());
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }
    actual.clear();
    exists = true;
    result = query_registry_string(
        key_path, KEY_WOW64_64KEY, value_name, actual, exists);
    return SUCCEEDED(result) && !exists ? S_OK : (FAILED(result) ? result : E_UNEXPECTED);
}

HRESULT create_user_finalizer_marker_at(
    const wchar_t* key_path,
    bool* marker_written = nullptr) noexcept {
    if (marker_written != nullptr) {
        *marker_written = false;
    }
    bool exists = false;
    HRESULT result = query_user_finalizer_marker_at(key_path, exists);
    if (FAILED(result)) {
        return result;
    }
    if (exists) {
        return HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS);
    }
    bool written = false;
    result = write_user_finalizer_value_at(
        key_path, kUserFinalizerValue, kUserFinalizerMarker, written);
    if (marker_written != nullptr) {
        *marker_written = written;
    }
    return result;
}

HRESULT delete_user_finalizer_marker_at(
    const wchar_t* key_path,
    bool missing_is_success,
    bool* marker_deleted = nullptr) noexcept {
    bool deleted = false;
    const HRESULT result = delete_user_finalizer_value_at(
        key_path,
        kUserFinalizerValue,
        kUserFinalizerMarker,
        missing_is_success,
        deleted);
    if (marker_deleted != nullptr) {
        *marker_deleted = deleted;
    }
    return result;
}

HRESULT write_user_finalizer_transaction_at(
    const wchar_t* key_path,
    const UserFinalizerTransaction& transaction) noexcept {
    const wchar_t* value = user_finalizer_transaction_value(transaction);
    bool written = false;
    HRESULT result = write_user_finalizer_value_at(
        key_path, kUserFinalizerTransactionValue, value, written);
    if (FAILED(result)) {
        return result;
    }
    UserFinalizerTransaction actual{};
    result = query_user_finalizer_transaction_at(key_path, actual);
    return SUCCEEDED(result) && actual.kind == transaction.kind
            && actual.previous_enabled == transaction.previous_enabled
        ? S_OK
        : (FAILED(result) ? result : E_UNEXPECTED);
}

HRESULT delete_user_finalizer_transaction_at(
    const wchar_t* key_path,
    const UserFinalizerTransaction& transaction,
    bool missing_is_success) noexcept {
    const wchar_t* value = user_finalizer_transaction_value(transaction);
    bool deleted = false;
    return delete_user_finalizer_value_at(
        key_path,
        kUserFinalizerTransactionValue,
        value,
        missing_is_success,
        deleted);
}

HRESULT verify_machine_installation_for_user_finalizer() noexcept {
    if (!is_64_bit_windows()) {
        return HRESULT_FROM_WIN32(ERROR_NOT_SUPPORTED);
    }
    std::wstring x64_path;
    std::wstring x86_path;
    HRESULT result = get_fixed_tip_paths(x64_path, x86_path);
    if (FAILED(result) || !is_absolute_file(x64_path) || !is_absolute_file(x86_path)) {
        return FAILED(result) ? result : HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND);
    }
    for (const auto& item : std::array{
             std::pair{KEY_WOW64_64KEY, std::cref(x64_path)},
             std::pair{KEY_WOW64_32KEY, std::cref(x86_path)}}) {
        // HKCR merges CLSID at the GUID child. Any per-user GUID key can hide
        // the machine registration even when its InprocServer32 is incomplete.
        bool user_shadow_exists = false;
        result = query_registry_key_exists_at(
            HKEY_CURRENT_USER, kComClsidPath, item.first, user_shadow_exists);
        if (FAILED(result) || user_shadow_exists) {
            return FAILED(result) ? result : HRESULT_FROM_WIN32(ERROR_INVALID_STATE);
        }
        std::wstring registered_path;
        bool exists = false;
        result = query_registry_string_at(
            HKEY_LOCAL_MACHINE, kComKey, item.first, nullptr, registered_path, exists);
        if (FAILED(result) || !exists
            || _wcsicmp(registered_path.c_str(), item.second.get().c_str()) != 0) {
            return FAILED(result) ? result : HRESULT_FROM_WIN32(ERROR_PRODUCT_UNINSTALLED);
        }
        std::wstring threading_model;
        result = query_registry_string_at(
            HKEY_LOCAL_MACHINE,
            kComKey,
            item.first,
            L"ThreadingModel",
            threading_model,
            exists);
        if (FAILED(result) || !exists || threading_model != L"Apartment") {
            return FAILED(result) ? result : HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
        }
    }
    ComPtr<ITfInputProcessorProfileMgr> profiles;
    ComPtr<ITfCategoryMgr> categories;
    result = create_profile_manager(profiles);
    if (SUCCEEDED(result)) {
        result = create_category_manager(categories);
    }
    if (FAILED(result)) {
        return result;
    }
    MachineProfileState state{};
    result = query_machine_profile_state(state);
    if (FAILED(result)) {
        return result;
    }
    return state.profile && state.category
        ? S_OK
        : HRESULT_FROM_WIN32(ERROR_PRODUCT_UNINSTALLED);
}

HRESULT delete_com_view(
    const wchar_t* parent_path,
    const wchar_t* clsid_key,
    REGSAM view,
    bool& removed) noexcept {
    removed = false;
    RegistryKey parent;
    LSTATUS status = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        parent_path,
        0,
        DELETE | KEY_ENUMERATE_SUB_KEYS | KEY_QUERY_VALUE | KEY_SET_VALUE | view,
        parent.put());
    if (status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND) {
        return S_OK;
    }
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }
    status = RegDeleteTreeW(parent.get(), clsid_key);
    if (status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND) {
        return S_OK;
    }
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }
    removed = true;
    return S_OK;
}

HRESULT register_com_view(
    const wchar_t* key_path,
    const wchar_t* parent_path,
    const wchar_t* clsid_key,
    REGSAM view,
    const std::wstring& dll_path,
    bool& created) noexcept {
    created = false;
    std::wstring existing_path;
    bool exists = false;
    HRESULT result = query_registry_string(key_path, view, nullptr, existing_path, exists);
    if (FAILED(result)) {
        return result;
    }
    if (exists && _wcsicmp(existing_path.c_str(), dll_path.c_str()) != 0) {
        return HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS);
    }

    RegistryKey key;
    DWORD disposition = 0;
    LSTATUS status = RegCreateKeyExW(
        HKEY_CURRENT_USER,
        key_path,
        0,
        nullptr,
        REG_OPTION_NON_VOLATILE,
        KEY_QUERY_VALUE | KEY_SET_VALUE | view,
        nullptr,
        key.put(),
        &disposition);
    if (status != ERROR_SUCCESS) {
        return hresult_from_registry(status);
    }
    created = disposition == REG_CREATED_NEW_KEY;
    const DWORD path_bytes = static_cast<DWORD>((dll_path.size() + 1) * sizeof(wchar_t));
    status = RegSetValueExW(
        key.get(),
        nullptr,
        0,
        REG_SZ,
        reinterpret_cast<const BYTE*>(dll_path.c_str()),
        path_bytes);
    if (status == ERROR_SUCCESS) {
        constexpr wchar_t threading_model[] = L"Apartment";
        status = RegSetValueExW(
            key.get(),
            L"ThreadingModel",
            0,
            REG_SZ,
            reinterpret_cast<const BYTE*>(threading_model),
            sizeof(threading_model));
    }
    if (status != ERROR_SUCCESS && created) {
        bool ignored = false;
        delete_com_view(parent_path, clsid_key, view, ignored);
        created = false;
    }
    return hresult_from_registry(status);
}

HRESULT register_com_current_user(
    const std::wstring& x64_path,
    const std::wstring& x86_path) noexcept {
    if (!is_64_bit_windows()) {
        return HRESULT_FROM_WIN32(ERROR_NOT_SUPPORTED);
    }
    if (!is_absolute_file(x64_path) || !is_absolute_file(x86_path)) {
        return E_INVALIDARG;
    }

    bool x64_created = false;
    HRESULT result = register_com_view(
        kComKey, kComParentKey, kComClsidKey, KEY_WOW64_64KEY, x64_path, x64_created);
    if (FAILED(result)) {
        return result;
    }
    bool x86_created = false;
    result = register_com_view(
        kComKey, kComParentKey, kComClsidKey, KEY_WOW64_32KEY, x86_path, x86_created);
    if (FAILED(result) && x64_created) {
        bool ignored = false;
        delete_com_view(kComParentKey, kComClsidKey, KEY_WOW64_64KEY, ignored);
    }
    return result;
}

HRESULT unregister_com_current_user() noexcept {
    bool ignored = false;
    const HRESULT x64_result = delete_com_view(
        kComParentKey, kComClsidKey, KEY_WOW64_64KEY, ignored);
    const HRESULT x86_result = delete_com_view(
        kComParentKey, kComClsidKey, KEY_WOW64_32KEY, ignored);
    return FAILED(x64_result) ? x64_result : x86_result;
}

HRESULT create_machine_managers(
    ComPtr<ITfInputProcessorProfileMgr>& profiles,
    ComPtr<ITfCategoryMgr>& categories) noexcept {
    HRESULT result = create_profile_manager(profiles);
    if (FAILED(result)) {
        return result;
    }
    return create_category_manager(categories);
}

HRESULT change_machine_profile_without_marker(
    const std::wstring& icon_path,
    const MachineProfileState& target) noexcept {
    const HRESULT elevation_result = require_elevated_process();
    if (FAILED(elevation_result)) {
        return elevation_result;
    }
    if (target.profile && !is_absolute_file(icon_path)) {
        return E_INVALIDARG;
    }

    ComPtr<ITfInputProcessorProfileMgr> profiles;
    ComPtr<ITfCategoryMgr> categories;
    HRESULT result = create_machine_managers(profiles, categories);
    if (FAILED(result)) {
        return result;
    }
    MachineProfileState previous{};
    result = query_machine_profile_state(previous);
    if (FAILED(result)) {
        return result;
    }
    result = apply_machine_profile_state(
        profiles.operator->(), categories.operator->(), icon_path, target);
    if (FAILED(result)) {
        const HRESULT rollback_result = apply_machine_profile_state(
            profiles.operator->(), categories.operator->(), icon_path, previous);
        if (FAILED(rollback_result)) {
            std::wcerr << L"Machine profile in-process rollback failed: 0x"
                       << std::hex << rollback_result << L'\n';
        }
    }
    return result;
}

HRESULT register_machine_profile(const std::wstring& icon_path) noexcept {
    return change_machine_profile_without_marker(icon_path, {true, true});
}

HRESULT unregister_machine_profile() noexcept {
    const HRESULT elevation_result = require_elevated_process();
    if (FAILED(elevation_result)) {
        return elevation_result;
    }
    ComPtr<ITfInputProcessorProfileMgr> profiles;
    ComPtr<ITfCategoryMgr> categories;
    HRESULT result = create_machine_managers(profiles, categories);
    if (FAILED(result)) {
        return result;
    }
    return apply_machine_profile_state(
        profiles.operator->(), categories.operator->(), std::wstring{}, {false, false});
}

HRESULT begin_machine_profile_transaction(
    TransactionKind kind,
    const std::wstring& icon_path,
    const std::wstring& marker_path,
    const MachineProfileState& target,
    bool require_absent_previous = false) noexcept {
    const HRESULT elevation_result = require_elevated_process();
    if (FAILED(elevation_result)) {
        return elevation_result;
    }
    if (!is_absolute_local_path(marker_path) || (target.profile && !is_absolute_file(icon_path))) {
        return E_INVALIDARG;
    }
    ComPtr<ITfInputProcessorProfileMgr> profiles;
    ComPtr<ITfCategoryMgr> categories;
    HRESULT result = create_machine_managers(profiles, categories);
    if (FAILED(result)) {
        return result;
    }
    MachineProfileState previous{};
    result = query_machine_profile_state(previous);
    if (FAILED(result)) {
        return result;
    }
    if (require_absent_previous && (previous.profile || previous.category)) {
        return HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS);
    }
    result = create_transaction_marker(marker_path, kind, previous);
    if (FAILED(result)) {
        return result;
    }
    // Leave the marker on any failure. The paired MSI rollback action owns
    // restoration even if this process is cancelled between native calls.
    return apply_machine_profile_state(
        profiles.operator->(), categories.operator->(), icon_path, target);
}

HRESULT rollback_machine_profile_transaction(
    TransactionKind kind,
    const std::wstring& icon_path,
    const std::wstring& marker_path) noexcept {
    const HRESULT elevation_result = require_elevated_process();
    if (FAILED(elevation_result)) {
        return elevation_result;
    }
    bool exists = false;
    MachineProfileState previous{};
    HRESULT result = read_transaction_marker(marker_path, kind, exists, previous);
    if (FAILED(result) || !exists) {
        return result;
    }
    if (previous.profile && !is_absolute_file(icon_path)) {
        return E_INVALIDARG;
    }
    ComPtr<ITfInputProcessorProfileMgr> profiles;
    ComPtr<ITfCategoryMgr> categories;
    result = create_machine_managers(profiles, categories);
    if (FAILED(result)) {
        return result;
    }
    result = apply_machine_profile_state(
        profiles.operator->(), categories.operator->(), icon_path, previous);
    if (FAILED(result)) {
        return result;
    }
    return delete_transaction_marker(marker_path, kind, false);
}

HRESULT commit_machine_profile_transaction(
    TransactionKind kind,
    const std::wstring& marker_path) noexcept {
    const HRESULT elevation_result = require_elevated_process();
    if (FAILED(elevation_result)) {
        return elevation_result;
    }
    return delete_transaction_marker(marker_path, kind, false);
}

HRESULT remove_empty_directory(const std::wstring& path) noexcept {
    const DWORD attributes = GetFileAttributesW(path.c_str());
    if (attributes == INVALID_FILE_ATTRIBUTES) {
        const DWORD error = GetLastError();
        return error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND
            ? S_OK : HRESULT_FROM_WIN32(error);
    }
    if ((attributes & FILE_ATTRIBUTE_DIRECTORY) == 0
        || (attributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0) {
        return HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
    }
    if (RemoveDirectoryW(path.c_str()) != FALSE) {
        return S_OK;
    }
    const DWORD error = GetLastError();
    // Preserve unknown files. This cleanup never enumerates or deletes children.
    return error == ERROR_DIR_NOT_EMPTY ? S_FALSE : HRESULT_FROM_WIN32(error);
}

HRESULT fixed_machine_profile_operation(std::wstring_view command) noexcept {
    std::wstring icon_path;
    std::wstring install_marker;
    std::wstring remove_marker;
    HRESULT result = get_fixed_machine_paths(icon_path, install_marker, remove_marker);
    if (FAILED(result)) {
        return result;
    }
    if (command == L"install-machine-profile-fixed") {
        return begin_machine_profile_transaction(
            TransactionKind::install, icon_path, install_marker, {true, true});
    }
    if (command == L"install-new-machine-profile-fixed") {
        return begin_machine_profile_transaction(
            TransactionKind::install, icon_path, install_marker, {true, true}, true);
    }
    if (command == L"rollback-install-machine-profile-fixed") {
        return rollback_machine_profile_transaction(
            TransactionKind::install, icon_path, install_marker);
    }
    if (command == L"commit-install-machine-profile-fixed") {
        return commit_machine_profile_transaction(TransactionKind::install, install_marker);
    }
    if (command == L"remove-machine-profile-fixed") {
        return begin_machine_profile_transaction(
            TransactionKind::remove, icon_path, remove_marker, {false, false});
    }
    if (command == L"rollback-remove-machine-profile-fixed") {
        return rollback_machine_profile_transaction(
            TransactionKind::remove, icon_path, remove_marker);
    }
    if (command == L"commit-remove-machine-profile-fixed") {
        result = commit_machine_profile_transaction(TransactionKind::remove, remove_marker);
        if (FAILED(result)) {
            return result;
        }
        // RemoveFolders ran while the rollback receipt still occupied the root.
        // Once the removal is committed, remove only that now-empty fixed root.
        // A cleanup error must not initiate rollback after deleting the receipt.
        try {
            const std::wstring root = remove_marker.substr(0, remove_marker.find_last_of(L"\\/"));
            const HRESULT cleanup = remove_empty_directory(root);
            if (FAILED(cleanup)) {
                std::wcerr << L"Committed uninstall empty-root cleanup failed: 0x"
                           << std::hex << cleanup << L'\n';
            }
        } catch (...) {
            std::wcerr << L"Committed uninstall empty-root cleanup allocation failed.\n";
        }
        return S_OK;
    }
    return E_INVALIDARG;
}

HRESULT self_test_machine_transaction(const std::wstring& marker_path) noexcept {
    if (!is_absolute_local_path(marker_path)
        || GetFileAttributesW(marker_path.c_str()) != INVALID_FILE_ATTRIBUTES) {
        return E_INVALIDARG;
    }
    for (const TransactionKind kind : {TransactionKind::install, TransactionKind::remove}) {
        for (const bool profile : {false, true}) {
            for (const bool category : {false, true}) {
                const MachineProfileState expected{profile, category};
                HRESULT result = create_transaction_marker(marker_path, kind, expected);
                if (FAILED(result)) {
                    return result;
                }
                bool exists = false;
                MachineProfileState actual{};
                result = read_transaction_marker(marker_path, kind, exists, actual);
                if (FAILED(result) || !exists || actual.profile != expected.profile
                    || actual.category != expected.category) {
                    DeleteFileW(marker_path.c_str());
                    return FAILED(result) ? result : E_UNEXPECTED;
                }
                const HRESULT duplicate = create_transaction_marker(marker_path, kind, expected);
                if (duplicate != HRESULT_FROM_WIN32(ERROR_FILE_EXISTS)
                    && duplicate != HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS)) {
                    DeleteFileW(marker_path.c_str());
                    return E_UNEXPECTED;
                }
                result = delete_transaction_marker(marker_path, kind, false);
                if (FAILED(result)
                    || GetFileAttributesW(marker_path.c_str()) != INVALID_FILE_ATTRIBUTES) {
                    return FAILED(result) ? result : E_UNEXPECTED;
                }
            }
        }
    }
    bool exists = true;
    MachineProfileState state{};
    const HRESULT missing = read_transaction_marker(
        marker_path, TransactionKind::install, exists, state);
    if (FAILED(missing) || exists) {
        return E_UNEXPECTED;
    }
    const std::wstring directory = marker_path + L".empty-root";
    if (CreateDirectoryW(directory.c_str(), nullptr) == FALSE) {
        return HRESULT_FROM_WIN32(GetLastError());
    }
    const std::wstring child = directory + L"\\preserved-marker";
    HRESULT result = create_transaction_marker(child, TransactionKind::remove, {false, false});
    if (SUCCEEDED(result)) {
        if (remove_empty_directory(directory) != S_FALSE
            || GetFileAttributesW(child.c_str()) == INVALID_FILE_ATTRIBUTES) {
            result = E_UNEXPECTED;
        }
        const HRESULT cleanup = delete_transaction_marker(child, TransactionKind::remove, false);
        if (SUCCEEDED(result)) {
            result = cleanup;
        }
    }
    const HRESULT cleanup = remove_empty_directory(directory);
    if (FAILED(result)) {
        return result;
    }
    return cleanup == S_OK && remove_empty_directory(directory) == S_OK
        ? S_OK : E_UNEXPECTED;
}

std::wstring guid_string(REFGUID guid) {
    wchar_t buffer[40]{};
    if (StringFromGUID2(guid, buffer, static_cast<int>(std::size(buffer))) == 0) {
        return {};
    }
    return buffer;
}

HRESULT set_enabled_for_current_user(bool enabled) {
    using InstallLayoutOrTipFunction = BOOL(WINAPI*)(LPCWSTR, DWORD);
    HMODULE input = LoadLibraryExW(L"input.dll", nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (input == nullptr) {
        return HRESULT_FROM_WIN32(GetLastError());
    }
    const auto function = reinterpret_cast<InstallLayoutOrTipFunction>(
        GetProcAddress(input, "InstallLayoutOrTip"));
    if (function == nullptr) {
        const HRESULT error = HRESULT_FROM_WIN32(GetLastError());
        FreeLibrary(input);
        return error;
    }

    const std::wstring layout = L"0x0804:"
        + guid_string(mo::windows_tip::kTextServiceClsid)
        + guid_string(mo::windows_tip::kSimplifiedChineseProfileGuid);
    SetLastError(ERROR_SUCCESS);
    const BOOL succeeded = function(layout.c_str(), enabled ? 0 : kIlotUninstall);
    const DWORD last_error = GetLastError();
    FreeLibrary(input);
    if (succeeded != FALSE) {
        return S_OK;
    }
    return last_error == ERROR_SUCCESS ? E_FAIL : HRESULT_FROM_WIN32(last_error);
}

// ABI published for EnumEnabledLayoutOrTip; it has no Windows SDK header.
struct LayoutOrTipProfile final {
    DWORD profile_type;
    LANGID language;
    CLSID clsid;
    GUID profile;
    GUID category;
    DWORD substitute_layout;
    DWORD flags;
    WCHAR id[MAX_PATH];
};
constexpr DWORD kLayoutOrTipInputProcessor = 1;
constexpr DWORD kLayoutOrTipDisabled = 0x0002;

bool is_enabled_mo_profile(const LayoutOrTipProfile& profile) noexcept {
    return profile.profile_type == kLayoutOrTipInputProcessor
        && profile.language == kLanguage
        && IsEqualCLSID(profile.clsid, mo::windows_tip::kTextServiceClsid)
        && IsEqualGUID(profile.profile, mo::windows_tip::kSimplifiedChineseProfileGuid)
        && (profile.flags & kLayoutOrTipDisabled) == 0;
}

HRESULT query_current_user_enabled(bool& enabled) noexcept {
    enabled = false;
    using EnumEnabledLayoutOrTipFunction = UINT(WINAPI*)(
        LPCWSTR, LPCWSTR, LPCWSTR, LayoutOrTipProfile*, UINT);
    HMODULE input = LoadLibraryExW(L"input.dll", nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (input == nullptr) {
        return HRESULT_FROM_WIN32(GetLastError());
    }
    const auto function = reinterpret_cast<EnumEnabledLayoutOrTipFunction>(
        GetProcAddress(input, "EnumEnabledLayoutOrTip"));
    if (function == nullptr) {
        const HRESULT error = HRESULT_FROM_WIN32(GetLastError());
        FreeLibrary(input);
        return error;
    }
    // Read user settings rather than the process-wide TSF profile cache. Include
    // spare capacity for concurrent additions, and reject truncated snapshots.
    HRESULT result = HRESULT_FROM_WIN32(ERROR_RETRY);
    for (int attempt = 0; attempt != 3; ++attempt) {
        const UINT count = function(nullptr, nullptr, nullptr, nullptr, 0);
        if (count > 4096) {
            result = HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
            break;
        }
        const UINT capacity = count + 16;
        std::vector<LayoutOrTipProfile> items(capacity);
        const UINT copied = function(nullptr, nullptr, nullptr, items.data(), capacity);
        const UINT after = function(nullptr, nullptr, nullptr, nullptr, 0);
        if (copied > capacity || after > capacity || copied != after) {
            continue;
        }
        enabled = std::any_of(items.begin(), items.begin() + copied, is_enabled_mo_profile);
        result = S_OK;
        break;
    }
    FreeLibrary(input);
    return result;
}

HRESULT set_current_user_enabled_verified(bool enabled) noexcept {
    HRESULT result = set_enabled_for_current_user(enabled);
    if (FAILED(result)) {
        return result;
    }
    bool actual = false;
    result = query_current_user_enabled(actual);
    if (FAILED(result)) {
        return result;
    }
    if (actual != enabled) {
        std::wcerr << L"Current-user profile readback mismatch: enabled=" << actual
                   << L", target=" << enabled << L'\n';
        return E_UNEXPECTED;
    }
    return S_OK;
}

void report_user_state_rollback_failure(HRESULT result) noexcept {
    if (FAILED(result)) {
        std::wcerr << L"Current-user finalizer in-process rollback failed: 0x"
                   << std::hex << result << L'\n';
    }
}

enum class UserFinalizerOperation {
    install,
    repair,
    remove,
    rollback_install,
    rollback_remove,
};

HRESULT validate_user_finalizer_state(
    UserFinalizerOperation operation,
    bool marker_exists,
    const UserFinalizerTransaction& transaction,
    bool enabled) noexcept {
    switch (operation) {
    case UserFinalizerOperation::install:
        if (marker_exists || transaction.kind == UserFinalizerTransactionKind::repair) {
            return HRESULT_FROM_WIN32(
                marker_exists ? ERROR_ALREADY_EXISTS : ERROR_INVALID_STATE);
        }
        if (transaction.kind == UserFinalizerTransactionKind::install) {
            return transaction.previous_enabled
                ? HRESULT_FROM_WIN32(ERROR_INVALID_DATA)
                : S_OK;
        }
        return enabled ? HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS) : S_OK;
    case UserFinalizerOperation::repair:
    case UserFinalizerOperation::remove:
        return marker_exists ? S_OK : HRESULT_FROM_WIN32(ERROR_PRODUCT_UNINSTALLED);
    case UserFinalizerOperation::rollback_install:
        return transaction.kind == UserFinalizerTransactionKind::install
                && !transaction.previous_enabled
            ? S_OK
            : HRESULT_FROM_WIN32(ERROR_INVALID_STATE);
    case UserFinalizerOperation::rollback_remove:
        return transaction.kind == UserFinalizerTransactionKind::remove
            ? S_OK
            : HRESULT_FROM_WIN32(ERROR_INVALID_STATE);
    }
    return E_UNEXPECTED;
}

HRESULT install_or_repair_current_user_fixed(bool repair) noexcept {
    HRESULT result = require_standard_current_user_process();
    if (FAILED(result)) {
        return result;
    }
    UserFinalizerLock lock;
    result = lock.acquire();
    if (FAILED(result)) {
        return result;
    }
    result = verify_machine_installation_for_user_finalizer();
    if (FAILED(result)) {
        return result;
    }
    bool marker_exists = false;
    result = query_user_finalizer_marker_at(kUserFinalizerKey, marker_exists);
    if (FAILED(result)) {
        return result;
    }
    UserFinalizerTransaction existing_transaction{};
    result = query_user_finalizer_transaction_at(
        kUserFinalizerKey, existing_transaction);
    if (FAILED(result)) {
        return result;
    }
    ComPtr<ITfInputProcessorProfileMgr> profiles;
    result = create_profile_manager(profiles);
    if (FAILED(result)) {
        return result;
    }
    bool enabled = false;
    result = query_current_user_enabled(enabled);
    if (FAILED(result)) {
        return result;
    }
    const UserFinalizerOperation operation = repair
        ? UserFinalizerOperation::repair
        : UserFinalizerOperation::install;
    result = validate_user_finalizer_state(
        operation, marker_exists, existing_transaction, enabled);
    if (FAILED(result)) {
        return result;
    }

    // Preserve the original state when resuming the same interrupted action.
    // The journal intentionally survives success so a later Burn inverse action
    // in another process can restore the exact pre-action enabled bit.
    UserFinalizerTransaction transaction{
        repair ? UserFinalizerTransactionKind::repair
               : UserFinalizerTransactionKind::install,
        enabled};
    if ((repair && existing_transaction.kind == UserFinalizerTransactionKind::repair)
        || (!repair && existing_transaction.kind == UserFinalizerTransactionKind::install)) {
        transaction = existing_transaction;
    }
    result = write_user_finalizer_transaction_at(kUserFinalizerKey, transaction);
    if (FAILED(result)) {
        return result;
    }
    result = set_current_user_enabled_verified(true);
    if (FAILED(result)) {
        report_user_state_rollback_failure(
            set_current_user_enabled_verified(transaction.previous_enabled));
        return result;
    }
    if (!repair) {
        bool marker_written = false;
        result = create_user_finalizer_marker_at(kUserFinalizerKey, &marker_written);
        if (FAILED(result)) {
            if (!marker_written) {
                report_user_state_rollback_failure(
                    set_current_user_enabled_verified(transaction.previous_enabled));
            }
            return result;
        }
    }
    return S_OK;
}

HRESULT remove_current_user_fixed() noexcept {
    HRESULT result = require_standard_current_user_process();
    if (FAILED(result)) {
        return result;
    }
    UserFinalizerLock lock;
    result = lock.acquire();
    if (FAILED(result)) {
        return result;
    }
    bool marker_exists = false;
    result = query_user_finalizer_marker_at(kUserFinalizerKey, marker_exists);
    if (FAILED(result)) {
        return result;
    }
    UserFinalizerTransaction existing_transaction{};
    result = query_user_finalizer_transaction_at(
        kUserFinalizerKey, existing_transaction);
    if (FAILED(result)) {
        return result;
    }
    ComPtr<ITfInputProcessorProfileMgr> profiles;
    result = create_profile_manager(profiles);
    if (FAILED(result)) {
        return result;
    }
    bool profile_exists = false;
    result = query_profile_exists(profiles.operator->(), profile_exists);
    if (FAILED(result)) {
        return result;
    }
    bool previous_enabled = false;
    if (profile_exists) {
        result = query_current_user_enabled(previous_enabled);
        if (FAILED(result)) {
            return result;
        }
    }
    result = validate_user_finalizer_state(
        UserFinalizerOperation::remove,
        marker_exists,
        existing_transaction,
        previous_enabled);
    if (FAILED(result)) {
        return result;
    }
    UserFinalizerTransaction transaction{
        UserFinalizerTransactionKind::remove, previous_enabled};
    if (existing_transaction.kind == UserFinalizerTransactionKind::remove) {
        transaction = existing_transaction;
    }
    result = write_user_finalizer_transaction_at(kUserFinalizerKey, transaction);
    if (FAILED(result)) {
        return result;
    }
    if (profile_exists) {
        result = set_current_user_enabled_verified(false);
        if (FAILED(result)) {
            report_user_state_rollback_failure(
                set_current_user_enabled_verified(transaction.previous_enabled));
            return result;
        }
    }
    bool marker_deleted = false;
    result = delete_user_finalizer_marker_at(
        kUserFinalizerKey, false, &marker_deleted);
    if (FAILED(result)) {
        if (marker_deleted) {
            bool marker_restored = false;
            const HRESULT restore_marker = create_user_finalizer_marker_at(
                kUserFinalizerKey, &marker_restored);
            report_user_state_rollback_failure(restore_marker);
            if (FAILED(restore_marker) || !marker_restored) {
                return result;
            }
        }
        if (profile_exists) {
            report_user_state_rollback_failure(
                set_current_user_enabled_verified(transaction.previous_enabled));
        }
        return result;
    }
    return S_OK;
}

HRESULT rollback_install_current_user_fixed() noexcept {
    HRESULT result = require_standard_current_user_process();
    if (FAILED(result)) {
        return result;
    }
    UserFinalizerLock lock;
    result = lock.acquire();
    if (FAILED(result)) {
        return result;
    }
    bool marker_exists = false;
    result = query_user_finalizer_marker_at(kUserFinalizerKey, marker_exists);
    if (FAILED(result)) {
        return result;
    }
    UserFinalizerTransaction transaction{};
    result = query_user_finalizer_transaction_at(kUserFinalizerKey, transaction);
    if (FAILED(result)) {
        return result;
    }
    result = validate_user_finalizer_state(
        UserFinalizerOperation::rollback_install,
        marker_exists,
        transaction,
        false);
    if (FAILED(result)) {
        return result;
    }
    ComPtr<ITfInputProcessorProfileMgr> profiles;
    result = create_profile_manager(profiles);
    if (FAILED(result)) {
        return result;
    }
    bool profile_exists = false;
    result = query_profile_exists(profiles.operator->(), profile_exists);
    if (FAILED(result)) {
        return result;
    }
    if (profile_exists) {
        result = set_current_user_enabled_verified(transaction.previous_enabled);
        if (FAILED(result)) {
            return result;
        }
    }
    if (marker_exists) {
        result = delete_user_finalizer_marker_at(kUserFinalizerKey, false);
        if (FAILED(result)) {
            return result;
        }
    }
    return delete_user_finalizer_transaction_at(
        kUserFinalizerKey, transaction, false);
}

HRESULT rollback_remove_current_user_fixed() noexcept {
    HRESULT result = require_standard_current_user_process();
    if (FAILED(result)) {
        return result;
    }
    UserFinalizerLock lock;
    result = lock.acquire();
    if (FAILED(result)) {
        return result;
    }
    result = verify_machine_installation_for_user_finalizer();
    if (FAILED(result)) {
        return result;
    }
    bool marker_exists = false;
    result = query_user_finalizer_marker_at(kUserFinalizerKey, marker_exists);
    if (FAILED(result)) {
        return result;
    }
    UserFinalizerTransaction transaction{};
    result = query_user_finalizer_transaction_at(kUserFinalizerKey, transaction);
    if (FAILED(result)) {
        return result;
    }
    result = validate_user_finalizer_state(
        UserFinalizerOperation::rollback_remove,
        marker_exists,
        transaction,
        false);
    if (FAILED(result)) {
        return result;
    }
    ComPtr<ITfInputProcessorProfileMgr> profiles;
    result = create_profile_manager(profiles);
    if (FAILED(result)) {
        return result;
    }
    result = set_current_user_enabled_verified(transaction.previous_enabled);
    if (FAILED(result)) {
        return result;
    }
    if (!marker_exists) {
        result = create_user_finalizer_marker_at(kUserFinalizerKey);
        if (FAILED(result)) {
            return result;
        }
    }
    return delete_user_finalizer_transaction_at(
        kUserFinalizerKey, transaction, false);
}

HRESULT run_fixed_current_user_operation(std::wstring_view command) noexcept {
    if (command == L"install-current-user-fixed") {
        return install_or_repair_current_user_fixed(false);
    }
    if (command == L"repair-current-user-fixed") {
        return install_or_repair_current_user_fixed(true);
    }
    if (command == L"remove-current-user-fixed") {
        return remove_current_user_fixed();
    }
    if (command == L"rollback-install-current-user-fixed") {
        return rollback_install_current_user_fixed();
    }
    if (command == L"rollback-remove-current-user-fixed") {
        return rollback_remove_current_user_fixed();
    }
    return E_INVALIDARG;
}

HRESULT self_test_user_finalizer_marker() noexcept {
    bool exists = false;
    HRESULT result = query_user_finalizer_marker_at(kProbeUserFinalizerKey, exists);
    if (FAILED(result) || exists) {
        return FAILED(result) ? result : HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS);
    }
    UserFinalizerTransaction transaction{};
    result = query_user_finalizer_transaction_at(kProbeUserFinalizerKey, transaction);
    if (FAILED(result) || transaction.kind != UserFinalizerTransactionKind::none) {
        return FAILED(result) ? result : HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS);
    }
    result = create_user_finalizer_marker_at(kProbeUserFinalizerKey);
    if (FAILED(result)) {
        return result;
    }
    HRESULT outcome = S_OK;
    do {
        exists = false;
        outcome = query_user_finalizer_marker_at(kProbeUserFinalizerKey, exists);
        if (FAILED(outcome) || !exists) {
            outcome = FAILED(outcome) ? outcome : E_UNEXPECTED;
            break;
        }
        const HRESULT duplicate = create_user_finalizer_marker_at(kProbeUserFinalizerKey);
        if (duplicate != HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS)) {
            outcome = E_UNEXPECTED;
            break;
        }
        for (const UserFinalizerTransaction expected : std::array{
                 UserFinalizerTransaction{UserFinalizerTransactionKind::install, false},
                 UserFinalizerTransaction{UserFinalizerTransactionKind::install, true},
                 UserFinalizerTransaction{UserFinalizerTransactionKind::repair, false},
                 UserFinalizerTransaction{UserFinalizerTransactionKind::repair, true},
                 UserFinalizerTransaction{UserFinalizerTransactionKind::remove, false},
                 UserFinalizerTransaction{UserFinalizerTransactionKind::remove, true}}) {
            outcome = write_user_finalizer_transaction_at(
                kProbeUserFinalizerKey, expected);
            if (FAILED(outcome)) {
                break;
            }
            transaction = {};
            outcome = query_user_finalizer_transaction_at(
                kProbeUserFinalizerKey, transaction);
            if (FAILED(outcome) || transaction.kind != expected.kind
                || transaction.previous_enabled != expected.previous_enabled) {
                outcome = FAILED(outcome) ? outcome : E_UNEXPECTED;
                break;
            }
            outcome = delete_user_finalizer_transaction_at(
                kProbeUserFinalizerKey, expected, false);
            if (FAILED(outcome)) {
                break;
            }
        }
    } while (false);
    transaction = {};
    if (SUCCEEDED(query_user_finalizer_transaction_at(
            kProbeUserFinalizerKey, transaction))
        && transaction.kind != UserFinalizerTransactionKind::none) {
        const HRESULT transaction_cleanup = delete_user_finalizer_transaction_at(
            kProbeUserFinalizerKey, transaction, false);
        if (FAILED(transaction_cleanup)) {
            return transaction_cleanup;
        }
    }
    const HRESULT cleanup = delete_user_finalizer_marker_at(kProbeUserFinalizerKey, false);
    if (FAILED(cleanup)) {
        return cleanup;
    }
    exists = true;
    result = query_user_finalizer_marker_at(kProbeUserFinalizerKey, exists);
    if (FAILED(result) || exists) {
        return FAILED(result) ? result : E_UNEXPECTED;
    }
    return outcome;
}

HRESULT self_test_user_finalizer_policy() noexcept {
    LayoutOrTipProfile sample{};
    sample.profile_type = kLayoutOrTipInputProcessor;
    sample.language = kLanguage;
    sample.clsid = mo::windows_tip::kTextServiceClsid;
    sample.profile = mo::windows_tip::kSimplifiedChineseProfileGuid;
    if (!is_enabled_mo_profile(sample)) {
        return E_UNEXPECTED;
    }
    sample.flags = kLayoutOrTipDisabled;
    if (is_enabled_mo_profile(sample)) {
        return E_UNEXPECTED;
    }
    sample.flags = 1; // Default is still enabled.
    if (!is_enabled_mo_profile(sample)) {
        return E_UNEXPECTED;
    }
    for (int field = 0; field != 4; ++field) {
        LayoutOrTipProfile foreign = sample;
        switch (field) {
        case 0: foreign.profile_type = 2; break;
        case 1: foreign.language = 0; break;
        case 2: foreign.clsid = GUID_NULL; break;
        case 3: foreign.profile = GUID_NULL; break;
        }
        if (is_enabled_mo_profile(foreign)) {
            return E_UNEXPECTED;
        }
    }
    const auto transactions = std::array{
        UserFinalizerTransaction{},
        UserFinalizerTransaction{UserFinalizerTransactionKind::install, false},
        UserFinalizerTransaction{UserFinalizerTransactionKind::install, true},
        UserFinalizerTransaction{UserFinalizerTransactionKind::repair, false},
        UserFinalizerTransaction{UserFinalizerTransactionKind::repair, true},
        UserFinalizerTransaction{UserFinalizerTransactionKind::remove, false},
        UserFinalizerTransaction{UserFinalizerTransactionKind::remove, true}};
    for (const UserFinalizerOperation operation : {
             UserFinalizerOperation::install,
             UserFinalizerOperation::repair,
             UserFinalizerOperation::remove,
             UserFinalizerOperation::rollback_install,
             UserFinalizerOperation::rollback_remove}) {
        for (const bool marker : {false, true}) {
            for (const UserFinalizerTransaction& transaction : transactions) {
                for (const bool enabled : {false, true}) {
                    bool allowed = false;
                    switch (operation) {
                    case UserFinalizerOperation::install:
                        allowed = !marker
                            && transaction.kind != UserFinalizerTransactionKind::repair
                            && (transaction.kind == UserFinalizerTransactionKind::install
                                    ? !transaction.previous_enabled
                                    : !enabled);
                        break;
                    case UserFinalizerOperation::repair:
                    case UserFinalizerOperation::remove:
                        allowed = marker;
                        break;
                    case UserFinalizerOperation::rollback_install:
                        allowed = transaction.kind == UserFinalizerTransactionKind::install
                            && !transaction.previous_enabled;
                        break;
                    case UserFinalizerOperation::rollback_remove:
                        allowed = transaction.kind == UserFinalizerTransactionKind::remove;
                        break;
                    }
                    const HRESULT actual = validate_user_finalizer_state(
                        operation, marker, transaction, enabled);
                    if (SUCCEEDED(actual) != allowed) {
                        return E_UNEXPECTED;
                    }
                }
            }
        }
    }
    struct ProcessCase final {
        bool elevated;
        DWORD session;
        bool app_container;
        HRESULT expected;
    };
    for (const ProcessCase& item : std::array{
             ProcessCase{false, 1, false, S_OK},
             ProcessCase{true, 1, false, HRESULT_FROM_WIN32(ERROR_ACCESS_DENIED)},
             ProcessCase{false, 0, false, HRESULT_FROM_WIN32(ERROR_NOT_LOGGED_ON)},
             ProcessCase{false, 1, true, HRESULT_FROM_WIN32(ERROR_ACCESS_DENIED)}}) {
        if (validate_user_finalizer_process_context(
                item.elevated, item.session, item.app_container)
            != item.expected) {
            return E_UNEXPECTED;
        }
    }
    return S_OK;
}

HRESULT print_status() noexcept {
    for (const auto& item : std::array{
             std::pair{L"com.x64", KEY_WOW64_64KEY},
             std::pair{L"com.x86", KEY_WOW64_32KEY}}) {
        std::wstring path;
        bool exists = false;
        const HRESULT result = query_registry_string(kComKey, item.second, nullptr, path, exists);
        if (FAILED(result)) {
            return result;
        }
        std::wcout << item.first << L'=' << (exists ? path : L"missing") << L'\n';
    }

    ComPtr<ITfInputProcessorProfileMgr> profiles;
    HRESULT result = create_profile_manager(profiles);
    if (FAILED(result)) {
        return result;
    }
    TF_INPUTPROCESSORPROFILE profile{};
    result = profiles->GetProfile(
        TF_PROFILETYPE_INPUTPROCESSOR,
        kLanguage,
        mo::windows_tip::kTextServiceClsid,
        mo::windows_tip::kSimplifiedChineseProfileGuid,
        nullptr,
        &profile);
    if (FAILED(result)) {
        std::wcout << L"profile.registered=false\nprofile.enabled=false\nprofile.active=false\n";
    } else {
        bool enabled = false;
        result = query_current_user_enabled(enabled);
        if (FAILED(result)) {
            return result;
        }
        std::wcout
            << L"profile.registered=true\n"
            << L"profile.enabled=" << (enabled ? L"true" : L"false") << L'\n'
            << L"profile.active=" << ((profile.dwFlags & TF_IPP_FLAG_ACTIVE) != 0 ? L"true" : L"false") << L'\n';
    }
    bool marker_exists = false;
    result = query_user_finalizer_marker_at(kUserFinalizerKey, marker_exists);
    if (result == HRESULT_FROM_WIN32(ERROR_INVALID_DATA)) {
        std::wcout << L"user.finalizer=invalid\n";
    } else if (FAILED(result)) {
        return result;
    } else {
        std::wcout << L"user.finalizer=" << (marker_exists ? L"v1" : L"missing") << L'\n';
    }
    UserFinalizerTransaction transaction{};
    result = query_user_finalizer_transaction_at(kUserFinalizerKey, transaction);
    if (result == HRESULT_FROM_WIN32(ERROR_INVALID_DATA)) {
        std::wcout << L"user.finalizer.transaction=invalid\n";
        return S_OK;
    }
    if (FAILED(result)) {
        return result;
    }
    if (transaction.kind == UserFinalizerTransactionKind::none) {
        std::wcout << L"user.finalizer.transaction=missing\n";
    } else {
        const wchar_t* value = user_finalizer_transaction_value(transaction);
        if (value == nullptr) {
            return E_UNEXPECTED;
        }
        std::wcout << L"user.finalizer.transaction=" << value << L'\n';
    }
    return S_OK;
}

HRESULT self_test_registry(
    const std::wstring& x64_path,
    const std::wstring& x86_path) noexcept {
    if (!is_64_bit_windows()) {
        return HRESULT_FROM_WIN32(ERROR_NOT_SUPPORTED);
    }
    if (!is_absolute_file(x64_path) || !is_absolute_file(x86_path)) {
        return E_INVALIDARG;
    }

    for (const REGSAM view : {KEY_WOW64_64KEY, KEY_WOW64_32KEY}) {
        std::wstring existing;
        bool exists = false;
        HRESULT result = query_registry_string(kProbeComKey, view, nullptr, existing, exists);
        if (FAILED(result)) {
            std::wcerr << L"registry self-test preflight query failed for view 0x"
                       << std::hex << view << L'\n';
            return result;
        }
        if (exists) {
            return HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS);
        }
    }

    HRESULT outcome = S_OK;
    bool x64_created = false;
    bool x86_created = false;
    do {
        outcome = register_com_view(
            kProbeComKey,
            kComParentKey,
            kProbeComClsidKey,
            KEY_WOW64_64KEY,
            x64_path,
            x64_created);
        if (FAILED(outcome)) {
            std::wcerr << L"registry self-test x64 write failed\n";
            break;
        }
        outcome = register_com_view(
            kProbeComKey,
            kComParentKey,
            kProbeComClsidKey,
            KEY_WOW64_32KEY,
            x86_path,
            x86_created);
        if (FAILED(outcome)) {
            std::wcerr << L"registry self-test x86 write failed\n";
            break;
        }
        for (const auto& item : std::array{
                 std::pair{KEY_WOW64_64KEY, x64_path},
                 std::pair{KEY_WOW64_32KEY, x86_path}}) {
            std::wstring actual;
            bool exists = false;
            outcome = query_registry_string(kProbeComKey, item.first, nullptr, actual, exists);
            if (FAILED(outcome) || !exists || _wcsicmp(actual.c_str(), item.second.c_str()) != 0) {
                std::wcerr << L"registry self-test path readback failed for view 0x"
                           << std::hex << item.first << L'\n';
                outcome = FAILED(outcome) ? outcome : HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
                break;
            }
            std::wstring threading_model;
            outcome = query_registry_string(
                kProbeComKey, item.first, L"ThreadingModel", threading_model, exists);
            if (FAILED(outcome) || !exists || threading_model != L"Apartment") {
                std::wcerr << L"registry self-test threading model readback failed for view 0x"
                           << std::hex << item.first << L'\n';
                outcome = FAILED(outcome) ? outcome : HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
                break;
            }
        }
        if (FAILED(outcome)) {
            break;
        }

        bool repeated_created = true;
        outcome = register_com_view(
            kProbeComKey,
            kComParentKey,
            kProbeComClsidKey,
            KEY_WOW64_64KEY,
            x64_path,
            repeated_created);
        if (FAILED(outcome) || repeated_created) {
            std::wcerr << L"registry self-test idempotent registration failed\n";
            outcome = FAILED(outcome) ? outcome : E_UNEXPECTED;
            break;
        }

        bool conflict_created = false;
        const HRESULT conflict_result = register_com_view(
            kProbeComKey,
            kComParentKey,
            kProbeComClsidKey,
            KEY_WOW64_64KEY,
            x86_path,
            conflict_created);
        if (conflict_result != HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS) || conflict_created) {
            std::wcerr << L"registry self-test conflicting registration was not rejected\n";
            outcome = E_UNEXPECTED;
            break;
        }
    } while (false);

    bool removed = false;
    const HRESULT x86_cleanup = delete_com_view(
        kComParentKey, kProbeComClsidKey, KEY_WOW64_32KEY, removed);
    const HRESULT x64_cleanup = delete_com_view(
        kComParentKey, kProbeComClsidKey, KEY_WOW64_64KEY, removed);
    if (FAILED(x86_cleanup)) {
        std::wcerr << L"registry self-test x86 cleanup failed\n";
    }
    if (FAILED(x64_cleanup)) {
        std::wcerr << L"registry self-test x64 cleanup failed\n";
    }
    if (SUCCEEDED(outcome) && FAILED(x86_cleanup)) {
        outcome = x86_cleanup;
    }
    if (SUCCEEDED(outcome) && FAILED(x64_cleanup)) {
        outcome = x64_cleanup;
    }
    if (SUCCEEDED(outcome)) {
        for (const REGSAM view : {KEY_WOW64_64KEY, KEY_WOW64_32KEY}) {
            std::wstring residue;
            bool exists = false;
            const HRESULT query_result = query_registry_string(
                kProbeComKey, view, nullptr, residue, exists);
            if (FAILED(query_result) || exists) {
                std::wcerr << L"registry self-test cleanup left residue in view 0x"
                           << std::hex << view << L'\n';
                outcome = FAILED(query_result)
                    ? query_result
                    : HRESULT_FROM_WIN32(ERROR_DIR_NOT_EMPTY);
                break;
            }
        }
    }
    return outcome;
}

void print_usage() {
    std::wcerr
        << L"Usage:\n"
        << L"  mo_tip_registrar register-com-user <absolute-x64-dll> <absolute-x86-dll>\n"
        << L"  mo_tip_registrar unregister-com-user\n"
        << L"  mo_tip_registrar register-machine-profile <absolute-icon-module-path>\n"
        << L"  mo_tip_registrar unregister-machine-profile\n"
        << L"  mo_tip_registrar install-machine-profile-fixed\n"
        << L"  mo_tip_registrar install-new-machine-profile-fixed\n"
        << L"  mo_tip_registrar rollback-install-machine-profile-fixed\n"
        << L"  mo_tip_registrar commit-install-machine-profile-fixed\n"
        << L"  mo_tip_registrar remove-machine-profile-fixed\n"
        << L"  mo_tip_registrar rollback-remove-machine-profile-fixed\n"
        << L"  mo_tip_registrar commit-remove-machine-profile-fixed\n"
#if defined(MO_DEVELOPMENT_FAULT_INJECTION)
        << L"  mo_tip_registrar development-test-fail-fixed\n"
#endif
        << L"  mo_tip_registrar install-current-user-fixed\n"
        << L"  mo_tip_registrar repair-current-user-fixed\n"
        << L"  mo_tip_registrar remove-current-user-fixed\n"
        << L"  mo_tip_registrar rollback-install-current-user-fixed\n"
        << L"  mo_tip_registrar rollback-remove-current-user-fixed\n"
        << L"  mo_tip_registrar burn-user-finalizer <fixed-current-user-operation>\n"
        << L"  mo_tip_registrar enable-current-user\n"
        << L"  mo_tip_registrar disable-current-user\n"
        << L"  mo_tip_registrar status\n"
        << L"  mo_tip_registrar self-test-registry <absolute-x64-dll> <absolute-x86-dll>\n"
        << L"  mo_tip_registrar self-test-machine-transaction <absolute-new-marker-path>\n"
        << L"  mo_tip_registrar self-test-user-finalizer-marker\n"
        << L"  mo_tip_registrar self-test-user-finalizer-policy\n\n"
        << L"COM development registration is scoped to HKCU and writes both WOW64 views.\n"
        << L"Machine profile/category commands require an elevated process.\n"
        << L"Current-user commands reject elevation, Session 0 and AppContainer tokens.\n"
        << L"TSF mutation commands are separate so an installer can own transaction rollback.\n";
}

}  // namespace

int wmain(int argument_count, wchar_t** arguments) {
    if (argument_count < 2) {
        print_usage();
        return 2;
    }

    const HRESULT initialized = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    if (FAILED(initialized)) {
        std::wcerr << L"CoInitializeEx failed: 0x" << std::hex << initialized << L'\n';
        return 1;
    }

    const std::wstring_view command(arguments[1]);
    HRESULT result = E_INVALIDARG;
    if (command == L"register-com-user" && argument_count == 4) {
        result = register_com_current_user(arguments[2], arguments[3]);
    } else if (command == L"unregister-com-user" && argument_count == 2) {
        result = unregister_com_current_user();
    } else if (command == L"register-machine-profile" && argument_count == 3) {
        result = register_machine_profile(arguments[2]);
    } else if (command == L"unregister-machine-profile" && argument_count == 2) {
        result = unregister_machine_profile();
    } else if ((command == L"install-machine-profile-fixed"
                   || command == L"install-new-machine-profile-fixed"
                   || command == L"rollback-install-machine-profile-fixed"
                   || command == L"commit-install-machine-profile-fixed"
                   || command == L"remove-machine-profile-fixed"
                   || command == L"rollback-remove-machine-profile-fixed"
                   || command == L"commit-remove-machine-profile-fixed")
               && argument_count == 2) {
        result = fixed_machine_profile_operation(command);
#if defined(MO_DEVELOPMENT_FAULT_INJECTION)
    } else if (command == L"development-test-fail-fixed" && argument_count == 2) {
        // Deliberate, side-effect-free failure used only by the unsigned
        // development installer to prove MSI/Burn rollback behavior.
        result = E_FAIL;
#endif
    } else if ((command == L"install-current-user-fixed"
                   || command == L"repair-current-user-fixed"
                   || command == L"remove-current-user-fixed"
                   || command == L"rollback-install-current-user-fixed"
                   || command == L"rollback-remove-current-user-fixed")
               && argument_count == 2) {
        result = run_fixed_current_user_operation(command);
    } else if (command == L"burn-user-finalizer" && argument_count == 3) {
        result = run_fixed_current_user_operation(arguments[2]);
    } else if (command == L"enable-current-user" && argument_count == 2) {
        result = require_standard_current_user_process();
        if (SUCCEEDED(result)) {
            result = set_enabled_for_current_user(true);
        }
    } else if (command == L"disable-current-user" && argument_count == 2) {
        result = require_standard_current_user_process();
        if (SUCCEEDED(result)) {
            result = set_enabled_for_current_user(false);
        }
    } else if (command == L"status" && argument_count == 2) {
        result = print_status();
    } else if (command == L"self-test-registry" && argument_count == 4) {
        result = self_test_registry(arguments[2], arguments[3]);
    } else if (command == L"self-test-machine-transaction" && argument_count == 3) {
        result = self_test_machine_transaction(arguments[2]);
    } else if (command == L"self-test-user-finalizer-marker" && argument_count == 2) {
        result = self_test_user_finalizer_marker();
    } else if (command == L"self-test-user-finalizer-policy" && argument_count == 2) {
        result = self_test_user_finalizer_policy();
    } else {
        print_usage();
    }

    CoUninitialize();
    if (FAILED(result)) {
        std::wcerr << L"Operation failed: 0x" << std::hex << result << L'\n';
        return 1;
    }
    return 0;
}
