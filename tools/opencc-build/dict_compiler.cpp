// Build-time utility, never linked into the TIP or Broker.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <filesystem>
#include <cstdio>
#include <iostream>
#include <memory>
#include <stdexcept>
#include "TextDict.hpp"
#include "MarisaDict.hpp"
#include "Lexicon.hpp"

namespace {
using File = std::unique_ptr<FILE, decltype(&fclose)>;
File Open(const wchar_t* path, const wchar_t* mode) {
    File file(_wfopen(path, mode), fclose);
    if (!file) { throw std::runtime_error("Could not open dictionary file"); }
    return file;
}
}
int wmain(int argc, wchar_t** argv) {
    if (argc != 3) { std::cerr << "usage: mo_opencc_dict <absolute-input.txt> <new-absolute-output.ocd2>\n"; return 2; }
    try {
        if (!std::filesystem::path(argv[1]).is_absolute() || !std::filesystem::path(argv[2]).is_absolute()) {
            throw std::runtime_error("Dictionary paths must be absolute");
        }
        auto input = Open(argv[1], L"rb");
        auto text = opencc::TextDict::NewFromFile(input.get());
        auto binary = opencc::MarisaDict::NewFromDict(*text);
        // Exclusive creation: never replace input or a pre-existing build artifact.
        auto output = Open(argv[2], L"wbx");
        binary->SerializeToFile(output.get());
        if (ferror(output.get()) || fflush(output.get()) != 0) { throw std::runtime_error("Dictionary write failed"); }
        output.reset();
        auto saved = Open(argv[2], L"rb");
        auto loaded = opencc::MarisaDict::NewFromFile(saved.get());
        const auto lexicon = text->GetLexicon();
        if (loaded->GetLexicon()->Length() != lexicon->Length()) { throw std::runtime_error("Dictionary entry count changed"); }
        // Verify EVERY key and ordered value list after serialization/reload.
        for (const auto& entry : *lexicon) {
            const auto match = loaded->Match(entry->Key().c_str(), entry->KeyLength());
            if (match.IsNull() || match.Get()->Values() != entry->Values()) {
                throw std::runtime_error("Dictionary values changed");
            }
        }
        std::cout << "MO_DICT entries=" << lexicon->Length() << " verified=true\n";
        return 0;
    } catch (const std::exception&) {
        // Do not log dictionary contents or user-provided paths.
        std::cerr << "Dictionary compilation/round-trip verification failed\n";
        return 1;
    }
}
