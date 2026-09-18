// SPDX-License-Identifier: Apache-2.0
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <fcntl.h>
#include <io.h>
#include <filesystem>
#include <stdexcept>
#include <vector>
#include "mo_resource_file.h"

namespace opencc::mo {
namespace {
[[noreturn]] void Reject() { throw std::runtime_error("Mo resource path or file rejected"); }
class Handle {
 public:
  explicit Handle(HANDLE value) : value_(value) { if (value == INVALID_HANDLE_VALUE) Reject(); }
  ~Handle() { if (value_ != INVALID_HANDLE_VALUE) CloseHandle(value_); }
  Handle(const Handle&) = delete;
  Handle& operator=(const Handle&) = delete;
  HANDLE get() const { return value_; }
  void release() { value_ = INVALID_HANDLE_VALUE; }
 private:
  HANDLE value_;
};
std::wstring FinalPath(HANDLE file) {
  std::vector<wchar_t> buffer(32768);
  const DWORD length = GetFinalPathNameByHandleW(file, buffer.data(),
      static_cast<DWORD>(buffer.size()), FILE_NAME_NORMALIZED | VOLUME_NAME_DOS);
  if (length == 0 || length >= buffer.size()) Reject();
  std::wstring result(buffer.data(), length);
  // Local disk resources only, not UNC/network or arbitrary device namespaces.
  if (result.size() < 7 || result.compare(0, 4, L"\\\\?\\") != 0 ||
      result[5] != L':' || result[6] != L'\\') Reject();
  const std::wstring drive = result.substr(4, 3);
  if (GetDriveTypeW(drive.c_str()) != DRIVE_FIXED) Reject();
  return result;
}
bool LeafName(const std::string& name, const std::string& extension) {
  if (name.size() > 128 || name.size() <= extension.size() ||
      name.compare(name.size() - extension.size(), extension.size(), extension) != 0) return false;
  std::string stem = name.substr(0, name.size() - extension.size());
  for (char& c : stem) {
    if (c >= 'a' && c <= 'z') c = static_cast<char>(c - 'a' + 'A');
    else if (!((c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_' || c == '-')) return false;
  }
  if (stem == "CON" || stem == "PRN" || stem == "AUX" || stem == "NUL") return false;
  if (stem.size() == 4 && (stem.compare(0, 3, "COM") == 0 || stem.compare(0, 3, "LPT") == 0)
      && stem[3] >= '1' && stem[3] <= '9') return false;
  return true;
}
}
FILE* OpenResourceFile(const std::string& directory, const std::string& name,
                       const std::string& extension, std::uint64_t max_bytes) {
  if (!LeafName(name, extension)) Reject();
  const auto root = std::filesystem::u8path(directory);
  if (!root.is_absolute()) Reject();
  Handle root_handle(CreateFileW(root.c_str(), FILE_READ_ATTRIBUTES,
      FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING,
      FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT, nullptr));
  BY_HANDLE_FILE_INFORMATION root_info{};
  if (!GetFileInformationByHandle(root_handle.get(), &root_info) ||
      !(root_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) ||
      (root_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT)) Reject();
  std::wstring expected = FinalPath(root_handle.get());
  if (expected.back() != L'\\') expected += L'\\';
  expected += std::filesystem::u8path(name).native();
  const auto resource = root / std::filesystem::u8path(name);
  Handle file(CreateFileW(resource.c_str(), GENERIC_READ, FILE_SHARE_READ,
      nullptr, OPEN_EXISTING, FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_SEQUENTIAL_SCAN, nullptr));
  BY_HANDLE_FILE_INFORMATION info{};
  LARGE_INTEGER size{};
  if (GetFileType(file.get()) != FILE_TYPE_DISK ||
      !GetFileInformationByHandle(file.get(), &info) ||
      (info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)) ||
      info.nNumberOfLinks != 1 ||
      !GetFileSizeEx(file.get(), &size) || size.QuadPart < 0 ||
      static_cast<std::uint64_t>(size.QuadPart) > max_bytes) Reject();
  const auto actual = FinalPath(file.get());
  if (CompareStringOrdinal(expected.c_str(), static_cast<int>(expected.size()),
      actual.c_str(), static_cast<int>(actual.size()), TRUE) != CSTR_EQUAL) Reject();
  const int descriptor = _open_osfhandle(reinterpret_cast<intptr_t>(file.get()), _O_RDONLY | _O_BINARY | _O_NOINHERIT);
  if (descriptor == -1) Reject();
  file.release(); // descriptor now owns the exact verified OS handle.
  FILE* stream = _fdopen(descriptor, "rb");
  if (!stream) { _close(descriptor); Reject(); }
  return stream;
}
}
