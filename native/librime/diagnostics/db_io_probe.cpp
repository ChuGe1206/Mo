// SPDX-License-Identifier: Apache-2.0
// Synthetic LevelDB forwarding/reopen checks; no keys, values, or paths in logs.
#include "mo_db_diagnostic.h"
#include <leveldb/db.h>
#include <filesystem>
#include <memory>
#include <stdexcept>
#include <string>

namespace {
void Require(bool success, const char* phase) {
    if (!success) throw std::runtime_error(phase);
}

std::unique_ptr<leveldb::DB> Open(mo_diagnostic::DbEnv& env,
                                 const std::filesystem::path& path,
                                 bool create, bool reuse) {
    leveldb::Options options;
    options.env = &env;
    options.create_if_missing = create;
    options.reuse_logs = reuse;
    leveldb::DB* raw = nullptr;
    auto status = leveldb::DB::Open(options, path.string(), &raw);
    std::unique_ptr<leveldb::DB> db(raw);
    Require(status.ok() && db != nullptr, "open failed");
    return db;
}

void ReadBack(leveldb::DB& db, const char* key, const char* expected) {
    std::string value;
    Require(db.Get(leveldb::ReadOptions(), key, &value).ok() && value == expected,
            "synthetic readback mismatch");
}
}  // namespace

int wmain(int argc, wchar_t** argv) {
    try {
        Require(argc == 2, "one absolute synthetic fixture path required");
        const std::filesystem::path root(argv[1]);
        Require(root.is_absolute() &&
                    std::filesystem::is_regular_file(root / "mo-db-io-fixture"),
                "synthetic fixture marker required");
        const auto path = root / "synthetic-db";
        Require(!std::filesystem::exists(path), "fresh synthetic DB required");
        mo_diagnostic::DbEnv env;
        leveldb::WriteOptions writes;
        writes.sync = true;
        {
            auto db = Open(env, path, true, false);
            Require(db->Put(writes, "synthetic-key-a", "synthetic-value-a").ok(),
                    "synchronous write failed");
            ReadBack(*db, "synthetic-key-a", "synthetic-value-a");
            leveldb::Options options;
            options.env = &env;
            options.create_if_missing = false;
            options.reuse_logs = true;
            leveldb::DB* conflicting = nullptr;
            auto locked = leveldb::DB::Open(options, path.string(), &conflicting);
            std::unique_ptr<leveldb::DB> unexpected(conflicting);
            Require(!locked.ok() && !unexpected, "concurrent DB lock was accepted");
        }
        {
            auto db = Open(env, path, false, true);
            ReadBack(*db, "synthetic-key-a", "synthetic-value-a");
            Require(db->Put(writes, "synthetic-key-b", "synthetic-value-b").ok(),
                    "reuse synchronous write failed");
        }
        {
            auto db = Open(env, path, false, true);
            ReadBack(*db, "synthetic-key-a", "synthetic-value-a");
            ReadBack(*db, "synthetic-key-b", "synthetic-value-b");
        }
        {
            auto db = Open(env, path, false, false);
            ReadBack(*db, "synthetic-key-a", "synthetic-value-a");
            ReadBack(*db, "synthetic-key-b", "synthetic-value-b");
        }
        leveldb::Options missing_options;
        missing_options.env = &env;
        missing_options.create_if_missing = false;
        leveldb::DB* missing = nullptr;
        auto absent = leveldb::DB::Open(missing_options,
                                       (root / "missing-db").string(), &missing);
        std::unique_ptr<leveldb::DB> unexpected_missing(missing);
        Require(!absent.ok() && !unexpected_missing, "missing DB was accepted");
        leveldb::WritableFile* file = nullptr;
        auto failed = env.NewWritableFile((root / "no-parent" / "file").string(), &file);
        std::unique_ptr<leveldb::WritableFile> unexpected_file(file);
        Require(!failed.ok() && !unexpected_file, "failed file creation was altered");
        std::printf("MO_DB_IO_PROBE legacy_reopen=1 reuse_reopen=1 sync_write=1 lock_rejected=1 missing_db_rejected=1 failed_create_preserved=1\n");
        return 0;
    } catch (const std::exception&) {
        std::fprintf(stderr, "MO_DB_IO_PROBE assertion_failed=1\n");
        return 1;
    }
}
