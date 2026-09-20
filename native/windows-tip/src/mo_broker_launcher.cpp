#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <shlobj.h>

#include <array>
#include <filesystem>
#include <utility>

#include "mo_broker_launcher.h"

namespace {

bool ComponentEquals(
    const std::filesystem::path& component,
    const wchar_t* expected) noexcept {
    return _wcsicmp(component.c_str(), expected) == 0;
}

bool PathEquals(
    const std::filesystem::path& left,
    const std::filesystem::path& right) noexcept {
    try {
        return _wcsicmp(
                   left.lexically_normal().c_str(),
                   right.lexically_normal().c_str())
            == 0;
    } catch (...) {
        return false;
    }
}

bool IsKnownArchitecture(const std::filesystem::path& component) noexcept {
    return ComponentEquals(component, L"x64")
        || ComponentEquals(component, L"x86")
        || ComponentEquals(component, L"Win32");
}

std::filesystem::path ProgramFilesX64() noexcept {
    PWSTR value = nullptr;
    const HRESULT result = SHGetKnownFolderPath(
        FOLDERID_ProgramFilesX64,
        KF_FLAG_DEFAULT,
        nullptr,
        &value);
    if (SUCCEEDED(result) && value != nullptr) {
        try {
            std::filesystem::path path(value);
            CoTaskMemFree(value);
            return path;
        } catch (...) {
            CoTaskMemFree(value);
            return {};
        }
    }
    if (value != nullptr) {
        CoTaskMemFree(value);
    }

    // FOLDERID_ProgramFilesX64 returns ERROR_FILE_NOT_FOUND in a 32-bit
    // process on supported 64-bit Windows versions. Read the same machine
    // location from the 64-bit registry view; never consult ProgramW6432 or
    // another caller-controlled environment variable.
    HKEY key = nullptr;
    if (RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            L"SOFTWARE\\Microsoft\\Windows\\CurrentVersion",
            0,
            KEY_QUERY_VALUE | KEY_WOW64_64KEY,
            &key)
        != ERROR_SUCCESS) {
        return {};
    }
    std::array<wchar_t, 32768> buffer{};
    DWORD bytes = static_cast<DWORD>(buffer.size() * sizeof(wchar_t));
    const LSTATUS query = RegGetValueW(
        key,
        nullptr,
        L"ProgramFilesDir",
        RRF_RT_REG_SZ | RRF_ZEROONFAILURE,
        nullptr,
        buffer.data(),
        &bytes);
    RegCloseKey(key);
    if (query != ERROR_SUCCESS || bytes < sizeof(wchar_t)) {
        return {};
    }
    try {
        return std::filesystem::path(buffer.data());
    } catch (...) {
        return {};
    }
}

}  // namespace

namespace mo::windows_tip {

BrokerLocation ResolveBrokerLocation(HMODULE tip_module) noexcept {
    if (tip_module == nullptr) {
        return {};
    }
    std::array<wchar_t, 32768> module_path{};
    const DWORD length = GetModuleFileNameW(
        tip_module,
        module_path.data(),
        static_cast<DWORD>(module_path.size()));
    if (length == 0 || length >= module_path.size()) {
        return {};
    }
    return ResolveBrokerLocationFromPath(std::wstring(module_path.data(), length));
}

BrokerLocation ResolveBrokerLocationFromPath(const std::wstring& tip_path) noexcept {
    try {
        const std::filesystem::path module(tip_path);
        if (!module.is_absolute()) {
            return {};
        }
        const std::filesystem::path directory = module.parent_path();

        // Installed/relocated payload layout. Relocated development fixtures
        // may connect to an explicitly started Broker, but only the exact
        // Known Folder product path receives process-start authority.
        const std::filesystem::path tip_directory = directory.parent_path();
        if (IsKnownArchitecture(directory.filename())
            && ComponentEquals(tip_directory.filename(), L"tip")) {
            const std::filesystem::path root = tip_directory.parent_path();
            BrokerLocation location{
                (root / L"bin" / L"mo-broker.exe").wstring(), false};
            const std::filesystem::path program_files = ProgramFilesX64();
            const std::filesystem::path expected_root = program_files / L"Mo";
            location.auto_start = !program_files.empty()
                && ComponentEquals(module.filename(), L"mo-tip.dll")
                && PathEquals(root, expected_root);
            return location;
        }

        // Repository-only layout used by native probes.
        const std::filesystem::path msbuild_directory = directory.parent_path().parent_path();
        const std::filesystem::path out_directory = msbuild_directory.parent_path();
        const std::filesystem::path windows_tip_directory = out_directory.parent_path();
        const std::filesystem::path native_directory = windows_tip_directory.parent_path();
        if (IsKnownArchitecture(directory.parent_path().filename())
            && ComponentEquals(directory.filename(), L"Release")
            && ComponentEquals(msbuild_directory.filename(), L"msbuild")
            && ComponentEquals(out_directory.filename(), L"out")
            && ComponentEquals(windows_tip_directory.filename(), L"windows-tip")
            && ComponentEquals(native_directory.filename(), L"native")) {
            return {
                (native_directory.parent_path() / L"target" / L"debug" / L"mo-broker.exe")
                    .wstring(),
                false};
        }
    } catch (...) {
    }
    return {};
}

bool StartBrokerProcess(const std::wstring& broker_path, DWORD* process_id) noexcept {
    if (process_id != nullptr) {
        *process_id = 0;
    }
    try {
        const std::filesystem::path image(broker_path);
        if (!image.is_absolute() || !ComponentEquals(image.filename(), L"mo-broker.exe")) {
            SetLastError(ERROR_INVALID_NAME);
            return false;
        }
        const DWORD attributes = GetFileAttributesW(image.c_str());
        if (attributes == INVALID_FILE_ATTRIBUTES) {
            return false;
        }
        if ((attributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)) != 0) {
            SetLastError(ERROR_ACCESS_DENIED);
            return false;
        }

        STARTUPINFOW startup{};
        startup.cb = sizeof(startup);
        PROCESS_INFORMATION process{};
        if (!CreateProcessW(
                image.c_str(),
                nullptr,
                nullptr,
                nullptr,
                FALSE,
                CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
                nullptr,
                nullptr,
                &startup,
                &process)) {
            return false;
        }
        if (process_id != nullptr) {
            *process_id = process.dwProcessId;
        }
        CloseHandle(process.hThread);
        CloseHandle(process.hProcess);
        SetLastError(ERROR_SUCCESS);
        return true;
    } catch (...) {
        SetLastError(ERROR_NOT_ENOUGH_MEMORY);
        return false;
    }
}

}  // namespace mo::windows_tip
