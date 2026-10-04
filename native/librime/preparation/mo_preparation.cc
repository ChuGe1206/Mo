// SPDX-License-Identifier: Apache-2.0
// Mo's versioned extension; never add slots to the upstream RimeApi table.
#include <rime_api.h>
// Keep the strict C export wrapper free of private C++ DLL-interface headers.
namespace rime { bool PrepareResourcesForSession(RimeSessionId id); }

extern "C" RIME_API int mo_rime_prepare_resources_v3(RimeSessionId id) noexcept {
    try {
        return id && rime::PrepareResourcesForSession(id) ? 1 : 0;
    } catch (...) { return 0; }
}
