// SPDX-License-Identifier: Apache-2.0
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <io.h>
#include <filesystem>
#include <memory>
#include <stdexcept>
#include <string>
#include <cstdio>
#include "mo_resource_file.h"

namespace fs = std::filesystem;
namespace {
void Require(bool value) { if (!value) throw std::runtime_error("resource boundary probe failed"); }
void Create(const fs::path& path, const std::string& data) {
  HANDLE file = CreateFileW(path.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, nullptr);
  Require(file != INVALID_HANDLE_VALUE);
  DWORD written = 0;
  const bool ok = WriteFile(file, data.data(), static_cast<DWORD>(data.size()), &written, nullptr) != FALSE;
  CloseHandle(file);
  Require(ok && written == data.size());
}
unsigned cases = 0;
void Reject(const fs::path& root, const std::string& name, std::uint64_t limit = 1024) {
  bool rejected = false;
  try {
    std::unique_ptr<FILE, decltype(&fclose)> file(opencc::mo::OpenResourceFile(root.u8string(), name, ".json", limit), &fclose);
  } catch (const std::exception&) { rejected = true; }
  Require(rejected);
  ++cases;
}
}
int wmain(int argc, wchar_t** argv) {
  try {
    Require(argc == 2);
    const fs::path fixture(argv[1]);
    Require(fixture.is_absolute() && fs::is_regular_file(fixture / L"mo-resource-file-fixture"));
    const auto root = fixture / L"opencc";
    Create(root / L"good.json", "verified\r\n");
    Create(root / L"large.json", std::string(1025, 'x'));
    Create(fixture / L"not-directory", "file");
    {
      std::unique_ptr<FILE, decltype(&fclose)> file(opencc::mo::OpenResourceFile(root.u8string(), "good.json", ".json", 10), &fclose);
      char data[11]{};
      Require(fread(data, 1, 10, file.get()) == 10 && std::string(data, 10) == "verified\r\n");
      const HANDLE handle = reinterpret_cast<HANDLE>(_get_osfhandle(_fileno(file.get())));
      DWORD flags = 0;
      Require(GetHandleInformation(handle, &flags) && !(flags & HANDLE_FLAG_INHERIT));
      HANDLE writer = CreateFileW((root / L"good.json").c_str(), GENERIC_WRITE,
          FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr, OPEN_EXISTING, 0, nullptr);
      Require(writer == INVALID_HANDLE_VALUE && GetLastError() == ERROR_SHARING_VIOLATION);
      HANDLE remover = CreateFileW((root / L"good.json").c_str(), DELETE,
          FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr, OPEN_EXISTING, 0, nullptr);
      Require(remover == INVALID_HANDLE_VALUE && GetLastError() == ERROR_SHARING_VIOLATION);
    }
    ++cases;
    for (const auto& name : {"../good.json", "..\\good.json", "C:\\good.json", "/good.json", "a/b.json",
        "a\\b.json", "good.json:ads", "good.json.", "good.txt", "CON.json", "nul.json", "COM1.json",
        "LPT9.json", ".json", "a b.json", "missing.json"}) Reject(root, name);
    Reject(root, std::string("good.json\0extra", 15));
    Reject(root, std::string(129, 'a') + ".json");
    Reject(root, "large.json");
    Reject(root, "good.json", 9);
    Reject(fs::path(L"relative"), "good.json");
    Reject(fixture / L"not-directory", "good.json");
    Reject(fixture / L"root-link", "good.json");
    Reject(root, "linked.json");
    Require(CreateDirectoryW((root / L"directory.json").c_str(), nullptr) != FALSE);
    Reject(root, "directory.json");
    Require(CreateHardLinkW((root / L"hard.json").c_str(), (root / L"good.json").c_str(), nullptr) != FALSE);
    Reject(root, "hard.json");
    Reject(root, "good.json");
    std::printf("Mo native same-handle resource boundary: %u cases passed.\n", cases);
    return 0;
  } catch (const std::exception&) { std::fputs("Mo native resource boundary failed.\n", stderr); return 1; }
}
