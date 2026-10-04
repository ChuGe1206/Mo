// SPDX-License-Identifier: Apache-2.0
#pragma once
// Isolated synthetic-fixture diagnostics; never log paths, keys or file contents.
#include <rime/mo_diagnostic.h>
#include <leveldb/env.h>
#include <memory>

namespace mo_diagnostic {
inline bool DbFlagEnabled(const char* name) {
    char value[2]{};
    return GetEnvironmentVariableA(name, value, sizeof(value)) == 1 && value[0] == '1';
}

class DbWritableFile final : public leveldb::WritableFile {
public:
    explicit DbWritableFile(leveldb::WritableFile* file) : file_(file) {}
    leveldb::Status Append(const leveldb::Slice& data) override {
        Scope trace("db_io", "Append");
        return file_->Append(data);
    }
    leveldb::Status Close() override {
        Scope trace("db_io", "Close");
        return file_->Close();
    }
    leveldb::Status Flush() override {
        Scope trace("db_io", "Flush");
        return file_->Flush();
    }
    leveldb::Status Sync() override {
        Scope trace("db_io", "Sync", 0);
        return file_->Sync();
    }
private:
    std::unique_ptr<leveldb::WritableFile> file_;
};

class DbSequentialFile final : public leveldb::SequentialFile {
public:
    explicit DbSequentialFile(leveldb::SequentialFile* file) : file_(file) {}
    leveldb::Status Read(size_t bytes, leveldb::Slice* result, char* scratch) override {
        Scope trace("db_io", "SequentialRead");
        return file_->Read(bytes, result, scratch);
    }
    leveldb::Status Skip(uint64_t bytes) override {
        Scope trace("db_io", "SequentialSkip");
        return file_->Skip(bytes);
    }
private:
    std::unique_ptr<leveldb::SequentialFile> file_;
};

class DbEnv final : public leveldb::EnvWrapper {
public:
    DbEnv() : EnvWrapper(leveldb::Env::Default()) {}
    leveldb::Status NewWritableFile(const std::string& path, leveldb::WritableFile** result) override {
        Scope trace("db_io", "NewWritableFile");
        auto status = target()->NewWritableFile(path, result);
        if (status.ok()) *result = new DbWritableFile(*result);
        return status;
    }
    leveldb::Status NewAppendableFile(const std::string& path, leveldb::WritableFile** result) override {
        Scope trace("db_io", "NewAppendableFile");
        auto status = target()->NewAppendableFile(path, result);
        if (status.ok()) *result = new DbWritableFile(*result);
        return status;
    }
    leveldb::Status NewSequentialFile(const std::string& path, leveldb::SequentialFile** result) override {
        Scope trace("db_io", "NewSequentialFile");
        auto status = target()->NewSequentialFile(path, result);
        if (status.ok()) *result = new DbSequentialFile(*result);
        return status;
    }
    leveldb::Status NewRandomAccessFile(const std::string& path, leveldb::RandomAccessFile** result) override {
        Scope trace("db_io", "NewRandomAccessFile");
        return target()->NewRandomAccessFile(path, result);
    }
    bool FileExists(const std::string& path) override {
        Scope trace("db_io", "FileExists");
        return target()->FileExists(path);
    }
    leveldb::Status GetChildren(const std::string& path, std::vector<std::string>* result) override {
        Scope trace("db_io", "GetChildren");
        return target()->GetChildren(path, result);
    }
    leveldb::Status CreateDir(const std::string& path) override {
        Scope trace("db_io", "CreateDir");
        return target()->CreateDir(path);
    }
    leveldb::Status RemoveFile(const std::string& path) override {
        Scope trace("db_io", "RemoveFile");
        return target()->RemoveFile(path);
    }
    leveldb::Status GetFileSize(const std::string& path, uint64_t* bytes) override {
        Scope trace("db_io", "GetFileSize");
        return target()->GetFileSize(path, bytes);
    }
    leveldb::Status RenameFile(const std::string& from, const std::string& to) override {
        Scope trace("db_io", "RenameFile");
        return target()->RenameFile(from, to);
    }
    leveldb::Status LockFile(const std::string& path, leveldb::FileLock** lock) override {
        Scope trace("db_io", "LockFile");
        return target()->LockFile(path, lock);
    }
    leveldb::Status UnlockFile(leveldb::FileLock* lock) override {
        Scope trace("db_io", "UnlockFile");
        return target()->UnlockFile(lock);
    }
    leveldb::Status NewLogger(const std::string& path, leveldb::Logger** result) override {
        Scope trace("db_io", "NewLogger");
        return target()->NewLogger(path, result);
    }
};
}  // namespace mo_diagnostic
