#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#define _WIN32_WINNT 0x0A00
#include <windows.h>

#include <msctf.h>

#include <iostream>
#include <iterator>
#include <string>
#include <string_view>

#include "mo_tip_ids.h"

namespace {

constexpr LANGID kLanguage = MAKELANGID(LANG_CHINESE, SUBLANG_CHINESE_SIMPLIFIED);
constexpr DWORD kIlotUninstall = 0x00000001;

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

HRESULT register_profile(const std::wstring& dll_path) noexcept {
    if (GetFileAttributesW(dll_path.c_str()) == INVALID_FILE_ATTRIBUTES) {
        return HRESULT_FROM_WIN32(GetLastError());
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
        dll_path.c_str(),
        static_cast<ULONG>(dll_path.size()),
        0,
        nullptr,
        0,
        TRUE,
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

HRESULT unregister_profile() noexcept {
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
    const BOOL succeeded = function(layout.c_str(), enabled ? 0 : kIlotUninstall);
    const DWORD last_error = GetLastError();
    FreeLibrary(input);
    if (succeeded != FALSE) {
        return S_OK;
    }
    return last_error == ERROR_SUCCESS ? E_FAIL : HRESULT_FROM_WIN32(last_error);
}

void print_usage() {
    std::wcerr
        << L"Usage:\n"
        << L"  mo_tip_registrar register-profile <absolute-dll-path>\n"
        << L"  mo_tip_registrar unregister-profile\n"
        << L"  mo_tip_registrar enable-current-user\n"
        << L"  mo_tip_registrar disable-current-user\n\n"
        << L"This helper mutates TSF state. The build probe never invokes it.\n";
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
    if (command == L"register-profile" && argument_count == 3) {
        result = register_profile(arguments[2]);
    } else if (command == L"unregister-profile" && argument_count == 2) {
        result = unregister_profile();
    } else if (command == L"enable-current-user" && argument_count == 2) {
        result = set_enabled_for_current_user(true);
    } else if (command == L"disable-current-user" && argument_count == 2) {
        result = set_enabled_for_current_user(false);
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

