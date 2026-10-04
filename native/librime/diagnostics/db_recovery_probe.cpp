// SPDX-License-Identifier: Apache-2.0
// Owned synthetic fixtures only; fixed metadata output, no paths, keys or values.
#include "mo_db_diagnostic.h"
#include <leveldb/db.h>
#include <leveldb/write_batch.h>
#include <atomic>
#include <cstdio>
#include <filesystem>
#include <memory>
#include <stdexcept>
#include <string>

namespace {
constexpr int kSmallCount = 128;
constexpr int kLargeCount = 32768;
void Require(bool success) {
    if (!success) throw std::runtime_error("synthetic recovery assertion");
}
std::string Key(int index) { return "synthetic-key-" + std::to_string(index); }
std::string Value(int index, bool updated, bool large = false) {
    return "synthetic-value-" + std::to_string(index) + (updated ? "-v2" : "-v1") +
        (large ? std::string(256, 'x') : "");
}
bool IsLog(const std::string& path) {
    return std::filesystem::path(path).extension() == ".log";
}
bool IsManifest(const std::string& path) {
    return std::filesystem::path(path).filename().string().rfind("MANIFEST-", 0) == 0;
}
enum class Fault { None, AppendLog, AppendManifest, ReadLog, SyncLog };
class SyncFaultFile final : public leveldb::WritableFile {
public:
    SyncFaultFile(leveldb::WritableFile* file, std::atomic<bool>& armed,
                  std::atomic<int>& hits) : file_(file), armed_(armed), hits_(hits) {}
    leveldb::Status Append(const leveldb::Slice& data) override { return file_->Append(data); }
    leveldb::Status Close() override { return file_->Close(); }
    leveldb::Status Flush() override { return file_->Flush(); }
    leveldb::Status Sync() override {
        if (armed_.load()) {
            ++hits_;
            return leveldb::Status::IOError("synthetic Sync fault");
        }
        return file_->Sync();
    }
private:
    std::unique_ptr<leveldb::WritableFile> file_;
    std::atomic<bool>& armed_;
    std::atomic<int>& hits_;
};
class FaultEnv final : public leveldb::EnvWrapper {
public:
    explicit FaultEnv(leveldb::Env* target, Fault fault = Fault::None)
        : EnvWrapper(target), fault_(fault) {}
    std::atomic<bool> armed{false};
    std::atomic<int> hits{0};
    std::atomic<int> log_appends{0};
    std::atomic<int> new_tables{0};
    leveldb::Status NewAppendableFile(const std::string& path,
                                     leveldb::WritableFile** result) override {
        if (IsLog(path)) ++log_appends;
        if ((fault_ == Fault::AppendLog && IsLog(path)) ||
            (fault_ == Fault::AppendManifest && IsManifest(path))) {
            *result = nullptr;
            ++hits;
            return leveldb::Status::IOError("synthetic append-open fault");
        }
        auto status = target()->NewAppendableFile(path, result);
        Wrap(path, status, result);
        return status;
    }
    leveldb::Status NewWritableFile(const std::string& path,
                                   leveldb::WritableFile** result) override {
        if (std::filesystem::path(path).extension() == ".ldb") ++new_tables;
        auto status = target()->NewWritableFile(path, result);
        Wrap(path, status, result);
        return status;
    }
    leveldb::Status NewSequentialFile(const std::string& path,
                                     leveldb::SequentialFile** result) override {
        if (fault_ == Fault::ReadLog && IsLog(path)) {
            *result = nullptr;
            ++hits;
            return leveldb::Status::IOError("synthetic log-read fault");
        }
        return target()->NewSequentialFile(path, result);
    }
private:
    void Wrap(const std::string& path, const leveldb::Status& status,
              leveldb::WritableFile** result) {
        if (status.ok() && fault_ == Fault::SyncLog && IsLog(path))
            *result = new SyncFaultFile(*result, armed, hits);
    }
    Fault fault_;
};
leveldb::Options Options(leveldb::Env& env, bool reuse, bool create = false,
                         bool paranoid = false) {
    leveldb::Options options;
    options.env = &env;
    options.reuse_logs = reuse;
    options.create_if_missing = create;
    options.paranoid_checks = paranoid;
    return options;
}
std::unique_ptr<leveldb::DB> Open(const std::filesystem::path& path,
                                 const leveldb::Options& options) {
    leveldb::DB* raw = nullptr;
    auto status = leveldb::DB::Open(options, path.string(), &raw);
    std::unique_ptr<leveldb::DB> db(raw);
    Require(status.ok() && db != nullptr);
    return db;
}
void Check(leveldb::DB& db, int count, bool updated, bool large = false) {
    for (int index = 0; index < count; ++index) {
        std::string value;
        auto status = db.Get(leveldb::ReadOptions(), Key(index), &value);
        if (updated && index % 7 == 0) Require(status.IsNotFound());
        else Require(status.ok() && value == Value(index, updated, large));
    }
}
void Write(leveldb::DB& db, int count, bool updated, bool large = false) {
    leveldb::WriteBatch batch;
    for (int index = 0; index < count; ++index) {
        if (updated && index % 7 == 0) batch.Delete(Key(index));
        else batch.Put(Key(index), Value(index, updated, large));
    }
    leveldb::WriteOptions writes;
    writes.sync = true;
    Require(db.Write(writes, &batch).ok());
}
void CheckCanWrite(leveldb::DB& db) {
    leveldb::WriteOptions writes;
    writes.sync = true;
    Require(db.Put(writes, "synthetic-post-recovery", "present").ok());
    std::string value;
    Require(db.Get(leveldb::ReadOptions(), "synthetic-post-recovery", &value).ok()
            && value == "present");
}
bool Flag(const wchar_t* value) {
    const std::wstring flag(value);
    Require(flag == L"0" || flag == L"1");
    return flag == L"1";
}
}  // namespace

