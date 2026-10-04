// SPDX-License-Identifier: Apache-2.0
// Fixed synthetic records, marker-guarded existing database; no content output.
#include <windows.h>
#include <leveldb/db.h>
#include <leveldb/write_batch.h>
#include <cstdio>
#include <filesystem>
#include <memory>
#include <stdexcept>
#include <string>
namespace {
void Require(bool ok) { if (!ok) throw std::runtime_error("synthetic fixture assertion"); }
}
int wmain(int argc, wchar_t** argv) {
    try {
        Require(argc == 3);
        const std::wstring mode(argv[1]);
        Require(mode == L"--seed" || mode == L"--verify");
        const std::filesystem::path root(argv[2]);
        Require(root.is_absolute());
        for (auto p = root; !p.empty(); p = p.parent_path()) {
            const auto attrs = GetFileAttributesW(p.c_str());
            Require(attrs != INVALID_FILE_ATTRIBUTES && !(attrs & FILE_ATTRIBUTE_REPARSE_POINT));
            if (p == p.parent_path()) break;
        }
        Require(std::filesystem::is_regular_file(root / "mo-userdb-fixture"));
        const auto db_path = root / "rime_ice.userdb";
        Require(std::filesystem::is_directory(db_path));
        Require(std::filesystem::is_regular_file(db_path / "CURRENT"));
        Require(!(GetFileAttributesW(db_path.c_str()) & FILE_ATTRIBUTE_REPARSE_POINT));
        for (const auto& entry : std::filesystem::directory_iterator(db_path)) {
            const auto attrs = GetFileAttributesW(entry.path().c_str());
            Require(attrs != INVALID_FILE_ATTRIBUTES && !(attrs & FILE_ATTRIBUTE_REPARSE_POINT));
        }
        leveldb::Options options;
        options.paranoid_checks = true;
        options.create_if_missing = false;
        leveldb::DB* raw = nullptr;
        Require(leveldb::DB::Open(options, db_path.string(), &raw).ok());
        std::unique_ptr<leveldb::DB> db(raw);
        if (mode == L"--seed") {
            leveldb::WriteBatch batch;
            for (int i = 0; i < 32; ++i) {
                batch.Put("mo-synthetic-fixture-" + std::to_string(i), "synthetic-value");
            }
            leveldb::WriteOptions write;
            write.sync = true;
            Require(db->Write(write, &batch).ok());
        }
        for (int i = 0; i < 32; ++i) {
            std::string value;
            Require(db->Get(leveldb::ReadOptions(), "mo-synthetic-fixture-" +
                            std::to_string(i), &value).ok());
            Require(value == "synthetic-value");
        }
        std::puts("MO_USERDB_FIXTURE records_verified=32");
        return 0;
    } catch (...) {
        std::fputs("MO_USERDB_FIXTURE assertion_failed=1\n", stderr);
        return 1;
    }
}
