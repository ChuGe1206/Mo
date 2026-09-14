#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#define _WIN32_WINNT 0x0A00
#include <windows.h>

#include <msctf.h>

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

bool is_absolute_file(const std::wstring& path) noexcept {
    const bool drive_path = path.size() >= 3
        && ((path[0] >= L'A' && path[0] <= L'Z') || (path[0] >= L'a' && path[0] <= L'z'))
        && path[1] == L':'
        && (path[2] == L'\\' || path[2] == L'/');
    const bool unc_path = path.size() >= 3
        && path[0] == L'\\'
        && path[1] == L'\\'
        && path[2] != L'\\';
    if (!drive_path && !unc_path) {
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

HRESULT register_machine_profile(const std::wstring& icon_path) noexcept {
    const HRESULT elevation_result = require_elevated_process();
    if (FAILED(elevation_result)) {
        return elevation_result;
    }
    if (!is_absolute_file(icon_path)) {
        return E_INVALIDARG;
    }

    ComPtr<ITfInputProcessorProfileMgr> profiles;
    HRESULT result = create_profile_manager(profiles);
    if (FAILED(result)) {
        return result;
    }
    constexpr ULONG description_length =
        static_cast<ULONG>((sizeof(mo::windows_tip::kProfileDescription) / sizeof(wchar_t)) - 1);
    result = profiles->RegisterProfile(
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
    if (FAILED(result)) {
        return result;
    }

    ComPtr<ITfCategoryMgr> categories;
    result = create_category_manager(categories);
    if (SUCCEEDED(result)) {
        result = categories->RegisterCategory(
            mo::windows_tip::kTextServiceClsid,
            GUID_TFCAT_TIP_KEYBOARD,
            mo::windows_tip::kTextServiceClsid);
    }
    if (FAILED(result)) {
        profiles->UnregisterProfile(
            mo::windows_tip::kTextServiceClsid,
            kLanguage,
            mo::windows_tip::kSimplifiedChineseProfileGuid,
            0);
    }
    return result;
}

HRESULT unregister_machine_profile() noexcept {
    const HRESULT elevation_result = require_elevated_process();
    if (FAILED(elevation_result)) {
        return elevation_result;
    }
    ComPtr<ITfCategoryMgr> categories;
    HRESULT category_result = create_category_manager(categories);
    if (SUCCEEDED(category_result)) {
        category_result = categories->UnregisterCategory(
            mo::windows_tip::kTextServiceClsid,
            GUID_TFCAT_TIP_KEYBOARD,
            mo::windows_tip::kTextServiceClsid);
    }

    ComPtr<ITfInputProcessorProfileMgr> profiles;
    HRESULT profile_result = create_profile_manager(profiles);
    if (SUCCEEDED(profile_result)) {
        profile_result = profiles->UnregisterProfile(
            mo::windows_tip::kTextServiceClsid,
            kLanguage,
            mo::windows_tip::kSimplifiedChineseProfileGuid,
            0);
    }
    return FAILED(profile_result) ? profile_result : category_result;
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
        << L"  mo_tip_registrar enable-current-user\n"
        << L"  mo_tip_registrar disable-current-user\n"
        << L"  mo_tip_registrar status\n"
        << L"  mo_tip_registrar self-test-registry <absolute-x64-dll> <absolute-x86-dll>\n\n"
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
    } else if (command == L"enable-current-user" && argument_count == 2) {
        result = set_enabled_for_current_user(true);
    } else if (command == L"disable-current-user" && argument_count == 2) {
        result = set_enabled_for_current_user(false);
    } else if (command == L"status" && argument_count == 2) {
        result = print_status();
    } else if (command == L"self-test-registry" && argument_count == 4) {
        result = self_test_registry(arguments[2], arguments[3]);
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
