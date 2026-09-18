// SPDX-License-Identifier: Apache-2.0
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <filesystem>
#include <stdexcept>
#include <vector>

namespace rime {
std::string MoOpenccResourceDirectory() {
  static const int module_anchor = 0;
  HMODULE module = nullptr;
  // Engine's LoadedLibrary owns this DLL throughout the call; do not FreeLibrary
  // a borrowed UNCHANGED_REFCOUNT handle or infer a path from the host's EXE.
  if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
      GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
      reinterpret_cast<LPCWSTR>(&module_anchor), &module)) {
    throw std::runtime_error("Mo resource module unavailable");
  }
  std::vector<wchar_t> buffer(32768);
  const DWORD length = GetModuleFileNameW(module, buffer.data(), static_cast<DWORD>(buffer.size()));
  if (length == 0 || length >= buffer.size()) throw std::runtime_error("Mo resource module path unavailable");
  const std::filesystem::path image(std::wstring(buffer.data(), length));
  if (!image.is_absolute()) throw std::runtime_error("Mo resource module path rejected");
  return (image.parent_path() / L"opencc").u8string();
}
}
