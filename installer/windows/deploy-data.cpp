#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <rime_api.h>
#include <filesystem>
#include <iostream>
#include <string>

// Build-only tool: explicit DLL, fresh marked workspace, no user/machine registration.
namespace {
std::string Utf8(const wchar_t* value) {
    const int size = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value, -1, nullptr, 0, nullptr, nullptr);
    if (size <= 0) { throw std::runtime_error("invalid Unicode path"); }
    std::string output(static_cast<std::size_t>(size), '\0');
    if (!WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value, -1, output.data(), size, nullptr, nullptr)) {
        throw std::runtime_error("UTF-8 conversion failed");
    }
    output.pop_back();
    return output;
}
}

int wmain(int argc, wchar_t** argv) {
    try {
        if (argc != 5) { throw std::runtime_error("usage: deploy-data <dll> <shared> <fresh-user> <staging>"); }
        for (int i = 1; i < argc; ++i) {
            if (!std::filesystem::path(argv[i]).is_absolute()) { throw std::runtime_error("absolute paths required"); }
        }
        const std::filesystem::path user(argv[3]);
        if (!std::filesystem::is_regular_file(user / "mo-data-build-fixture")
            || std::filesystem::exists(std::filesystem::path(argv[4]))) {
            throw std::runtime_error("marked fresh workspace required");
        }
        HMODULE module = LoadLibraryExW(argv[1], nullptr, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
        if (!module) { throw std::runtime_error("runtime load failed"); }
        const auto getter = reinterpret_cast<RimeApi*(*)()>(GetProcAddress(module, "rime_get_api"));
        const RimeApi* api = getter ? getter() : nullptr;
        if (!GetProcAddress(module, "mo_rime_prepare_resources_v3") || !api || api->data_size < 0
            || !RIME_PROVIDED(api, setup) || !RIME_PROVIDED(api, deployer_initialize)
            || !RIME_PROVIDED(api, deploy) || !RIME_PROVIDED(api, finalize)) {
            FreeLibrary(module); throw std::runtime_error("required runtime ABI missing");
        }
        const auto shared = Utf8(argv[2]);
        const auto user_utf8 = Utf8(argv[3]);
        const auto staging = Utf8(argv[4]);
        RimeTraits traits{};
        RIME_STRUCT_INIT(RimeTraits, traits);
        traits.shared_data_dir = shared.c_str();
        traits.user_data_dir = user_utf8.c_str();
        traits.staging_dir = staging.c_str();
        traits.distribution_name = "Mo Development Resource Build";
        traits.distribution_code_name = "mo";
        traits.distribution_version = "0.1.0";
        traits.app_name = "rime.mo.data-build";
        traits.min_log_level = 3;
        traits.log_dir = "";
        api->setup(&traits);
        api->deployer_initialize(&traits);
        const bool succeeded = api->deploy() != 0;
        api->finalize();
        FreeLibrary(module);
        if (!succeeded) { throw std::runtime_error("workspace deployment failed"); }
        std::cout << "Fresh locked-source resource deployment passed.\n";
        return 0;
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