int wmain(int argc, wchar_t** argv) {
    try {
        Require(argc == 4 || argc == 5);
        const std::wstring mode(argv[1]);
        const std::filesystem::path root(argv[2]);
        Require(root.is_absolute() &&
            std::filesystem::is_regular_file(root / "mo-db-recovery-fixture"));
        const bool reuse = Flag(argv[3]);
        const auto path = root / "synthetic-db";
        mo_diagnostic::DbEnv traced;
        if (mode == L"--seed" || mode == L"--large") {
            Require(argc == 4 && !std::filesystem::exists(path));
            const bool large = mode == L"--large";
            auto options = Options(traced, reuse, true);
            if (large) options.write_buffer_size = 64 * 1024 * 1024;
            auto db = Open(path, options);
            Write(*db, large ? kLargeCount : kSmallCount, false, large);
            db.reset();
            if (large) {
                FaultEnv recovered(&traced);
                db = Open(path, Options(recovered, reuse)); // Default 4 MiB recovery buffer.
                Check(*db, kLargeCount, false, true);
                Require(recovered.new_tables.load() > 0 && recovered.log_appends.load() == 0);
                CheckCanWrite(*db);
                db.reset();
                db = Open(path, Options(traced, !reuse));
                Check(*db, kLargeCount, false, true);
                std::printf("MO_DB_RECOVERY large_records=32768 recovery_compacted=1 log_reuse_skipped=1 opposite_reopen=1\n");
            } else {
                std::printf("MO_DB_RECOVERY seeded=1\n");
            }
        } else if (mode == L"--crash-writer") {
            Require(argc == 4 && std::filesystem::is_directory(path));
            auto db = Open(path, Options(traced, reuse));
            Check(*db, kSmallCount, false);
            Write(*db, kSmallCount, true);
            Check(*db, kSmallCount, true);
            std::printf("MO_DB_RECOVERY durable_ready=1\n");
            Require(std::fflush(stdout) == 0);
            Sleep(INFINITE); // Parent terminates this owned child; no DB/C++ destructors.
        } else if (mode == L"--verify-crash") {
            Require(argc == 4 && std::filesystem::is_directory(path));
            auto db = Open(path, Options(traced, reuse));
            Check(*db, kSmallCount, true);
            CheckCanWrite(*db);
            db.reset();
            db = Open(path, Options(traced, !reuse));
            Check(*db, kSmallCount, true);
            std::printf("MO_DB_RECOVERY durable_update_delete_recovered=1 continued_write=1 opposite_reopen=1\n");
        } else if (mode == L"--append-log" || mode == L"--append-manifest") {
            Require(argc == 4 && reuse && std::filesystem::is_directory(path));
            FaultEnv env(&traced, mode == L"--append-log" ? Fault::AppendLog : Fault::AppendManifest);
            auto db = Open(path, Options(env, reuse));
            Require(env.hits.load() > 0);
            Check(*db, kSmallCount, false);
            CheckCanWrite(*db);
            db.reset();
            db = Open(path, Options(traced, false));
            Check(*db, kSmallCount, false);
            std::printf("MO_DB_RECOVERY append_fault_injected=1 fallback_preserved_records=1 continued_write=1 legacy_reopen=1\n");
        } else if (mode == L"--read-error") {
            Require(argc == 5 && std::filesystem::is_directory(path));
            const bool paranoid = Flag(argv[4]);
            FaultEnv env(&traced, Fault::ReadLog);
            leveldb::DB* raw = nullptr;
            auto status = leveldb::DB::Open(Options(env, reuse, false, paranoid), path.string(), &raw);
            std::unique_ptr<leveldb::DB> db(raw);
            Require(env.hits.load() > 0);
            if (paranoid) {
                Require(status.IsIOError() && !db);
                db = Open(path, Options(traced, reuse));
                Check(*db, kSmallCount, false);
                std::printf("MO_DB_RECOVERY strict_read_error_rejected=1 later_recovery_preserved_records=1\n");
            } else {
                Require(status.ok() && db);
                int missing = 0;
                for (int index = 0; index < kSmallCount; ++index) {
                    std::string value;
                    auto read = db->Get(leveldb::ReadOptions(), Key(index), &value);
                    if (read.IsNotFound()) ++missing;
                    else Require(read.ok() && value == Value(index, false));
                }
                Require(missing > 0);
                std::printf("MO_DB_RECOVERY negative_evidence=1 read_error_open_ok=1 missing_records=%d\n", missing);
                return 2; // Unsafe behavior observed under current default policy.
            }
        } else if (mode == L"--sync-error") {
            Require(argc == 4 && std::filesystem::is_directory(path));
            FaultEnv env(&traced, Fault::SyncLog);
            auto db = Open(path, Options(env, reuse));
            Check(*db, kSmallCount, false);
            env.armed.store(true);
            leveldb::WriteOptions writes;
            writes.sync = true;
            Require(db->Put(writes, "synthetic-failed-sync", "ambiguous").IsIOError());
            Require(env.hits.load() == 1);
            Require(db->Put(writes, "synthetic-after-failure", "must-fail").IsIOError());
            Require(env.hits.load() == 1); // The DB latched the first Sync error.
            db.reset();
            env.armed.store(false);
            db = Open(path, Options(traced, reuse));
            Check(*db, kSmallCount, false);
            std::string value;
            auto read = db->Get(leveldb::ReadOptions(), "synthetic-failed-sync", &value);
            Require(read.IsNotFound() || (read.ok() && value == "ambiguous"));
            CheckCanWrite(*db);
            std::printf("MO_DB_RECOVERY sync_error_propagated=1 error_latched=1 earlier_sync_recovered=1 failed_write_present=%d\n", read.ok() ? 1 : 0);
        } else {
            Require(false);
        }
        return 0;
    } catch (const std::exception&) {
        std::fprintf(stderr, "MO_DB_RECOVERY assertion_failed=1\n");
        return 1;
    }
}
