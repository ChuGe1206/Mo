#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#define _WIN32_WINNT 0x0A00
#include <windows.h>

#include <msctf.h>
#include <shlobj.h>

#include <array>
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
constexpr wchar_t kComKey[] =
    L"Software\\Classes\\CLSID\\{B4911146-2A27-47AA-9D12-109B6AE10A70}\\InprocServer32";
constexpr wchar_t kComParentKey[] = L"Software\\Classes\\CLSID";
constexpr wchar_t kComClsidKey[] = L"{B4911146-2A27-47AA-9D12-109B6AE10A70}";
constexpr wchar_t kProbeComKey[] =
    L"Software\\Classes\\CLSID\\"
    L"{A51FCF97-6D9E-4C59-8905-C36A443AB7C2}\\InprocServer32";
constexpr wchar_t kProbeComClsidKey[] = L"{A51FCF97-6D9E-4C59-8905-C36A443AB7C2}";
constexpr wchar_t kFixedTipSuffix[] = L"\\Mo\\tip\\x64\\mo-tip.dll";
constexpr wchar_t kInstallMarkerSuffix[] = L"\\Mo\\.machine-profile-install-rollback-v1";
constexpr wchar_t kRemoveMarkerSuffix[] = L"\\Mo\\.machine-profile-remove-rollback-v1";

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

HRESULT require_elevated_process() noexcept {
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
    return elevation.TokenIsElevated != 0
        ? S_OK
        : HRESULT_FROM_WIN32(ERROR_ELEVATION_REQUIRED);
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

HRESULT query_category_exists(ITfCategoryMgr* manager, bool& exists) noexcept {
    exists = false;
    ComPtr<IEnumGUID> enumerator;
    HRESULT result = manager->EnumItemsInCategory(GUID_TFCAT_TIP_KEYBOARD, enumerator.put());
    if (FAILED(result)) {
        return result;
    }
    for (;;) {
        GUID item{};
        ULONG fetched = 0;
        result = enumerator->Next(1, &item, &fetched);
        if (result == S_FALSE || fetched == 0) {
            return S_OK;
        }
        if (FAILED(result)) {
            return result;
        }
        if (IsEqualGUID(item, mo::windows_tip::kTextServiceClsid)) {
            exists = true;
            return S_OK;
        }
    }
}

HRESULT query_machine_profile_state(
    ITfInputProcessorProfileMgr* profiles,
    ITfCategoryMgr* categories,
    MachineProfileState& state) noexcept {
    HRESULT result = query_profile_exists(profiles, state.profile);
    if (FAILED(result)) {
        return result;
    }
    return query_category_exists(categories, state.category);
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
    HRESULT result = query_machine_profile_state(profiles, categories, current);
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
    result = query_machine_profile_state(profiles, categories, final_state);
    if (FAILED(result)) {
        return result;
    }
    return final_state.profile == target.profile && final_state.category == target.category
        ? S_OK
        : E_UNEXPECTED;
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

HRESULT query_registry_string(
    const wchar_t* key_path,
    REGSAM view,
    const wchar_t* value_name,
    std::wstring& value,
    bool& exists) noexcept {
    exists = false;
    value.clear();
    RegistryKey key;
    LSTATUS status = RegOpenKeyExW(
        HKEY_CURRENT_USER,
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
    if (type != REG_SZ || bytes == 0 || bytes % sizeof(wchar_t) != 0) {
        return HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
    }

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
    if (buffer.empty() || buffer.back() != L'\0') {
        buffer.push_back(L'\0');
    }
    value.assign(buffer.data());
    exists = true;
    return S_OK;
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
    result = query_machine_profile_state(profiles.operator->(), categories.operator->(), previous);
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
    result = query_machine_profile_state(profiles.operator->(), categories.operator->(), previous);
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
        return commit_machine_profile_transaction(TransactionKind::remove, remove_marker);
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
    return SUCCEEDED(missing) && !exists ? S_OK : E_UNEXPECTED;
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
        return S_OK;
    }
    std::wcout
        << L"profile.registered=true\n"
        << L"profile.enabled=" << ((profile.dwFlags & TF_IPP_FLAG_ENABLED) != 0 ? L"true" : L"false") << L'\n'
        << L"profile.active=" << ((profile.dwFlags & TF_IPP_FLAG_ACTIVE) != 0 ? L"true" : L"false") << L'\n';
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
        << L"  mo_tip_registrar enable-current-user\n"
        << L"  mo_tip_registrar disable-current-user\n"
        << L"  mo_tip_registrar status\n"
        << L"  mo_tip_registrar self-test-registry <absolute-x64-dll> <absolute-x86-dll>\n"
        << L"  mo_tip_registrar self-test-machine-transaction <absolute-new-marker-path>\n\n"
        << L"COM development registration is scoped to HKCU and writes both WOW64 views.\n"
        << L"Machine profile/category commands require an elevated process.\n"
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
    } else if (command == L"enable-current-user" && argument_count == 2) {
        result = set_enabled_for_current_user(true);
    } else if (command == L"disable-current-user" && argument_count == 2) {
        result = set_enabled_for_current_user(false);
    } else if (command == L"status" && argument_count == 2) {
        result = print_status();
    } else if (command == L"self-test-registry" && argument_count == 4) {
        result = self_test_registry(arguments[2], arguments[3]);
    } else if (command == L"self-test-machine-transaction" && argument_count == 3) {
        result = self_test_machine_transaction(arguments[2]);
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
