// SPDX-License-Identifier: Apache-2.0
#pragma once
#include <cstdio>
#include <cstdint>
#include <string>

namespace opencc::mo {
// Fixed-directory, ASCII leaf name only; the verified handle is consumed by
// OpenCC directly, never reopened by name. Caller must fclose on every path.
FILE* OpenResourceFile(const std::string& directory, const std::string& name,
                       const std::string& extension, std::uint64_t max_bytes);
}
