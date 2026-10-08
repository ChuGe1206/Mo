#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#define _WIN32_WINNT 0x0A00
#include <windows.h>

#include <msctf.h>
#include <olectl.h>
#include <textstor.h>
#include <shlobj.h>

#include <iostream>
#include <array>
#include <filesystem>
#include <limits>
#include <new>
#include <string>
#include <wrl/client.h>

#include "mo_tip_ids.h"
#include "mo_broker_launcher.h"
#include "mo_latency_diagnostics.h"
#include "mo_deadline.h"
#include "mo_candidate_palette.h"
#include "mo_settings_change_window.h"

namespace {

using DllGetClassObjectFunction = HRESULT(__stdcall*)(REFCLSID, REFIID, void**);
using DllCanUnloadNowFunction = HRESULT(__stdcall*)();
using Microsoft::WRL::ComPtr;

int fail(const wchar_t* operation, HRESULT result) {
    std::wcerr << operation << L" failed: 0x" << std::hex << result << L'\n';
    return 1;
}

int expect_result(const wchar_t* operation, HRESULT actual, HRESULT expected) {
    if (actual == expected) {
        return 0;
    }
    std::wcerr << operation << L" returned 0x" << std::hex << actual
               << L", expected 0x" << expected << L'\n';
    return 1;
}

bool EqualPalette(const mo::windows_tip::CandidatePalette& left,
    const mo::windows_tip::CandidatePalette& right) noexcept {
    return left.background == right.background && left.preedit == right.preedit
        && left.text == right.text && left.pressed == right.pressed
        && left.pressed_text == right.pressed_text && left.footer == right.footer
        && left.footer_text == right.footer_text;
}

bool ProbeCandidatePalette() {
    using namespace mo::windows_tip;
    const CandidatePalette system{RGB(1, 2, 3), RGB(4, 5, 6), RGB(7, 8, 9),
        RGB(10, 11, 12), RGB(13, 14, 15), RGB(16, 17, 18), RGB(19, 20, 21)};
    const CandidatePalette accessible{system.background, system.text, system.text,
        system.pressed, system.pressed_text, system.background, system.text};
    for (const auto theme : {CandidateTheme::System, CandidateTheme::Light, CandidateTheme::Dark}) {
        if (!EqualPalette(ResolveCandidatePalette(theme, system, true), accessible)) { return false; }
    }
    const CandidatePalette light{RGB(250, 250, 248), RGB(80, 80, 80), RGB(25, 25, 25),
        RGB(220, 232, 245), RGB(25, 25, 25), RGB(238, 238, 235), RGB(70, 70, 70)};
    const CandidatePalette dark{RGB(32, 32, 34), RGB(190, 190, 194), RGB(245, 245, 247),
        RGB(54, 72, 92), RGB(245, 245, 247), RGB(43, 43, 46), RGB(214, 214, 218)};
    if (!EqualPalette(ResolveCandidatePalette(CandidateTheme::System, system, false), system)
        || !EqualPalette(ResolveCandidatePalette(CandidateTheme::Light, system, false), light)
        || !EqualPalette(ResolveCandidatePalette(CandidateTheme::Dark, system, false), dark)) { return false; }
    if (!UseHighContrastPalette(false, 0)
        || !UseHighContrastPalette(true, HCF_HIGHCONTRASTON)
        || UseHighContrastPalette(true, 0)
        || UseHighContrastPalette(true, HCF_AVAILABLE | HCF_HOTKEYACTIVE)) { return false; }
    return IsCandidateAppearanceMessage(WM_SYSCOLORCHANGE)
        && IsCandidateAppearanceMessage(WM_SETTINGCHANGE)
        && IsCandidateAppearanceMessage(WM_THEMECHANGED)
        && !IsCandidateAppearanceMessage(WM_PAINT)
        && !IsCandidateAppearanceMessage(WM_LBUTTONUP);
}

bool ProbeDeadlineArithmetic() {
    using namespace mo::windows_tip;
    const Deadline now{};
    const auto deadline = now + std::chrono::milliseconds(50);
    return RemainingMillisecondsAt(deadline, now) == 50
        && RemainingMillisecondsAt(deadline, now + std::chrono::nanoseconds(1)) == 50
        && RemainingMillisecondsAt(deadline, deadline - std::chrono::nanoseconds(1)) == 1
        && RemainingMillisecondsAt(deadline, deadline) == 0
        && RemainingMillisecondsAt(deadline, deadline + std::chrono::nanoseconds(1)) == 0
        && RemainingMillisecondsAt(now + std::chrono::milliseconds(MAXDWORD), now) == MAXDWORD - 1;
}

void CountSettingsNotification(void* context) noexcept {
    auto* count = static_cast<unsigned int*>(context);
    ++*count;
}

bool ProbeSettingsChangeWindow() {
    unsigned int first_count = 0;
    unsigned int second_count = 0;
    mo::windows_tip::SettingsChangeWindow first;
    mo::windows_tip::SettingsChangeWindow second;
    if (!first.Start(GetModuleHandleW(nullptr), CountSettingsNotification, &first_count)
        || !second.Start(GetModuleHandleW(nullptr), CountSettingsNotification, &second_count)
        || !first.active() || !second.active()
        || !mo::windows_tip::BroadcastSettingsChanged()) {
        first.Stop();
        second.Stop();
        return false;
    }
    const ULONGLONG deadline = GetTickCount64() + 1000;
    while ((first_count == 0 || second_count == 0) && GetTickCount64() < deadline) {
        MsgWaitForMultipleObjectsEx(0, nullptr, 25, QS_POSTMESSAGE, MWMO_INPUTAVAILABLE);
        MSG message{};
        while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    first.Stop();
    if (first_count != 1 || second_count != 1 || first.active() || !second.active()
        || !mo::windows_tip::BroadcastSettingsChanged()) {
        second.Stop();
        return false;
    }
    const ULONGLONG second_deadline = GetTickCount64() + 1000;
    while (second_count == 1 && GetTickCount64() < second_deadline) {
        MsgWaitForMultipleObjectsEx(0, nullptr, 25, QS_POSTMESSAGE, MWMO_INPUTAVAILABLE);
        MSG message{};
        while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    if (first_count != 1 || second_count != 2 || !second.ScheduleRefresh()) {
        second.Stop();
        return false;
    }
    const ULONGLONG deferred_deadline = GetTickCount64() + 1000;
    while (second_count == 2 && GetTickCount64() < deferred_deadline) {
        MsgWaitForMultipleObjectsEx(0, nullptr, 25, QS_POSTMESSAGE, MWMO_INPUTAVAILABLE);
        MSG message{};
        while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    second.Stop();
    return first_count == 1 && second_count == 3
        && !second.active() && !second.ScheduleRefresh();
}

bool ProbeBrokerLauncher() {
    using mo::windows_tip::ResolveBrokerLocationFromPath;

    PWSTR program_files_value = nullptr;
    const HRESULT program_files_result = SHGetKnownFolderPath(
            FOLDERID_ProgramFilesX64,
            KF_FLAG_DEFAULT,
            nullptr,
            &program_files_value);
    std::filesystem::path program_files;
    if (SUCCEEDED(program_files_result) && program_files_value != nullptr) {
        program_files = program_files_value;
        CoTaskMemFree(program_files_value);
    } else {
        if (program_files_value != nullptr) { CoTaskMemFree(program_files_value); }
        HKEY key = nullptr;
        std::array<wchar_t, 32768> buffer{};
        DWORD bytes = static_cast<DWORD>(buffer.size() * sizeof(wchar_t));
        const LSTATUS opened = RegOpenKeyExW(HKEY_LOCAL_MACHINE,
            L"SOFTWARE\\Microsoft\\Windows\\CurrentVersion", 0,
            KEY_QUERY_VALUE | KEY_WOW64_64KEY, &key);
        const LSTATUS queried = opened == ERROR_SUCCESS
            ? RegGetValueW(key, nullptr, L"ProgramFilesDir",
                RRF_RT_REG_SZ | RRF_ZEROONFAILURE, nullptr, buffer.data(), &bytes)
            : opened;
        if (key != nullptr) { RegCloseKey(key); }
        if (queried != ERROR_SUCCESS || bytes < sizeof(wchar_t)) {
            std::wcerr << L"Program Files x64 lookup failed: shell=0x" << std::hex
                << program_files_result << L" registry=" << queried << L'\n';
            return false;
        }
        program_files = buffer.data();
    }
    const wchar_t* architecture = sizeof(void*) == 8 ? L"x64" : L"x86";
    const auto installed = ResolveBrokerLocationFromPath(
        (program_files / L"Mo" / L"tip" / architecture / L"mo-tip.dll").wstring());
    const auto relocated = ResolveBrokerLocationFromPath(
        (std::filesystem::path(LR"(C:\fixture\Mo)") / L"tip" / architecture / L"mo-tip.dll")
            .wstring());
    const auto repository = ResolveBrokerLocationFromPath(
        (std::filesystem::path(LR"(C:\repo\native\windows-tip\out\msbuild)")
            / architecture / L"Release" / L"mo_tip.dll").wstring());
    if (!installed.auto_start
        || installed.path != (program_files / L"Mo" / L"bin" / L"mo-broker.exe").wstring()
        || relocated.auto_start || relocated.path != LR"(C:\fixture\Mo\bin\mo-broker.exe)"
        || repository.auto_start
        || repository.path != LR"(C:\repo\target\debug\mo-broker.exe)"
        || !ResolveBrokerLocationFromPath(LR"(C:\unknown\mo-tip.dll)").path.empty()) {
        std::wcerr << L"Broker location policy mismatch: installed=" << installed.path
            << L" auto=" << installed.auto_start << L" relocated=" << relocated.path
            << L" repository=" << repository.path << L'\n';
        return false;
    }

    std::array<wchar_t, 32768> current_image{};
    std::array<wchar_t, 32768> temporary_root{};
    const DWORD image_length = GetModuleFileNameW(
        nullptr, current_image.data(), static_cast<DWORD>(current_image.size()));
    const DWORD temporary_length = GetTempPathW(
        static_cast<DWORD>(temporary_root.size()), temporary_root.data());
    if (image_length == 0 || image_length >= current_image.size()
        || temporary_length == 0 || temporary_length >= temporary_root.size()) {
        return false;
    }

    const std::filesystem::path fixture = std::filesystem::path(temporary_root.data())
        / (L"mo-broker-launch-" + std::to_wstring(GetCurrentProcessId()) + L"-"
            + std::to_wstring(GetTickCount64()));
    const std::filesystem::path broker = fixture / L"mo-broker.exe";
    try {
        std::filesystem::create_directory(fixture);
        if (!CopyFileW(current_image.data(), broker.c_str(), TRUE)) {
            std::filesystem::remove_all(fixture);
            return false;
        }
    } catch (...) {
        return false;
    }

    DWORD process_id = 0;
    const bool started = mo::windows_tip::StartBrokerProcess(broker.wstring(), &process_id);
    HANDLE process = started ? OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
        FALSE, process_id) : nullptr;
    DWORD exit_code = STILL_ACTIVE;
    const bool exited = process != nullptr && WaitForSingleObject(process, 5000) == WAIT_OBJECT_0
        && GetExitCodeProcess(process, &exit_code) && exit_code == 2;
    if (process != nullptr) { CloseHandle(process); }
    try { std::filesystem::remove_all(fixture); } catch (...) { return false; }
    SetLastError(ERROR_SUCCESS);
    DWORD rejected_process_id = 99;
    const bool rejected = !mo::windows_tip::StartBrokerProcess(L"relative\\mo-broker.exe", &rejected_process_id)
        && rejected_process_id == 0 && GetLastError() == ERROR_INVALID_NAME;
    if (!started || process_id == 0 || !exited || !rejected) {
        std::wcerr << L"Broker process policy mismatch: started=" << started << L" pid=" << process_id
            << L" opened=" << (process != nullptr) << L" exit=" << exit_code
            << L" rejected=" << rejected << L'\n';
    }
    return started && process_id != 0 && exited && rejected;
}

class ComApartment final {
public:
    ComApartment() noexcept : result_(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED)) {}
    ~ComApartment() noexcept {
        if (SUCCEEDED(result_)) {
            CoUninitialize();
        }
    }

    ComApartment(const ComApartment&) = delete;
    ComApartment& operator=(const ComApartment&) = delete;

    HRESULT result() const noexcept { return result_; }

private:
    HRESULT result_;
};

// Probe-only observer. Callbacks never change focus, acquire context locks,
// dereference borrowed document/context pointers or emit unbounded logging.
class ProbeFocusRecorder final : public ITfThreadMgrEventSink, public ITfThreadFocusSink {
public:
    explicit ProbeFocusRecorder(ITfThreadMgr* manager) noexcept : manager_(manager) {}
    void ExpectedDocument(ITfDocumentMgr* document) noexcept { expected_ = document; }
    void Checkpoint(const wchar_t* checkpoint) noexcept { checkpoint_ = checkpoint; }
    void Report() const noexcept {
        ComPtr<ITfDocumentMgr> focus;
        const HRESULT result = manager_->GetFocus(&focus);
        std::wcerr << L"MO_DOCUMENT focus_result=" << result << L" focus=" << DocumentKind(focus.Get())
            << L" events=" << total_ << L" retained=" << (total_ < events_.size() ? total_ : events_.size()) << L'\n';
        const std::uint64_t first = total_ > events_.size() ? total_ - events_.size() : 0;
        for (std::uint64_t sequence = first; sequence < total_; ++sequence) {
            const auto& event = events_[static_cast<size_t>(sequence % events_.size())];
            std::wcerr << L"MO_FOCUS seq=" << sequence + 1 << L" kind=" << event.kind
                << L" current=" << event.current << L" previous=" << event.previous
                << L" checkpoint=" << event.checkpoint << L'\n';
        }
    }
    STDMETHODIMP QueryInterface(REFIID iid, void** object) noexcept override {
        if (object == nullptr) { return E_POINTER; }
        *object = nullptr;
        if (iid == IID_IUnknown || iid == IID_ITfThreadMgrEventSink) {
            *object = static_cast<ITfThreadMgrEventSink*>(this);
        } else if (iid == IID_ITfThreadFocusSink) { *object = static_cast<ITfThreadFocusSink*>(this); }
        else { return E_NOINTERFACE; }
        AddRef(); return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() noexcept override { return static_cast<ULONG>(InterlockedIncrement(&references_)); }
    STDMETHODIMP_(ULONG) Release() noexcept override {
        const LONG count = InterlockedDecrement(&references_);
        if (count == 0) { delete this; }
        return static_cast<ULONG>(count);
    }
    STDMETHODIMP OnInitDocumentMgr(ITfDocumentMgr* document) noexcept override { Record(1, DocumentKind(document), 0); return S_OK; }
    STDMETHODIMP OnUninitDocumentMgr(ITfDocumentMgr* document) noexcept override { Record(2, DocumentKind(document), 0); return S_OK; }
    STDMETHODIMP OnSetFocus(ITfDocumentMgr* current, ITfDocumentMgr* previous) noexcept override {
        Record(3, DocumentKind(current), DocumentKind(previous)); return S_OK;
    }
    STDMETHODIMP OnPushContext(ITfContext*) noexcept override { Record(4, 0, 0); return S_OK; }
    STDMETHODIMP OnPopContext(ITfContext*) noexcept override { Record(5, 0, 0); return S_OK; }
    STDMETHODIMP OnSetThreadFocus() noexcept override { Record(6, 0, 0); return S_OK; }
    STDMETHODIMP OnKillThreadFocus() noexcept override { Record(7, 0, 0); return S_OK; }
private:
    DWORD DocumentKind(ITfDocumentMgr* document) const noexcept {
        return document == nullptr ? 0 : document == expected_ ? 1 : 2;
    }
    void Record(DWORD kind, DWORD current, DWORD previous) noexcept {
        events_[static_cast<size_t>(total_ % events_.size())] = {kind, current, previous, checkpoint_};
        ++total_;
    }
    struct Event final { DWORD kind = 0, current = 0, previous = 0; const wchar_t* checkpoint = L"setup"; };
    LONG references_ = 1;
    ITfThreadMgr* manager_;
    ITfDocumentMgr* expected_ = nullptr;
    const wchar_t* checkpoint_ = L"setup";
    std::uint64_t total_ = 0;
    std::array<Event, 32> events_{};
};
ProbeFocusRecorder* g_probe_focus = nullptr;

class FakeThreadManager final : public ITfThreadMgr, public ITfKeystrokeMgr {
public:
    STDMETHODIMP QueryInterface(REFIID interface_id, void** object) noexcept override {
        if (object == nullptr) {
            return E_POINTER;
        }
        *object = nullptr;
        if (IsEqualIID(interface_id, IID_IUnknown)
            || IsEqualIID(interface_id, IID_ITfThreadMgr)) {
            *object = static_cast<ITfThreadMgr*>(this);
        } else if (IsEqualIID(interface_id, IID_ITfKeystrokeMgr)) {
            *object = static_cast<ITfKeystrokeMgr*>(this);
        } else {
            return E_NOINTERFACE;
        }
        AddRef();
        return S_OK;
    }

    STDMETHODIMP_(ULONG) AddRef() noexcept override {
        return static_cast<ULONG>(InterlockedIncrement(&reference_count_));
    }

    STDMETHODIMP_(ULONG) Release() noexcept override {
        const LONG count = InterlockedDecrement(&reference_count_);
        if (count == 0) {
            delete this;
            return 0;
        }
        return static_cast<ULONG>(count);
    }

    STDMETHODIMP Activate(TfClientId*) noexcept override { return E_NOTIMPL; }
    STDMETHODIMP Deactivate() noexcept override { return E_NOTIMPL; }
    STDMETHODIMP CreateDocumentMgr(ITfDocumentMgr**) noexcept override { return E_NOTIMPL; }
    STDMETHODIMP EnumDocumentMgrs(IEnumTfDocumentMgrs**) noexcept override { return E_NOTIMPL; }
    STDMETHODIMP GetFocus(ITfDocumentMgr**) noexcept override { return E_NOTIMPL; }
    STDMETHODIMP SetFocus(ITfDocumentMgr*) noexcept override { return E_NOTIMPL; }
    STDMETHODIMP AssociateFocus(HWND, ITfDocumentMgr*, ITfDocumentMgr**) noexcept override {
        return E_NOTIMPL;
    }
    STDMETHODIMP IsThreadFocus(BOOL*) noexcept override { return E_NOTIMPL; }
    STDMETHODIMP GetFunctionProvider(REFCLSID, ITfFunctionProvider**) noexcept override {
        return E_NOTIMPL;
    }
    STDMETHODIMP EnumFunctionProviders(IEnumTfFunctionProviders**) noexcept override {
        return E_NOTIMPL;
    }
    STDMETHODIMP GetGlobalCompartment(ITfCompartmentMgr**) noexcept override { return E_NOTIMPL; }

    STDMETHODIMP AdviseKeyEventSink(
        TfClientId client_id,
        ITfKeyEventSink* sink,
        BOOL foreground) noexcept override {
        if (client_id == TF_CLIENTID_NULL || sink == nullptr) {
            return E_INVALIDARG;
        }
        if (sink_ != nullptr) {
            return CONNECT_E_ADVISELIMIT;
        }
        sink->AddRef();
        sink_ = sink;
        client_id_ = client_id;
        foreground_ = foreground;
        return S_OK;
    }

    STDMETHODIMP UnadviseKeyEventSink(TfClientId client_id) noexcept override {
        if (sink_ == nullptr || client_id != client_id_) {
            return CONNECT_E_NOCONNECTION;
        }
        ITfKeyEventSink* sink = sink_;
        sink_ = nullptr;
        client_id_ = TF_CLIENTID_NULL;
        foreground_ = FALSE;
        sink->Release();
        return S_OK;
    }

    STDMETHODIMP GetForeground(CLSID*) noexcept override { return E_NOTIMPL; }
    STDMETHODIMP TestKeyDown(WPARAM, LPARAM, BOOL*) noexcept override { return E_NOTIMPL; }
    STDMETHODIMP TestKeyUp(WPARAM, LPARAM, BOOL*) noexcept override { return E_NOTIMPL; }
    STDMETHODIMP KeyDown(WPARAM, LPARAM, BOOL*) noexcept override { return E_NOTIMPL; }
    STDMETHODIMP KeyUp(WPARAM, LPARAM, BOOL*) noexcept override { return E_NOTIMPL; }
    STDMETHODIMP GetPreservedKey(
        ITfContext*,
        const TF_PRESERVEDKEY*,
        GUID*) noexcept override {
        return E_NOTIMPL;
    }
    STDMETHODIMP IsPreservedKey(
        REFGUID,
        const TF_PRESERVEDKEY*,
        BOOL*) noexcept override {
        return E_NOTIMPL;
    }
    STDMETHODIMP PreserveKey(
        TfClientId,
        REFGUID,
        const TF_PRESERVEDKEY*,
        const WCHAR*,
        ULONG) noexcept override {
        return E_NOTIMPL;
    }
    STDMETHODIMP UnpreserveKey(REFGUID, const TF_PRESERVEDKEY*) noexcept override {
        return E_NOTIMPL;
    }
    STDMETHODIMP SetPreservedKeyDescription(REFGUID, const WCHAR*, ULONG) noexcept override {
        return E_NOTIMPL;
    }
    STDMETHODIMP GetPreservedKeyDescription(REFGUID, BSTR*) noexcept override {
        return E_NOTIMPL;
    }
    STDMETHODIMP SimulatePreservedKey(ITfContext*, REFGUID, BOOL*) noexcept override {
        return E_NOTIMPL;
    }

    bool has_expected_sink(TfClientId client_id) const noexcept {
        return sink_ != nullptr && client_id_ == client_id && foreground_ != FALSE;
    }

private:
    ~FakeThreadManager() noexcept {
        if (sink_ != nullptr) {
            sink_->Release();
        }
    }

    volatile LONG reference_count_ = 1;
    ITfKeyEventSink* sink_ = nullptr;
    TfClientId client_id_ = TF_CLIENTID_NULL;
    BOOL foreground_ = FALSE;
};

// A deterministic ACP text store backed by a real EDIT control. The probe owns
// the application side of the TSF contract while msctf owns the context/ranges
// used by the TIP's edit sessions.
class EditTextStore final : public ITextStoreACP, public ITfContextOwnerCompositionSink {
public:
    explicit EditTextStore(HWND window) noexcept : window_(window) {}
    const std::wstring& text() const noexcept { return text_; }
    void NotifyLayoutChanged() noexcept {
        if (sink_ != nullptr) { sink_->OnLayoutChange(TS_LC_CHANGE, 1); }
    }
    void DeferLocks(bool defer) noexcept { defer_locks_ = defer; }
    using TextExtHook = void (*)(void*) noexcept;
    void OnNextTextExt(TextExtHook hook, void* context) noexcept {
        text_ext_hook_ = hook; text_ext_hook_context_ = context;
    }
    void OnNextWrite(TextExtHook hook, void* context) noexcept {
        write_hook_ = hook; write_hook_context_ = context;
    }
    bool has_deferred_lock() const noexcept { return deferred_lock_flags_ != 0; }
    void GrantDeferredLock() noexcept {
        const DWORD flags = deferred_lock_flags_;
        deferred_lock_flags_ = 0;
        if (flags != 0 && sink_ != nullptr) {
            lock_flags_ = flags;
            last_lock_result_ = sink_->OnLockGranted(flags);
            lock_flags_ = 0;
        }
    }

    STDMETHODIMP QueryInterface(REFIID interface_id, void** object) noexcept override {
        if (object == nullptr) {
            return E_POINTER;
        }
        *object = nullptr;
        if (IsEqualIID(interface_id, IID_IUnknown)
            || IsEqualIID(interface_id, IID_ITextStoreACP)) {
            *object = static_cast<ITextStoreACP*>(this);
        } else if (IsEqualIID(interface_id, IID_ITfContextOwnerCompositionSink)) {
            *object = static_cast<ITfContextOwnerCompositionSink*>(this);
        } else {
            return E_NOINTERFACE;
        }
        AddRef();
        return S_OK;
    }

    STDMETHODIMP_(ULONG) AddRef() noexcept override {
        return static_cast<ULONG>(InterlockedIncrement(&reference_count_));
    }

    STDMETHODIMP_(ULONG) Release() noexcept override {
        const LONG count = InterlockedDecrement(&reference_count_);
        if (count == 0) {
            delete this;
            return 0;
        }
        return static_cast<ULONG>(count);
    }

    STDMETHODIMP AdviseSink(REFIID interface_id, IUnknown* unknown, DWORD mask) noexcept override {
        if (!IsEqualIID(interface_id, IID_ITextStoreACPSink) || unknown == nullptr) {
            return E_INVALIDARG;
        }
        if (sink_ != nullptr) {
            return CONNECT_E_ADVISELIMIT;
        }
        HRESULT result = unknown->QueryInterface(IID_PPV_ARGS(sink_.GetAddressOf()));
        if (SUCCEEDED(result)) {
            sink_mask_ = mask;
        }
        return result;
    }

    STDMETHODIMP UnadviseSink(IUnknown* unknown) noexcept override {
        if (unknown == nullptr || sink_ == nullptr) {
            return CONNECT_E_NOCONNECTION;
        }
        ComPtr<IUnknown> advised_identity;
        ComPtr<IUnknown> supplied_identity;
        HRESULT result = sink_.As(&advised_identity);
        if (FAILED(result)) {
            return result;
        }
        result = unknown->QueryInterface(IID_PPV_ARGS(supplied_identity.GetAddressOf()));
        if (FAILED(result) || advised_identity.Get() != supplied_identity.Get()) {
            return CONNECT_E_NOCONNECTION;
        }
        sink_.Reset();
        sink_mask_ = 0;
        return S_OK;
    }

    STDMETHODIMP RequestLock(DWORD lock_flags, HRESULT* session_result) noexcept override {
        if (session_result == nullptr) {
            return E_POINTER;
        }
        if (sink_ == nullptr) {
            return E_UNEXPECTED;
        }
        if (lock_flags_ != 0) {
            *session_result = TS_E_SYNCHRONOUS;
            return S_OK;
        }
        if (defer_locks_) {
            if ((lock_flags & TS_LF_SYNC) != 0) { *session_result = TS_E_SYNCHRONOUS; }
            else { deferred_lock_flags_ = lock_flags; *session_result = TS_S_ASYNC; }
            return S_OK;
        }
        lock_flags_ = lock_flags;
        *session_result = sink_->OnLockGranted(lock_flags);
        last_lock_result_ = *session_result;
        lock_flags_ = 0;
        return S_OK;
    }

    HRESULT last_lock_result() const noexcept { return last_lock_result_; }

    STDMETHODIMP GetStatus(TS_STATUS* status) noexcept override {
        if (status == nullptr) {
            return E_POINTER;
        }
        status->dwDynamicFlags = 0;
        status->dwStaticFlags = 0;
        return S_OK;
    }

    STDMETHODIMP QueryInsert(
        LONG test_start,
        LONG test_end,
        ULONG length,
        LONG* result_start,
        LONG* result_end) noexcept override {
        if (result_start == nullptr || result_end == nullptr) {
            return E_POINTER;
        }
        if (!ValidRange(test_start, test_end)) {
            return TS_E_INVALIDPOS;
        }
        if (length > static_cast<ULONG>(std::numeric_limits<LONG>::max() - test_start)) {
            return E_INVALIDARG;
        }
        *result_start = test_start;
        *result_end = test_start + static_cast<LONG>(length);
        return S_OK;
    }

    STDMETHODIMP GetSelection(
        ULONG index,
        ULONG count,
        TS_SELECTION_ACP* selection,
        ULONG* fetched) noexcept override {
        if (!HasReadLock()) {
            return TS_E_NOLOCK;
        }
        if (selection == nullptr || fetched == nullptr) {
            return E_POINTER;
        }
        *fetched = 0;
        if ((index != 0 && index != TS_DEFAULT_SELECTION) || count == 0) {
            return E_INVALIDARG;
        }
        selection[0].acpStart = selection_start_;
        selection[0].acpEnd = selection_end_;
        selection[0].style.ase = TS_AE_END;
        selection[0].style.fInterimChar = FALSE;
        *fetched = 1;
        return S_OK;
    }

    STDMETHODIMP SetSelection(
        ULONG count,
        const TS_SELECTION_ACP* selection) noexcept override {
        if (!HasWriteLock()) {
            return TS_E_NOLOCK;
        }
        if (selection == nullptr || count != 1) {
            return E_INVALIDARG;
        }
        if (!ValidRange(selection[0].acpStart, selection[0].acpEnd)) {
            return TS_E_INVALIDPOS;
        }
        selection_start_ = selection[0].acpStart;
        selection_end_ = selection[0].acpEnd;
        SendMessageW(window_, EM_SETSEL, selection_start_, selection_end_);
        return S_OK;
    }

    STDMETHODIMP GetText(
        LONG start,
        LONG end,
        WCHAR* plain,
        ULONG plain_capacity,
        ULONG* plain_length,
        TS_RUNINFO* run_info,
        ULONG run_capacity,
        ULONG* run_count,
        LONG* next) noexcept override {
        if (!HasReadLock()) {
            return TS_E_NOLOCK;
        }
        if (plain_length == nullptr || run_count == nullptr || next == nullptr) {
            return E_POINTER;
        }
        if (end == -1) {
            end = static_cast<LONG>(text_.size());
        }
        if (!ValidRange(start, end)) {
            return TS_E_INVALIDPOS;
        }
        const ULONG available = static_cast<ULONG>(end - start);
        const ULONG copied = (std::min)(available, plain_capacity);
        if (copied != 0 && plain == nullptr) {
            return E_POINTER;
        }
        if (copied != 0) {
            std::copy_n(text_.data() + start, copied, plain);
        }
        *plain_length = copied;
        *run_count = 0;
        if (run_capacity != 0) {
            if (run_info == nullptr) {
                return E_POINTER;
            }
            run_info[0].uCount = copied;
            run_info[0].type = TS_RT_PLAIN;
            *run_count = 1;
        }
        *next = start + static_cast<LONG>(copied);
        return S_OK;
    }

    STDMETHODIMP SetText(
        DWORD,
        LONG start,
        LONG end,
        const WCHAR* replacement,
        ULONG replacement_length,
        TS_TEXTCHANGE* change) noexcept override {
        if (!HasWriteLock()) {
            return TS_E_NOLOCK;
        }
        return Replace(start, end, replacement, replacement_length, change);
    }

    STDMETHODIMP GetFormattedText(LONG, LONG, IDataObject**) noexcept override {
        return E_NOTIMPL;
    }
    STDMETHODIMP GetEmbedded(LONG, REFGUID, REFIID, IUnknown**) noexcept override {
        return E_NOTIMPL;
    }
    STDMETHODIMP QueryInsertEmbedded(const GUID*, const FORMATETC*, BOOL* insertable) noexcept override {
        if (insertable == nullptr) {
            return E_POINTER;
        }
        *insertable = FALSE;
        return S_OK;
    }
    STDMETHODIMP InsertEmbedded(DWORD, LONG, LONG, IDataObject*, TS_TEXTCHANGE*) noexcept override {
        return E_NOTIMPL;
    }

    STDMETHODIMP InsertTextAtSelection(
        DWORD flags,
        const WCHAR* replacement,
        ULONG replacement_length,
        LONG* start,
        LONG* end,
        TS_TEXTCHANGE* change) noexcept override {
        if (!HasWriteLock()) {
            return TS_E_NOLOCK;
        }
        if ((flags & TS_IAS_QUERYONLY) != 0) {
            if (start == nullptr || end == nullptr) {
                return E_POINTER;
            }
            *start = selection_start_;
            *end = selection_end_;
            return S_OK;
        }
        const LONG insertion_start = selection_start_;
        HRESULT result = Replace(
            selection_start_,
            selection_end_,
            replacement,
            replacement_length,
            change);
        if (FAILED(result)) {
            return result;
        }
        if ((flags & TS_IAS_NOQUERY) == 0) {
            if (start == nullptr || end == nullptr) {
                return E_POINTER;
            }
            *start = insertion_start;
            *end = selection_end_;
        }
        return S_OK;
    }

    STDMETHODIMP InsertEmbeddedAtSelection(
        DWORD,
        IDataObject*,
        LONG*,
        LONG*,
        TS_TEXTCHANGE*) noexcept override {
        return E_NOTIMPL;
    }
    STDMETHODIMP RequestSupportedAttrs(DWORD, ULONG, const TS_ATTRID*) noexcept override {
        return S_OK;
    }
    STDMETHODIMP RequestAttrsAtPosition(LONG, ULONG, const TS_ATTRID*, DWORD) noexcept override {
        return S_OK;
    }
    STDMETHODIMP RequestAttrsTransitioningAtPosition(
        LONG,
        ULONG,
        const TS_ATTRID*,
        DWORD) noexcept override {
        return S_OK;
    }
    STDMETHODIMP FindNextAttrTransition(
        LONG start,
        LONG,
        ULONG,
        const TS_ATTRID*,
        DWORD,
        LONG* next,
        BOOL* found,
        LONG* offset) noexcept override {
        if (next == nullptr || found == nullptr || offset == nullptr) {
            return E_POINTER;
        }
        *next = start;
        *found = FALSE;
        *offset = 0;
        return S_OK;
    }
    STDMETHODIMP RetrieveRequestedAttrs(ULONG, TS_ATTRVAL*, ULONG* fetched) noexcept override {
        if (fetched == nullptr) {
            return E_POINTER;
        }
        *fetched = 0;
        return S_OK;
    }
    STDMETHODIMP GetEndACP(LONG* end) noexcept override {
        if (!HasReadLock()) {
            return TS_E_NOLOCK;
        }
        if (end == nullptr) {
            return E_POINTER;
        }
        *end = static_cast<LONG>(text_.size());
        return S_OK;
    }
    STDMETHODIMP GetActiveView(TsViewCookie* view) noexcept override {
        if (view == nullptr) {
            return E_POINTER;
        }
        *view = 1;
        return S_OK;
    }
    STDMETHODIMP GetACPFromPoint(TsViewCookie, const POINT*, DWORD, LONG*) noexcept override {
        return E_NOTIMPL;
    }
    STDMETHODIMP GetTextExt(
        TsViewCookie,
        LONG start,
        LONG end,
        RECT* rectangle,
        BOOL* clipped) noexcept override {
        if (rectangle == nullptr || clipped == nullptr) {
            return E_POINTER;
        }
        GetClientRect(window_, rectangle);
        POINT origin{rectangle->left, rectangle->top};
        POINT extent{rectangle->right, rectangle->bottom};
        ClientToScreen(window_, &origin);
        ClientToScreen(window_, &extent);
        *rectangle = {origin.x, origin.y, extent.x, extent.y};
        if (start == end) { rectangle->right = rectangle->left; }
        *clipped = FALSE;
        const auto hook = text_ext_hook_;
        void* const hook_context = text_ext_hook_context_;
        text_ext_hook_ = nullptr; text_ext_hook_context_ = nullptr;
        if (hook != nullptr) { hook(hook_context); }
        return S_OK;
    }
    STDMETHODIMP GetScreenExt(TsViewCookie, RECT* rectangle) noexcept override {
        if (rectangle == nullptr) {
            return E_POINTER;
        }
        return GetWindowRect(window_, rectangle) != FALSE ? S_OK : HRESULT_FROM_WIN32(GetLastError());
    }
    STDMETHODIMP GetWnd(TsViewCookie, HWND* window) noexcept override {
        if (window == nullptr) {
            return E_POINTER;
        }
        *window = window_;
        return S_OK;
    }

    STDMETHODIMP OnStartComposition(ITfCompositionView*, BOOL* accepted) noexcept override {
        if (accepted == nullptr) {
            return E_POINTER;
        }
        *accepted = TRUE;
        return S_OK;
    }
    STDMETHODIMP OnUpdateComposition(ITfCompositionView*, ITfRange*) noexcept override {
        return S_OK;
    }
    STDMETHODIMP OnEndComposition(ITfCompositionView*) noexcept override { return S_OK; }

private:
    ~EditTextStore() noexcept = default;
    bool defer_locks_ = false;
    DWORD deferred_lock_flags_ = 0;
    TextExtHook text_ext_hook_ = nullptr;
    void* text_ext_hook_context_ = nullptr;
    TextExtHook write_hook_ = nullptr;
    void* write_hook_context_ = nullptr;

    bool HasReadLock() const noexcept { return (lock_flags_ & TS_LF_READ) != 0; }
    bool HasWriteLock() const noexcept {
        return (lock_flags_ & TS_LF_READWRITE) == TS_LF_READWRITE;
    }
    bool ValidRange(LONG start, LONG end) const noexcept {
        return start >= 0 && end >= start && static_cast<std::size_t>(end) <= text_.size();
    }
    HRESULT Replace(
        LONG start,
        LONG end,
        const WCHAR* replacement,
        ULONG replacement_length,
        TS_TEXTCHANGE* change) noexcept {
        if (!ValidRange(start, end) || (replacement == nullptr && replacement_length != 0)) {
            return TS_E_INVALIDPOS;
        }
        try {
            text_.replace(
                static_cast<std::size_t>(start),
                static_cast<std::size_t>(end - start),
                replacement == nullptr ? L"" : replacement,
                replacement_length);
        } catch (...) {
            return E_OUTOFMEMORY;
        }
        const LONG new_end = start + static_cast<LONG>(replacement_length);
        selection_start_ = new_end;
        selection_end_ = new_end;
        if (change != nullptr) {
            change->acpStart = start;
            change->acpOldEnd = end;
            change->acpNewEnd = new_end;
        }
        const auto hook = write_hook_;
        void* const hook_context = write_hook_context_;
        write_hook_ = nullptr; write_hook_context_ = nullptr;
        if (hook != nullptr) { hook(hook_context); }
        SetWindowTextW(window_, text_.c_str());
        SendMessageW(window_, EM_SETSEL, new_end, new_end);
        return S_OK;
    }

    volatile LONG reference_count_ = 1;
    HWND window_ = nullptr;
    ComPtr<ITextStoreACPSink> sink_;
    DWORD sink_mask_ = 0;
    DWORD lock_flags_ = 0;
    HRESULT last_lock_result_ = E_PENDING;
    std::wstring text_;
    LONG selection_start_ = 0;
    LONG selection_end_ = 0;
};

class ReadTextEditSession final : public ITfEditSession {
public:
    ReadTextEditSession(ITfContext* context, std::wstring* output) noexcept
        : context_(context), output_(output) {
        context_->AddRef();
    }

    STDMETHODIMP QueryInterface(REFIID interface_id, void** object) noexcept override {
        if (object == nullptr) {
            return E_POINTER;
        }
        *object = nullptr;
        if (IsEqualIID(interface_id, IID_IUnknown)
            || IsEqualIID(interface_id, IID_ITfEditSession)) {
            *object = static_cast<ITfEditSession*>(this);
            AddRef();
            return S_OK;
        }
        return E_NOINTERFACE;
    }

    STDMETHODIMP_(ULONG) AddRef() noexcept override {
        return static_cast<ULONG>(InterlockedIncrement(&reference_count_));
    }

    STDMETHODIMP_(ULONG) Release() noexcept override {
        const LONG count = InterlockedDecrement(&reference_count_);
        if (count == 0) {
            delete this;
            return 0;
        }
        return static_cast<ULONG>(count);
    }

    STDMETHODIMP DoEditSession(TfEditCookie edit_cookie) noexcept override {
        output_->clear();
        ComPtr<ITfRange> range;
        HRESULT result = context_->GetStart(edit_cookie, range.GetAddressOf());
        if (FAILED(result)) {
            return result;
        }
        LONG moved = 0;
        result = range->ShiftEnd(
            edit_cookie,
            std::numeric_limits<LONG>::max(),
            &moved,
            nullptr);
        if (FAILED(result)) {
            return result;
        }
        wchar_t buffer[64]{};
        ULONG length = 0;
        result = range->GetText(
            edit_cookie,
            TF_TF_MOVESTART,
            buffer,
            static_cast<ULONG>(std::size(buffer)),
            &length);
        if (FAILED(result)) {
            return result;
        }
        try {
            output_->assign(buffer, length);
        } catch (...) {
            return E_OUTOFMEMORY;
        }
        return S_OK;
    }

private:
    ~ReadTextEditSession() noexcept { context_->Release(); }

    volatile LONG reference_count_ = 1;
    ITfContext* context_;
    std::wstring* output_;
};

HRESULT ReadContextText(
    ITfContext* context,
    TfClientId client_id,
    std::wstring* output) noexcept {
    auto* edit_session = new (std::nothrow) ReadTextEditSession(context, output);
    if (edit_session == nullptr) {
        return E_OUTOFMEMORY;
    }
    HRESULT session_result = E_FAIL;
    const HRESULT request_result = context->RequestEditSession(
        client_id,
        edit_session,
        TF_ES_SYNC | TF_ES_READ,
        &session_result);
    edit_session->Release();
    return FAILED(request_result) ? request_result : session_result;
}

void PumpProbeMessages() noexcept;

bool SendTestedKey(
    ITfKeyEventSink* key_sink,
    ITfContext* context,
    EditTextStore* text_store,
    WPARAM virtual_key,
    bool notify_between_callbacks = false) {
    const UINT scan_code = MapVirtualKeyW(static_cast<UINT>(virtual_key), MAPVK_VK_TO_VSC);
    const LPARAM key_flags = static_cast<LPARAM>(scan_code) << 16;
    const LPARAM key_data = key_flags | (notify_between_callbacks ? 0 : 1);
    BOOL tested_eaten = FALSE;
    const ULONGLONG tested_started = GetTickCount64();
    HRESULT result = key_sink->OnTestKeyDown(context, virtual_key, key_data, &tested_eaten);
#ifdef MO_LATENCY_TRACE
    ComPtr<mo::windows_tip::IBrokerDiagnostics> diagnostics;
    if (SUCCEEDED(key_sink->QueryInterface(IID_PPV_ARGS(diagnostics.GetAddressOf())))) {
        mo::windows_tip::BrokerTiming timing;
        if (SUCCEEDED(diagnostics->ReadLastTiming(&timing))) {
            auto& output = tested_eaten ? std::wcout : std::wcerr;
            output << L"MO_CLIENT request=" << timing.request_id << L" kind=" << timing.kind
                << L" phase=" << timing.phase << L" error=" << timing.error << L" total_us=" << timing.total_us
                << L" write_us=" << timing.write_us << L" header_us=" << timing.header_us
                << L" payload_us=" << timing.payload_us << L" cancel_us=" << timing.cancel_us << L'\n';
            output << L"MO_DISPATCH total_us=" << timing.dispatch_total_us
                << L" pre_send_us=" << timing.dispatch_pre_send_us
                << L" connect_us=" << timing.dispatch_connect_us
                << L" modifiers_us=" << timing.dispatch_modifiers_us << L'\n';
        }
    }
#endif
    if (FAILED(result) || tested_eaten == FALSE) {
        if (FAILED(result)) {
            fail(L"ITfKeyEventSink::OnTestKeyDown", result);
        } else {
            std::wcerr << L"OnTestKeyDown did not consume virtual key 0x"
                       << std::hex << virtual_key << L" (elapsed " << std::dec
                       << GetTickCount64() - tested_started << L" ms)\n";
        }
        return false;
    }
    // A real Win10 Notepad host changes the low-word count across TSF
    // callbacks. Repeated queries must not advance the engine, and either
    // count direction must still reuse the single pending decision.
    for (const LPARAM repeat_count : {LPARAM{0}, LPARAM{1}, LPARAM{2}}) {
        BOOL repeated_eaten = FALSE;
        const HRESULT repeated_result = key_sink->OnTestKeyDown(
            context, virtual_key, key_flags | repeat_count, &repeated_eaten);
        if (FAILED(repeated_result) || repeated_eaten != tested_eaten) {
            std::wcerr << L"TSF repeated query changed its pending key decision\n";
            return false;
        }
    }
    if (notify_between_callbacks) {
        if (!mo::windows_tip::BroadcastSettingsChanged()) {
            std::wcerr << L"Settings broadcast failed between TSF key callbacks\n";
            return false;
        }
        PumpProbeMessages();
    }
    BOOL handled_eaten = FALSE;
    result = key_sink->OnKeyDown(context, virtual_key,
        key_flags | (notify_between_callbacks ? 1 : 0), &handled_eaten);
    if (FAILED(result) || handled_eaten != tested_eaten) {
        if (FAILED(result)) {
            fail(L"ITfKeyEventSink::OnKeyDown", result);
        } else {
            std::wcerr << L"OnKeyDown disagreed with its test callback for virtual key 0x"
                       << std::hex << virtual_key << L"; edit session=0x"
                       << text_store->last_lock_result() << L'\n';
        }
        return false;
    }
    if (FAILED(text_store->last_lock_result())) {
        fail(L"TIP edit-session text-store lock", text_store->last_lock_result());
        return false;
    }
    return true;
}

bool SendSystemTestedKey(
    ITfKeystrokeMgr* key_manager,
    EditTextStore* text_store,
    WPARAM virtual_key) {
    const UINT scan_code = MapVirtualKeyW(static_cast<UINT>(virtual_key), MAPVK_VK_TO_VSC);
    const LPARAM key_data = 1 | (static_cast<LPARAM>(scan_code) << 16);
    BOOL tested_eaten = FALSE;
    HRESULT result = key_manager->TestKeyDown(virtual_key, key_data, &tested_eaten);
    if (result != S_OK || tested_eaten == FALSE) {
        if (FAILED(result)) {
            fail(L"ITfKeystrokeMgr::TestKeyDown", result);
        } else {
            std::wcerr << L"Registered TIP did not consume virtual key 0x"
                       << std::hex << virtual_key << L"; result=0x" << result << L'\n';
        }
        return false;
    }
    BOOL handled_eaten = FALSE;
    result = key_manager->KeyDown(virtual_key, key_data, &handled_eaten);
    if (result != S_OK || handled_eaten != tested_eaten) {
        if (FAILED(result)) {
            fail(L"ITfKeystrokeMgr::KeyDown", result);
        } else {
            std::wcerr << L"Registered TIP key callback disagreed with its test callback for key 0x"
                       << std::hex << virtual_key << L"; result=0x" << result << L'\n';
        }
        return false;
    }
    if (FAILED(text_store->last_lock_result())) {
        fail(L"registered TIP edit-session text-store lock", text_store->last_lock_result());
        return false;
    }
    return true;
}

int probe_key_sink_activation(ITfTextInputProcessorEx* service) {
    ComPtr<ITfThreadMgr> thread_manager;
    HRESULT result = CoCreateInstance(
        CLSID_TF_ThreadMgr,
        nullptr,
        CLSCTX_INPROC_SERVER,
        IID_PPV_ARGS(&thread_manager));
    if (FAILED(result)) {
        return fail(L"CoCreateInstance(CLSID_TF_ThreadMgr)", result);
    }

    TfClientId application_client_id = TF_CLIENTID_NULL;
    result = thread_manager->Activate(&application_client_id);
    if (FAILED(result)) {
        return fail(L"ITfThreadMgr::Activate", result);
    }

    auto* fake_manager = new (std::nothrow) FakeThreadManager();
    if (fake_manager == nullptr) {
        thread_manager->Deactivate();
        return fail(L"FakeThreadManager allocation", E_OUTOFMEMORY);
    }
    ComPtr<ITfThreadMgr> service_thread_manager;
    service_thread_manager.Attach(static_cast<ITfThreadMgr*>(fake_manager));
    constexpr TfClientId service_client_id = 0x4d4f;

    int outcome = 0;
    bool service_active = false;
    bool context_pushed = false;
    ComPtr<ITfDocumentMgr> document_manager;
    ComPtr<ITfContext> context;
    do {
        result = thread_manager->CreateDocumentMgr(&document_manager);
        if (FAILED(result)) {
            outcome = fail(L"ITfThreadMgr::CreateDocumentMgr", result);
            break;
        }
        TfEditCookie edit_cookie = 0;
        result = document_manager->CreateContext(
            application_client_id,
            0,
            nullptr,
            &context,
            &edit_cookie);
        if (FAILED(result)) {
            outcome = fail(L"ITfDocumentMgr::CreateContext", result);
            break;
        }
        result = document_manager->Push(context.Get());
        if (FAILED(result)) {
            outcome = fail(L"ITfDocumentMgr::Push", result);
            break;
        }
        context_pushed = true;
        result = thread_manager->SetFocus(document_manager.Get());
        if (FAILED(result)) {
            outcome = fail(L"ITfThreadMgr::SetFocus", result);
            break;
        }

        result = service->ActivateEx(service_thread_manager.Get(), service_client_id, 0);
        if (FAILED(result)) {
            outcome = fail(L"ITfTextInputProcessorEx::ActivateEx", result);
            break;
        }
        service_active = true;
        if (!fake_manager->has_expected_sink(service_client_id)) {
            std::wcerr << L"ActivateEx did not install the expected foreground key sink\n";
            outcome = 1;
            break;
        }

        result = service->ActivateEx(service_thread_manager.Get(), service_client_id, 0);
        if (expect_result(
                L"ITfTextInputProcessorEx::ActivateEx(second)",
                result,
                TF_E_ALREADY_EXISTS)
            != 0) {
            outcome = 1;
            break;
        }

        ComPtr<ITfKeyEventSink> key_sink;
        result = service->QueryInterface(IID_PPV_ARGS(&key_sink));
        if (FAILED(result)) {
            outcome = fail(L"QueryInterface(ITfKeyEventSink)", result);
            break;
        }

        BOOL eaten = TRUE;
        result = key_sink->OnTestKeyDown(context.Get(), 'A', 1, &eaten);
        if (FAILED(result) || eaten != FALSE) {
            outcome = FAILED(result) ? fail(L"ITfKeyEventSink::OnTestKeyDown", result) : 1;
            break;
        }
        eaten = TRUE;
        result = key_sink->OnKeyDown(context.Get(), 'A', 1, &eaten);
        if (FAILED(result) || eaten != FALSE) {
            outcome = FAILED(result) ? fail(L"ITfKeyEventSink::OnKeyDown", result) : 1;
            break;
        }
        result = key_sink->OnSetFocus(TRUE);
        if (FAILED(result)) {
            outcome = fail(L"ITfKeyEventSink::OnSetFocus", result);
            break;
        }
    } while (false);

    if (service_active) {
        result = service->Deactivate();
        if (FAILED(result) && outcome == 0) {
            outcome = fail(L"ITfTextInputProcessor::Deactivate", result);
        }
        if (fake_manager->has_expected_sink(service_client_id) && outcome == 0) {
            std::wcerr << L"Deactivate left the key sink advised\n";
            outcome = 1;
        }
    }
    if (context_pushed) {
        result = document_manager->Pop(TF_POPF_ALL);
        if (FAILED(result) && outcome == 0) {
            outcome = fail(L"ITfDocumentMgr::Pop", result);
        }
    }
    result = thread_manager->Deactivate();
    if (FAILED(result) && outcome == 0) {
        outcome = fail(L"ITfThreadMgr::Deactivate", result);
    }
    return outcome;
}

HWND CandidateWindowForCurrentThread() noexcept {
    HWND result = nullptr;
    EnumThreadWindows(GetCurrentThreadId(), [](HWND window, LPARAM data) -> BOOL {
        wchar_t name[64]{};
        GetClassNameW(window, name, ARRAYSIZE(name));
        if (wcscmp(name, L"Mo.CandidateWindow.v1") == 0 && IsWindowVisible(window)) {
            *reinterpret_cast<HWND*>(data) = window; return FALSE;
        }
        return TRUE;
    }, reinterpret_cast<LPARAM>(&result));
    return result;
}

void PumpProbeMessages() noexcept {
    MSG message{};
    while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
        TranslateMessage(&message); DispatchMessageW(&message);
    }
}

void ReportCandidateState(ITfTextInputProcessorEx* service, const wchar_t* checkpoint) noexcept {
    // Always available in the test executable, even when production TIP
    // diagnostics are compiled out. Never log document or candidate content.
    std::wcerr << L"MO_CHECKPOINT checkpoint=" << checkpoint << L'\n';
    if (g_probe_focus != nullptr) { g_probe_focus->Report(); }
    EnumThreadWindows(GetCurrentThreadId(), [](HWND window, LPARAM) -> BOOL {
        wchar_t name[64]{};
        GetClassNameW(window, name, ARRAYSIZE(name));
        if (wcscmp(name, L"Mo.CandidateWindow.v1") == 0) {
            const HWND owner = GetWindow(window, GW_OWNER);
            std::wcerr << L"MO_WINDOW visible=" << IsWindowVisible(window)
                << L" owner_valid=" << IsWindow(owner)
                << L" owner_visible=" << IsWindowVisible(owner)
                << L" owner_iconic=" << IsIconic(owner) << L'\n';
        }
        return TRUE;
    }, 0);
#ifdef MO_LATENCY_TRACE
    ComPtr<mo::windows_tip::IBrokerDiagnostics> diagnostics;
    mo::windows_tip::BrokerTiming timing{};
    if (SUCCEEDED(service->QueryInterface(IID_PPV_ARGS(&diagnostics)))
        && SUCCEEDED(diagnostics->ReadLastTiming(&timing))) {
        std::wcerr << L"MO_VISUAL stage=" << timing.candidate_stage
            << L" result=" << timing.candidate_result << L" count=" << timing.candidate_count
            << L" focus=" << timing.candidate_focus << L" snapshot=" << timing.candidate_snapshot
            << L" reset=" << timing.candidate_reset << L" resets=" << timing.candidate_reset_count
            << L" edit_request=" << timing.edit_request << L" edit_session=" << timing.edit_session
            << L" request=" << timing.request_id << L" kind=" << timing.kind
            << L" phase=" << timing.phase << L" error=" << timing.error
            << L" total_us=" << timing.total_us
            << L" termination_owner_active=" << timing.termination_owner_active
            << L" termination_sent=" << timing.termination_sent
            << L" termination_owner_foreground=" << timing.termination_owner_foreground
            << L" termination_notifications=" << timing.termination_notification_count
            << L" dispatch_us=" << timing.dispatch_total_us << L" pre_send_us=" << timing.dispatch_pre_send_us
            << L" connect_us=" << timing.dispatch_connect_us << L" modifiers_us=" << timing.dispatch_modifiers_us << L'\n';
        std::wcerr << L"MO_TERMINATION_STACK count=" << timing.termination_frame_count << L'\n';
        for (DWORD i = 0; i < timing.termination_frame_count && i < mo::windows_tip::kTerminationFrameCapacity; ++i) {
            std::wcerr << L"MO_FRAME index=" << i << L" module=" << timing.termination_frames[i].module
                << L" rva=0x" << std::hex << timing.termination_frames[i].rva << std::dec << L'\n';
        }
    }
#else
    (void)service;
#endif
}

bool WaitForCandidateWindow(bool visible, ITfTextInputProcessorEx* service,
    const wchar_t* checkpoint) noexcept {
    if (g_probe_focus != nullptr) { g_probe_focus->Checkpoint(checkpoint); }
    const ULONGLONG deadline = GetTickCount64() + 2000;
    do {
        PumpProbeMessages();
        if ((CandidateWindowForCurrentThread() != nullptr) == visible) { return true; }
        MsgWaitForMultipleObjectsEx(0, nullptr, 10, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    } while (GetTickCount64() < deadline);
    std::wcerr << L"Candidate window visibility did not match expected state"
        << L" checkpoint=" << checkpoint << L" expected=" << visible << L'\n';
    ReportCandidateState(service, checkpoint);
    return false;
}

bool ClickCandidateWindow(int item) noexcept {
    const HWND window = CandidateWindowForCurrentThread();
    if (window == nullptr) { return false; }
    const auto styles = GetWindowLongPtrW(window, GWL_EXSTYLE);
    if ((styles & WS_EX_NOACTIVATE) == 0 || (styles & WS_EX_TOOLWINDOW) == 0
        || SendMessageW(window, WM_MOUSEACTIVATE, 0, 0) != MA_NOACTIVATE) {
        std::wcerr << L"Candidate window could steal document focus\n"; return false;
    }
    RECT client{}; GetClientRect(window, &client);
    const int dpi = static_cast<int>(GetDpiForWindow(window));
    const int x = item == 33 ? client.right * 3 / 4 : client.right / 4;
    const int y = item >= 32 ? client.bottom - MulDiv(16, dpi, 96)
        : MulDiv(40 + item * 32 + 16, dpi, 96);
    const HWND original_focus = GetFocus();
    const LPARAM position = MAKELPARAM(x, y);
    SendMessageW(window, WM_LBUTTONDOWN, MK_LBUTTON, position);
    SendMessageW(window, WM_LBUTTONUP, 0, position);
    PumpProbeMessages();
    if (GetFocus() != original_focus) {
        std::wcerr << L"Candidate click changed document focus\n"; return false;
    }
    return true;
}

// Test-only coordination with the harness. These events do not grant a peer a
// shutdown command: only the harness terminates its own Broker child process.
bool CoordinateBrokerFault(const wchar_t* prefix, const wchar_t* stage, int cycle) {
    const std::wstring name = std::wstring(prefix) + L"." + stage + L"." + std::to_wstring(cycle);
    const std::wstring acknowledge = name + L".done";
    const HANDLE request = OpenEventW(EVENT_MODIFY_STATE, FALSE, name.c_str());
    const HANDLE done = OpenEventW(SYNCHRONIZE, FALSE, acknowledge.c_str());
    if (request == nullptr || done == nullptr) {
        if (request != nullptr) { CloseHandle(request); }
        if (done != nullptr) { CloseHandle(done); }
        std::wcerr << L"Fault harness events missing\n"; return false;
    }
    bool succeeded = SetEvent(request) != FALSE;
    const ULONGLONG deadline = GetTickCount64() + 35000;
    while (succeeded) {
        const DWORD waited = MsgWaitForMultipleObjectsEx(1, &done, 10, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        if (waited == WAIT_OBJECT_0) { break; }
        if (waited == WAIT_FAILED || GetTickCount64() >= deadline) { succeeded = false; break; }
        PumpProbeMessages();
    }
    CloseHandle(done); CloseHandle(request);
    if (!succeeded) { std::wcerr << L"Fault harness coordination timed out\n"; }
    return succeeded;
}

bool SendFailOpenKey(ITfKeyEventSink* sink, ITfContext* context, WPARAM key) {
    const LPARAM data = 1 | (static_cast<LPARAM>(MapVirtualKeyW(static_cast<UINT>(key), MAPVK_VK_TO_VSC)) << 16);
    BOOL tested = TRUE, handled = TRUE;
    const ULONGLONG started = GetTickCount64();
    const HRESULT test = sink->OnTestKeyDown(context, key, data, &tested);
    const HRESULT apply = sink->OnKeyDown(context, key, data, &handled);
    if (FAILED(test) || FAILED(apply) || tested || handled || GetTickCount64() - started > 500) {
        std::wcerr << L"Dead Broker key was consumed or exceeded fail-open budget\n"; return false;
    }
    return true;
}

int probe_broker_input(
    ITfTextInputProcessorEx* service,
    bool rime_ice,
    bool registered,
    const wchar_t* fault_prefix = nullptr,
    bool activating_test_host = false) {
    if (!registered && service == nullptr) {
        return fail(L"probe_broker_input service", E_POINTER);
    }
    BYTE original_keyboard_state[256]{};
    const bool keyboard_state_saved = GetKeyboardState(original_keyboard_state) != FALSE;
    BYTE neutral_keyboard_state[256]{};
    SetKeyboardState(neutral_keyboard_state);

    ComPtr<ITfThreadMgr> thread_manager;
    HRESULT result = CoCreateInstance(
        CLSID_TF_ThreadMgr,
        nullptr,
        CLSCTX_INPROC_SERVER,
        IID_PPV_ARGS(thread_manager.GetAddressOf()));
    if (FAILED(result)) {
        return fail(L"CoCreateInstance(CLSID_TF_ThreadMgr)", result);
    }

    TfClientId client_id = TF_CLIENTID_NULL;
    DWORD thread_manager_flags = 0;
    if (!registered && !activating_test_host) {
        ComPtr<ITfThreadMgrEx> isolated_manager;
        result = thread_manager.As(&isolated_manager);
        if (SUCCEEDED(result)) {
            result = isolated_manager->ActivateEx(&client_id,
                TF_TMAE_NOACTIVATETIP | TF_TMAE_NOACTIVATEKEYBOARDLAYOUT);
        }
        if (SUCCEEDED(result)) { result = isolated_manager->GetActiveFlags(&thread_manager_flags); }
        if (SUCCEEDED(result) && (thread_manager_flags & TF_TMF_NOACTIVATETIP) == 0) {
            result = E_UNEXPECTED;
        }
    } else {
        result = thread_manager->Activate(&client_id);
    }
    if (FAILED(result)) {
        return fail(L"ITfThreadMgr activation policy", result);
    }

    int outcome = 0;
    bool service_active = false;
    bool profile_active = false;
    bool context_pushed = false;
    HWND edit_window = nullptr;
    ComPtr<ITfThreadMgr> service_thread_manager;
    if (!registered) {
        auto* fake_manager = new (std::nothrow) FakeThreadManager();
        if (fake_manager == nullptr) {
            thread_manager->Deactivate();
            return fail(L"FakeThreadManager allocation", E_OUTOFMEMORY);
        }
        service_thread_manager.Attach(static_cast<ITfThreadMgr*>(fake_manager));
    }
    ComPtr<ITextStoreACP> text_store;
    EditTextStore* edit_store = nullptr;
    ComPtr<ITfDocumentMgr> document_manager;
    ComPtr<ITfContext> context;
    ComPtr<ITfKeystrokeMgr> system_key_manager;
    ComPtr<ITfInputProcessorProfileMgr> profile_manager;
    ComPtr<ITfKeyEventSink> direct_key_sink;
    ComPtr<ProbeFocusRecorder> focus_recorder;
    ComPtr<ITfSource> focus_source;
    DWORD document_focus_cookie = TF_INVALID_COOKIE, thread_focus_cookie = TF_INVALID_COOKIE;
    struct FocusScope final {
        ~FocusScope() noexcept { g_probe_focus = nullptr; }
    } focus_scope;
    do {
        focus_recorder.Attach(new (std::nothrow) ProbeFocusRecorder(thread_manager.Get()));
        if (focus_recorder == nullptr) { outcome = fail(L"ProbeFocusRecorder allocation", E_OUTOFMEMORY); break; }
        result = thread_manager.As(&focus_source);
        if (SUCCEEDED(result)) {
            result = focus_source->AdviseSink(IID_ITfThreadMgrEventSink,
                static_cast<ITfThreadMgrEventSink*>(focus_recorder.Get()), &document_focus_cookie);
        }
        if (SUCCEEDED(result)) {
            result = focus_source->AdviseSink(IID_ITfThreadFocusSink,
                static_cast<ITfThreadFocusSink*>(focus_recorder.Get()), &thread_focus_cookie);
        }
        if (FAILED(result)) { outcome = fail(L"AdviseSink(probe focus observers)", result); break; }
        g_probe_focus = focus_recorder.Get();
        edit_window = CreateWindowExW(
            WS_EX_TOOLWINDOW | ((!registered && !activating_test_host) ? WS_EX_NOACTIVATE : 0),
            L"EDIT",
            L"",
            WS_POPUP | WS_BORDER | ES_AUTOHSCROLL,
            -32000,
            -32000,
            240,
            32,
            nullptr,
            nullptr,
            GetModuleHandleW(nullptr),
            nullptr);
        if (edit_window == nullptr) {
            outcome = fail(L"CreateWindowExW(EDIT)", HRESULT_FROM_WIN32(GetLastError()));
            break;
        }
        ShowWindow(edit_window, (!registered && !activating_test_host) ? SW_SHOWNOACTIVATE : SW_SHOW);
        // Controlled direct-sink tests model focus explicitly. They must not
        // obtain desktop focus merely by showing an off-screen EDIT popup.
        // Registered tests remain a separate real system-routing experiment.
        const bool owner_foreground = GetForegroundWindow() == edit_window;
        const bool owner_active = GetActiveWindow() == edit_window;
        std::wcout << L"MO_HOST noactivate=" << (!registered && !activating_test_host)
            << L" no_other_tip=" << ((thread_manager_flags & TF_TMF_NOACTIVATETIP) != 0)
            << L" created_foreground=" << owner_foreground << L" created_active=" << owner_active << L'\n';
        if (!registered && !activating_test_host && (owner_foreground || owner_active)) {
            std::wcerr << L"Nonactivating probe host acquired window activation\n";
            outcome = 1; break;
        }
        edit_store = new (std::nothrow) EditTextStore(edit_window);
        if (edit_store == nullptr) {
            outcome = fail(L"EditTextStore allocation", E_OUTOFMEMORY);
            break;
        }
        text_store.Attach(static_cast<ITextStoreACP*>(edit_store));

        result = thread_manager->CreateDocumentMgr(document_manager.GetAddressOf());
        if (FAILED(result)) {
            outcome = fail(L"ITfThreadMgr::CreateDocumentMgr", result);
            break;
        }
        TfEditCookie edit_cookie = 0;
        focus_recorder->ExpectedDocument(document_manager.Get());
        result = document_manager->CreateContext(
            client_id,
            0,
            text_store.Get(),
            context.GetAddressOf(),
            &edit_cookie);
        if (FAILED(result)) {
            outcome = fail(L"ITfDocumentMgr::CreateContext(ITextStoreACP)", result);
            break;
        }
        result = document_manager->Push(context.Get());
        if (FAILED(result)) {
            outcome = fail(L"ITfDocumentMgr::Push", result);
            break;
        }
        context_pushed = true;
        result = thread_manager->SetFocus(document_manager.Get());
        if (FAILED(result)) {
            outcome = fail(L"ITfThreadMgr::SetFocus", result);
            break;
        }
        if (registered) {
            result = CoCreateInstance(
                CLSID_TF_InputProcessorProfiles,
                nullptr,
                CLSCTX_INPROC_SERVER,
                IID_PPV_ARGS(profile_manager.GetAddressOf()));
            if (FAILED(result)) {
                outcome = fail(L"CoCreateInstance(CLSID_TF_InputProcessorProfiles)", result);
                break;
            }
            result = profile_manager->ActivateProfile(
                TF_PROFILETYPE_INPUTPROCESSOR,
                MAKELANGID(LANG_CHINESE, SUBLANG_CHINESE_SIMPLIFIED),
                mo::windows_tip::kTextServiceClsid,
                mo::windows_tip::kSimplifiedChineseProfileGuid,
                nullptr,
                TF_IPPMF_FORPROCESS | TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE);
            if (result != S_OK) {
                outcome = fail(L"ITfInputProcessorProfileMgr::ActivateProfile", result);
                break;
            }
            profile_active = true;
            result = thread_manager.As(&system_key_manager);
            if (FAILED(result)) {
                outcome = fail(L"QueryInterface(ITfKeystrokeMgr)", result);
                break;
            }
        } else {
            // The real context recognizes the client id returned above. The fake
            // manager captures the service key sink so the probe can invoke it
            // deterministically without registering Mo as a system TIP.
            result = service->ActivateEx(service_thread_manager.Get(), client_id, 0);
            if (FAILED(result)) {
                outcome = fail(L"ITfTextInputProcessorEx::ActivateEx", result);
                break;
            }
            service_active = true;

            result = service->QueryInterface(IID_PPV_ARGS(direct_key_sink.GetAddressOf()));
            if (FAILED(result)) {
                outcome = fail(L"QueryInterface(ITfKeyEventSink)", result);
                break;
            }
            result = direct_key_sink->OnSetFocus(TRUE);
            if (FAILED(result)) {
                outcome = fail(L"ITfKeyEventSink::OnSetFocus", result);
                break;
            }
        }

        const std::string input = rime_ice ? "NIHAO" : "M";
        bool keys_succeeded = true;
        for (int composition_index = 0; composition_index < 2 && keys_succeeded;
             ++composition_index) {
            for (const char key : input) {
                const bool key_succeeded = registered
                    ? SendSystemTestedKey(
                        system_key_manager.Get(), edit_store, static_cast<WPARAM>(key))
                    : SendTestedKey(
                        direct_key_sink.Get(),
                        context.Get(),
                        edit_store,
                        static_cast<WPARAM>(key),
                        composition_index == 0 && key == input.front());
                if (!key_succeeded) {
                    keys_succeeded = false;
                    break;
                }
            }
            if (!keys_succeeded) {
                break;
            }
            if (!WaitForCandidateWindow(true, service, L"initial-composition")) { keys_succeeded = false; break; }
            if (!registered) {
                const HWND appearance_window = CandidateWindowForCurrentThread();
                const HWND focus_before = GetFocus();
                const auto text_before = edit_store->text();
                RECT bounds_before{};
                GetWindowRect(appearance_window, &bounds_before);
                for (const UINT appearance : {WM_SYSCOLORCHANGE, WM_SETTINGCHANGE, WM_THEMECHANGED}) {
                    SendMessageW(appearance_window, appearance, 0, 0);
                    if (!GetUpdateRect(appearance_window, nullptr, FALSE)) {
                        std::wcerr << L"Candidate appearance message did not invalidate paint\n";
                        keys_succeeded = false; break;
                    }
                    UpdateWindow(appearance_window);
                    RECT bounds_after{};
                    GetWindowRect(appearance_window, &bounds_after);
                    if (GetUpdateRect(appearance_window, nullptr, FALSE)
                        || GetFocus() != focus_before || edit_store->text() != text_before
                        || !EqualRect(&bounds_before, &bounds_after)) {
                        std::wcerr << L"Candidate appearance repaint changed focus/text/layout\n";
                        keys_succeeded = false; break;
                    }
                }
                if (!keys_succeeded) { break; }
            }
            if (composition_index == 1 && !registered) {
                RECT before_layout{};
                GetWindowRect(CandidateWindowForCurrentThread(), &before_layout);
                SetWindowPos(edit_window, nullptr, 200, 180, 240, 32, SWP_NOACTIVATE | SWP_NOZORDER);
                edit_store->NotifyLayoutChanged();
                if (!WaitForCandidateWindow(true, service, L"owner-layout-change")) { keys_succeeded = false; break; }
                RECT after_layout{};
                GetWindowRect(CandidateWindowForCurrentThread(), &after_layout);
                if (EqualRect(&before_layout, &after_layout)) {
                    std::wcerr << L"Candidate window did not follow the changed text layout\n";
                    ReportCandidateState(service, L"layout-geometry-invariants");
                    keys_succeeded = false; break;
                }
                // A key-up returns an unhandled snapshot with a fresh revision.
                // A mouse press from before that refresh must not commit.
                const HWND candidate = CandidateWindowForCurrentThread();
                const int dpi = static_cast<int>(GetDpiForWindow(candidate));
                const LPARAM position = MAKELPARAM(20, MulDiv(56, dpi, 96));
                const auto before_release = edit_store->text();
                SendMessageW(candidate, WM_LBUTTONDOWN, MK_LBUTTON, position);
                BOOL release_eaten = TRUE;
                result = direct_key_sink->OnTestKeyUp(context.Get(), 'N', 0, &release_eaten);
                SendMessageW(candidate, WM_LBUTTONUP, 0, position);
                const bool unchanged_before_pump = edit_store->text() == before_release;
                PumpProbeMessages();
                if (FAILED(result) || release_eaten || edit_store->text() != before_release
                    || !WaitForCandidateWindow(true, service, L"stale-press-keyup-refresh")) {
                    std::wcerr << L"Candidate key-up refresh/stale press protection failed\n";
                    std::wcerr << L"MO_STALE_PRESS result=" << result << L" eaten=" << release_eaten
                        << L" unchanged_before_pump=" << unchanged_before_pump
                        << L" unchanged_after_pump=" << (edit_store->text() == before_release) << L'\n';
                    ReportCandidateState(service, L"stale-press-invariants");
                    keys_succeeded = false; break;
                }
                if (!ClickCandidateWindow(33) || !WaitForCandidateWindow(true, service, L"next-page")
                    || !ClickCandidateWindow(32) || !WaitForCandidateWindow(true, service, L"previous-page")
                    || !ClickCandidateWindow(0)) {
                    std::wcerr << L"Candidate mouse paging/selection failed\n";
                    ReportCandidateState(service, L"mouse-page-selection-invariants");
                    keys_succeeded = false; break;
                }
            } else {
                keys_succeeded = registered
                    ? SendSystemTestedKey(system_key_manager.Get(), edit_store, VK_SPACE)
                    : SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, VK_SPACE);
            }
            if (keys_succeeded && !WaitForCandidateWindow(false, service, L"composition-commit")) { keys_succeeded = false; }
        }
        if (!keys_succeeded) {
            outcome = 1;
            break;
        }

        if (!registered) {
            // Invalidate layout INSIDE GetTextExt, after it measured the old
            // geometry. The suspended read must not resurrect that stale view.
            for (const char key : input) {
                if (!SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, static_cast<WPARAM>(key))) {
                    keys_succeeded = false; break;
                }
            }
            ComPtr<ITfTextLayoutSink> layout_sink;
            ComPtr<ITfContextView> reentry_view;
            if (!keys_succeeded || FAILED(service->QueryInterface(IID_PPV_ARGS(&layout_sink)))
                || FAILED(context->GetActiveView(&reentry_view)) || reentry_view == nullptr) {
                outcome = 1; break;
            }
            struct LayoutReentry {
                ITfTextLayoutSink* sink;
                ITfContext* context;
                EditTextStore* store;
                ITfContextView* view;
                bool fired = false;
                HRESULT result = E_FAIL;
            } reentry{layout_sink.Get(), context.Get(), edit_store, reentry_view.Get()};
            edit_store->OnNextTextExt([](void* data) noexcept {
                auto& state = *static_cast<LayoutReentry*>(data);
                state.fired = true;
                state.store->DeferLocks(true);
                state.result = state.sink->OnLayoutChange(state.context, TF_LC_CHANGE, state.view);
            }, &reentry);
            const bool reentry_key = SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, 'A');
            edit_store->OnNextTextExt(nullptr, nullptr);
            const bool stale_visible = CandidateWindowForCurrentThread() != nullptr;
            edit_store->DeferLocks(false);
            edit_store->GrantDeferredLock();
            if (!reentry_key || !reentry.fired || FAILED(reentry.result) || stale_visible) {
                std::wcerr << L"Candidate layout invalidated during GetTextExt was resurrected\n";
                std::wcerr << L"MO_REENTRY key_ok=" << reentry_key << L" callback=" << reentry.fired
                    << L" result=" << reentry.result << L" stale_visible=" << stale_visible << L'\n';
                ReportCandidateState(service, L"layout-reentry-invariants");
                outcome = 1; break;
            }
            edit_store->NotifyLayoutChanged();
            if (!WaitForCandidateWindow(true, service, L"reentrant-layout-recovery")
                || !SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, VK_ESCAPE)
                || !WaitForCandidateWindow(false, service, L"reentrant-layout-escape")) {
                std::wcerr << L"Candidate layout did not recover after reentrant invalidation\n";
                ReportCandidateState(service, L"layout-recovery-invariants");
                outcome = 1; break;
            }
            std::wcout << L"Mo TIP candidate layout reentrancy probe passed.\n";

            // Force an actual deferred write lock. A newer synchronous key
            // supersedes the queued mouse action before it reaches the engine.
            for (const char key : input) {
                if (!SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, static_cast<WPARAM>(key))) {
                    keys_succeeded = false; break;
                }
            }
            edit_store->DeferLocks(true);
            const bool queued = ClickCandidateWindow(0) && edit_store->has_deferred_lock();
            edit_store->DeferLocks(false);
            const bool advanced = queued && SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, 'A');
            edit_store->GrantDeferredLock();
            PumpProbeMessages();
            if (!keys_succeeded || !advanced || !WaitForCandidateWindow(true, service, L"deferred-action-newer-key")
                || !SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, VK_ESCAPE)) {
                std::wcerr << L"Deferred candidate action did not cancel after a newer key\n";
                ReportCandidateState(service, L"deferred-action-invariants");
                outcome = 1; break;
            }

            // Focus loss must also cancel a queued action, clear only the
            // preedit and close the old engine session. Reconnection is tested
            // by another complete mouse-selection cycle below.
            for (const char key : input) {
                if (!SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, static_cast<WPARAM>(key))) {
                    keys_succeeded = false; break;
                }
            }
            edit_store->DeferLocks(true);
            const bool focus_queued = ClickCandidateWindow(0) && edit_store->has_deferred_lock();
            edit_store->DeferLocks(false);
            result = direct_key_sink->OnSetFocus(FALSE);
            edit_store->GrantDeferredLock();
            PumpProbeMessages();
            if (!keys_succeeded || !focus_queued || FAILED(result) || !WaitForCandidateWindow(false, service, L"deferred-focus-loss")) {
                std::wcerr << L"Deferred candidate action survived focus loss\n";
                ReportCandidateState(service, L"deferred-focus-invariants");
                outcome = 1; break;
            }
            result = direct_key_sink->OnSetFocus(TRUE);
            if (FAILED(result)) { outcome = fail(L"Candidate focus regain", result); break; }
            for (const char key : input) {
                if (!SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, static_cast<WPARAM>(key))) {
                    keys_succeeded = false; break;
                }
            }
            if (!keys_succeeded || !ClickCandidateWindow(0) || !WaitForCandidateWindow(false, service, L"focus-regain-selection")) {
                std::wcerr << L"Candidate selection did not recover after focus regain\n";
                ReportCandidateState(service, L"focus-recovery-invariants");
                outcome = 1; break;
            }
        }

        std::wstring expected = rime_ice
            ? (registered ? L"你好你好" : L"你好你好你好") : (registered ? L"mm" : L"mmm");
        for (int termination_case = 0; !registered && termination_case < 2; ++termination_case) {
            // Exercise the real TSF owner termination path, not a direct call
            // to the service sink with an invented edit cookie. Only our live
            // preedit may be discarded; the committed prefix must survive.
            ComPtr<ITfContextOwnerCompositionServices> owner_services;
            result = context->QueryInterface(IID_PPV_ARGS(&owner_services));
            if (FAILED(result)) { outcome = fail(L"Owner composition services", result); break; }
            for (const char key : input) {
                if (!SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, static_cast<WPARAM>(key))) {
                    keys_succeeded = false; break;
                }
            }
            if (!keys_succeeded || !WaitForCandidateWindow(true, service, L"owner-termination-before")) {
                outcome = 1; break;
            }
            if (termination_case == 1) {
                edit_store->DeferLocks(true);
                const bool queued = ClickCandidateWindow(0) && edit_store->has_deferred_lock();
                edit_store->DeferLocks(false);
                if (!queued) {
                    std::wcerr << L"Owner termination probe did not defer candidate action\n";
                    ReportCandidateState(service, L"owner-termination-queue-invariants");
                    outcome = 1; break;
                }
            }
            struct TerminationReentry {
                ITfKeyEventSink* sink;
                ITfContext* context;
                bool fired = false, fail_open = false;
            } termination_reentry{direct_key_sink.Get(), context.Get()};
            edit_store->OnNextWrite([](void* data) noexcept {
                auto& state = *static_cast<TerminationReentry*>(data);
                state.fired = true;
                state.fail_open = SendFailOpenKey(state.sink, state.context, 'A');
            }, &termination_reentry);
            result = owner_services->TerminateComposition(nullptr);
            edit_store->OnNextWrite(nullptr, nullptr);
            std::wcout << L"MO_TERMINATION_REENTRY fired=" << termination_reentry.fired
                << L" fail_open=" << termination_reentry.fail_open << L'\n';
            bool termination_count_ok = true;
#ifdef MO_LATENCY_TRACE
            ReportCandidateState(service, L"explicit-owner-termination");
            ComPtr<mo::windows_tip::IBrokerDiagnostics> termination_diagnostics;
            mo::windows_tip::BrokerTiming termination_timing{};
            termination_count_ok = SUCCEEDED(service->QueryInterface(IID_PPV_ARGS(&termination_diagnostics)))
                && SUCCEEDED(termination_diagnostics->ReadLastTiming(&termination_timing))
                && termination_timing.termination_notification_count
                    == static_cast<std::uint64_t>(termination_case + 1);
            if (!termination_count_ok) {
                std::wcerr << L"Unexpected host termination entered controlled probe\n";
            }
#endif
            edit_store->GrantDeferredLock();
            PumpProbeMessages();
            if (!termination_count_ok || FAILED(result)
                || !WaitForCandidateWindow(false, service, L"owner-termination-after")
                || edit_store->text() != expected) {
                std::wcerr << L"Owner termination left preedit or changed committed prefix"
                    << L" result=" << result << L" prefix_intact=" << (edit_store->text() == expected) << L'\n';
                ReportCandidateState(service, L"owner-termination-prefix-invariants");
                outcome = 1; break;
            }
            if (!termination_reentry.fired || !termination_reentry.fail_open) {
                std::wcerr << L"Owner termination accepted reentrant input or did not clear its range\n";
                ReportCandidateState(service, L"owner-termination-reentry-invariants");
                outcome = 1; break;
            }
            result = direct_key_sink->OnSetFocus(TRUE);
            if (FAILED(result)) { outcome = fail(L"Owner termination recovery focus", result); break; }
            for (const char key : input) {
                if (!SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, static_cast<WPARAM>(key))) {
                    keys_succeeded = false; break;
                }
            }
            if (!keys_succeeded || !WaitForCandidateWindow(true, service, L"owner-termination-recovery")
                || !SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, VK_SPACE)
                || !WaitForCandidateWindow(false, service, L"owner-termination-recovery-commit")) {
                outcome = 1; break;
            }
            expected += rime_ice ? L"你好" : L"m";
            if (edit_store->text() != expected) {
                std::wcerr << L"Owner termination recovery duplicated or lost committed text\n";
                ReportCandidateState(service, L"owner-termination-recovery-invariants");
                outcome = 1; break;
            }
            std::wcout << L"Mo TIP TSF owner termination/recovery probe passed.\n";
        }
        if (outcome != 0) { break; }
        if (fault_prefix != nullptr) {
            for (int cycle = 0; cycle < 2 && keys_succeeded; ++cycle) {
                // First crash: preedit + mouse action awaiting a real TSF lock.
                // Second crash: no preedit, previously committed text exists.
                if (cycle == 0) {
                    for (const char key : input) {
                        if (!SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, static_cast<WPARAM>(key))) {
                            keys_succeeded = false; break;
                        }
                    }
                    edit_store->DeferLocks(true);
                    keys_succeeded = keys_succeeded && ClickCandidateWindow(0) && edit_store->has_deferred_lock();
                }
                const auto before_fault = edit_store->text();
                if (!keys_succeeded || !CoordinateBrokerFault(fault_prefix, L"stop", cycle)) {
                    keys_succeeded = false; break;
                }
                if (edit_store->text() != before_fault) {
                    std::wcerr << L"Pending candidate ran before deliberate Broker exit\n";
                    keys_succeeded = false; break;
                }
                // Keep ALL locks deferred while coordination pumps messages:
                // another layout lock could otherwise execute the pending write
                // before the harness has actually terminated the Broker.
                if (cycle == 0) {
                    edit_store->DeferLocks(false);
                    edit_store->GrantDeferredLock(); PumpProbeMessages();
                }
                if (!SendFailOpenKey(direct_key_sink.Get(), context.Get(), VK_SPACE)
                    || edit_store->text() != expected || !WaitForCandidateWindow(false, service, L"broker-crash-fail-open")) {
                    std::wcerr << L"Broker crash changed document or replayed pending candidate\n";
                    ReportCandidateState(service, L"broker-crash-invariants");
                    keys_succeeded = false; break;
                }
                if (!CoordinateBrokerFault(fault_prefix, L"restart", cycle)
                    || FAILED(direct_key_sink->OnSetFocus(TRUE))) {
                    keys_succeeded = false; break;
                }
                for (const char key : input) {
                    if (!SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, static_cast<WPARAM>(key))) {
                        keys_succeeded = false; break;
                    }
                }
                if (!keys_succeeded || !WaitForCandidateWindow(true, service, L"broker-restart-composition")
                    || !SendTestedKey(direct_key_sink.Get(), context.Get(), edit_store, VK_SPACE)
                    || !WaitForCandidateWindow(false, service, L"broker-restart-commit")) { keys_succeeded = false; break; }
                expected += rime_ice ? L"你好" : L"m";
                if (edit_store->text() != expected) {
                    std::wcerr << L"Recovered session replayed/concatenated pre-crash input: actual=["
                        << edit_store->text() << L"] expected=[" << expected << L"]\n";
                    keys_succeeded = false; break;
                }
                // An additional Space is unhandled and must not replay commit.
                if (!SendFailOpenKey(direct_key_sink.Get(), context.Get(), VK_SPACE)
                    || edit_store->text() != expected) { keys_succeeded = false; break; }
            }
            if (!keys_succeeded) { outcome = 1; break; }
        }
        wchar_t text_buffer[32]{};
        const int text_length = GetWindowTextW(edit_window, text_buffer, ARRAYSIZE(text_buffer));
        const std::wstring text(text_buffer, static_cast<std::size_t>(text_length));
        if (text != expected) {
            std::wcerr << L"TIP edit session committed unexpected EDIT text: " << text << L'\n';
            outcome = 1;
            break;
        }
        std::wstring context_text;
        result = ReadContextText(context.Get(), client_id, &context_text);
        if (FAILED(result)) {
            outcome = fail(L"ReadContextText", result);
            break;
        }
        if (context_text != expected) {
            std::wcerr << L"TIP context exposed unexpected committed text: "
                       << context_text << L'\n';
            outcome = 1;
            break;
        }
    } while (false);

    if (profile_active) {
        result = profile_manager->DeactivateProfile(
            TF_PROFILETYPE_INPUTPROCESSOR,
            MAKELANGID(LANG_CHINESE, SUBLANG_CHINESE_SIMPLIFIED),
            mo::windows_tip::kTextServiceClsid,
            mo::windows_tip::kSimplifiedChineseProfileGuid,
            nullptr,
            TF_IPPMF_FORPROCESS);
        if (FAILED(result) && outcome == 0) {
            outcome = fail(L"ITfInputProcessorProfileMgr::DeactivateProfile", result);
        }
    }
    if (service_active) {
        result = service->Deactivate();
        if (FAILED(result) && outcome == 0) {
            outcome = fail(L"ITfTextInputProcessor::Deactivate", result);
        }
    }
    g_probe_focus = nullptr;
    if (document_focus_cookie != TF_INVALID_COOKIE) { focus_source->UnadviseSink(document_focus_cookie); }
    if (thread_focus_cookie != TF_INVALID_COOKIE) { focus_source->UnadviseSink(thread_focus_cookie); }
    if (context_pushed) {
        result = document_manager->Pop(TF_POPF_ALL);
        if (FAILED(result) && outcome == 0) {
            outcome = fail(L"ITfDocumentMgr::Pop", result);
        }
    }
    context.Reset();
    document_manager.Reset();
    text_store.Reset();
    if (edit_window != nullptr) {
        DestroyWindow(edit_window);
    }
    result = thread_manager->Deactivate();
    if (FAILED(result) && outcome == 0) {
        outcome = fail(L"ITfThreadMgr::Deactivate", result);
    }
    if (keyboard_state_saved) {
        SetKeyboardState(original_keyboard_state);
    }
    return outcome;
}

}  // namespace

int wmain(int argument_count, wchar_t** arguments) {
    if (argument_count == 1) {
        // Keep the launcher fixture alive long enough for both 32- and 64-bit
        // parents to reopen its PID before it returns the normal usage error.
        Sleep(250);
    }
    const bool activating_test_host = argument_count == 5
        && std::wstring(arguments[4]) == L"--activating-test-host";
    const int mode_argument_count = activating_test_host ? 4 : argument_count;
    const bool registered_broker_input = argument_count == 2
        && std::wstring(arguments[1]) == L"--registered-broker-input";
    const bool registered_broker_rime_ice = argument_count == 2
        && std::wstring(arguments[1]) == L"--registered-broker-rime-ice";
    const bool broker_input = argument_count == 3
        && std::wstring(arguments[2]) == L"--broker-input";
    const bool broker_rime_ice = argument_count == 3
        && std::wstring(arguments[2]) == L"--broker-rime-ice";
    const bool broker_fault = mode_argument_count == 4
        && std::wstring(arguments[2]) == L"--broker-fault";
    const bool broker_fault_rime_ice = mode_argument_count == 4
        && std::wstring(arguments[2]) == L"--broker-fault-rime-ice";
    if ((!registered_broker_input && !registered_broker_rime_ice)
        && argument_count != 2 && !broker_input && !broker_rime_ice && !broker_fault && !broker_fault_rime_ice) {
        std::wcerr
            << L"Usage: mo_tip_abi_probe <absolute-path-to-mo_tip.dll> "
               L"[--broker-input|--broker-rime-ice|--broker-fault[...-rime-ice] <event-prefix>]\n"
            << L"       mo_tip_abi_probe "
               L"--registered-broker-input|--registered-broker-rime-ice\n";
        return 2;
    }

    const ComApartment apartment;
    if (FAILED(apartment.result())) {
        return fail(L"CoInitializeEx", apartment.result());
    }

    if (registered_broker_input || registered_broker_rime_ice) {
        const bool rime_ice = registered_broker_rime_ice;
        if (probe_broker_input(nullptr, rime_ice, true) != 0) {
            return 1;
        }
        std::wcout << (rime_ice
            ? L"Registered Mo TIP Broker/librime/rime-ice system key route probe passed.\n"
            : L"Registered Mo TIP Broker system key route probe passed.\n");
        return 0;
    }

    const HMODULE module = LoadLibraryExW(
        arguments[1],
        nullptr,
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
    if (module == nullptr) {
        return fail(L"LoadLibraryExW", HRESULT_FROM_WIN32(GetLastError()));
    }

    const auto get_class_object = reinterpret_cast<DllGetClassObjectFunction>(
        GetProcAddress(module, "DllGetClassObject"));
    const auto can_unload = reinterpret_cast<DllCanUnloadNowFunction>(
        GetProcAddress(module, "DllCanUnloadNow"));
    if (get_class_object == nullptr || can_unload == nullptr) {
        const int result = fail(L"GetProcAddress", HRESULT_FROM_WIN32(GetLastError()));
        FreeLibrary(module);
        return result;
    }

    void* unavailable = nullptr;
    HRESULT result = get_class_object(GUID_NULL, IID_IClassFactory, &unavailable);
    if (expect_result(L"DllGetClassObject(unknown CLSID)", result, CLASS_E_CLASSNOTAVAILABLE)
            != 0
        || unavailable != nullptr) {
        FreeLibrary(module);
        return 1;
    }

    IClassFactory* factory = nullptr;
    result = get_class_object(
        mo::windows_tip::kTextServiceClsid,
        IID_IClassFactory,
        reinterpret_cast<void**>(&factory));
    if (FAILED(result)) {
        FreeLibrary(module);
        return fail(L"DllGetClassObject", result);
    }

    result = factory->LockServer(TRUE);
    if (FAILED(result)) {
        factory->Release();
        FreeLibrary(module);
        return fail(L"IClassFactory::LockServer(TRUE)", result);
    }
    factory->Release();
    if (expect_result(L"DllCanUnloadNow(locked)", can_unload(), S_FALSE) != 0) {
        FreeLibrary(module);
        return 1;
    }

    result = get_class_object(
        mo::windows_tip::kTextServiceClsid,
        IID_IClassFactory,
        reinterpret_cast<void**>(&factory));
    if (FAILED(result)) {
        FreeLibrary(module);
        return fail(L"DllGetClassObject(after lock)", result);
    }
    result = factory->LockServer(FALSE);
    if (FAILED(result)) {
        factory->Release();
        FreeLibrary(module);
        return fail(L"IClassFactory::LockServer(FALSE)", result);
    }

    void* aggregated = nullptr;
    result = factory->CreateInstance(
        factory,
        IID_ITfTextInputProcessorEx,
        &aggregated);
    if (expect_result(L"IClassFactory::CreateInstance(aggregated)", result, CLASS_E_NOAGGREGATION)
            != 0
        || aggregated != nullptr) {
        factory->Release();
        FreeLibrary(module);
        return 1;
    }

    ITfTextInputProcessorEx* service = nullptr;
    result = factory->CreateInstance(
        nullptr,
        IID_ITfTextInputProcessorEx,
        reinterpret_cast<void**>(&service));
    factory->Release();
    if (FAILED(result)) {
        FreeLibrary(module);
        return fail(L"IClassFactory::CreateInstance", result);
    }
    if (!ProbeCandidatePalette()) {
        service->Release(); FreeLibrary(module);
        return fail(L"Candidate palette/accessibility policy", E_FAIL);
    }
    if (!ProbeDeadlineArithmetic()) {
        service->Release(); FreeLibrary(module); return fail(L"Deadline arithmetic", E_FAIL);
    }
    if (!ProbeBrokerLauncher()) {
        service->Release(); FreeLibrary(module); return fail(L"Broker launcher policy", E_FAIL);
    }
    if (!ProbeSettingsChangeWindow()) {
        service->Release(); FreeLibrary(module); return fail(L"Settings change notification window", E_FAIL);
    }
    {
        const IID retired_iids[] = {
            {0x1DE6A239, 0x4965, 0x487B, {0xA8, 0x86, 0x21, 0x2C, 0x37, 0x5F, 0x37, 0x08}},
            {0xB37A59F4, 0x8F18, 0x4D58, {0x90, 0xD2, 0x37, 0xA5, 0x16, 0xD0, 0x41, 0x97}},
            {0xD46387F9, 0x3B9D, 0x48AD, {0xBB, 0x87, 0x18, 0x4D, 0x32, 0x2A, 0x6E, 0x65}}
        };
        for (const auto& iid : retired_iids) {
            IUnknown* retired = nullptr;
            result = service->QueryInterface(iid, reinterpret_cast<void**>(&retired));
            if (retired != nullptr) { retired->Release(); }
            if (result != E_NOINTERFACE || retired != nullptr) {
                service->Release(); FreeLibrary(module); return fail(L"Retired diagnostics ABI is still exposed", E_FAIL);
            }
        }
    }
#ifdef MO_LATENCY_TRACE
    {
        ComPtr<mo::windows_tip::IBrokerDiagnostics> diagnostics;
        result = service->QueryInterface(IID_PPV_ARGS(&diagnostics));
        if (FAILED(result)) {
            service->Release(); FreeLibrary(module); return fail(L"QueryInterface(IBrokerDiagnostics)", result);
        }
        mo::windows_tip::BrokerTiming timing{};
        ComPtr<IUnknown> service_identity, diagnostics_identity;
        service->QueryInterface(IID_PPV_ARGS(&service_identity));
        diagnostics->QueryInterface(IID_PPV_ARGS(&diagnostics_identity));
        if (diagnostics->ReadLastTiming(nullptr) != E_POINTER || diagnostics->ReadLastTiming(&timing) != S_OK
            || timing.total_us != 0 || timing.request_id != 0 || timing.error != 0
            || timing.candidate_snapshot != 0 || timing.candidate_reset != 0 || timing.candidate_reset_count != 0
            || timing.edit_request != S_OK || timing.edit_session != S_OK
            || timing.termination_owner_active != 0 || timing.termination_sent != 0 || timing.termination_owner_foreground != 0
            || timing.dispatch_total_us != 0 || timing.dispatch_pre_send_us != 0
            || timing.dispatch_connect_us != 0 || timing.dispatch_modifiers_us != 0
            || timing.termination_notification_count != 0
            || timing.termination_frame_count != 0
            || service_identity.Get() == nullptr || service_identity.Get() != diagnostics_identity.Get()) {
            // ComPtrs must die before unloading the module on this failure path.
            diagnostics.Reset(); service_identity.Reset(); diagnostics_identity.Reset();
            service->Release(); FreeLibrary(module); return fail(L"Read-only diagnostics contract", E_FAIL);
        }
    }
#else
    {
        const IID diagnostics_iid = {0x4EAD6830, 0x8CB1, 0x4C04, {0xA7, 0x03, 0x8F, 0x1E, 0x90, 0x99, 0x08, 0x22}};
        IUnknown* diagnostics = nullptr;
        result = service->QueryInterface(diagnostics_iid, reinterpret_cast<void**>(&diagnostics));
        if (diagnostics != nullptr) { diagnostics->Release(); }
        if (result != E_NOINTERFACE || diagnostics != nullptr) {
            service->Release(); FreeLibrary(module); return fail(L"Default build exposes diagnostics", E_FAIL);
        }
    }
#endif

    ITfCompositionSink* composition_sink = nullptr;
    result = service->QueryInterface(
        IID_ITfCompositionSink,
        reinterpret_cast<void**>(&composition_sink));
    if (FAILED(result)) {
        service->Release();
        FreeLibrary(module);
        return fail(L"QueryInterface(ITfCompositionSink)", result);
    }
    result = composition_sink->OnCompositionTerminated(0, nullptr);
    composition_sink->Release();
    if (expect_result(L"ITfCompositionSink::OnCompositionTerminated(NULL)", result, E_INVALIDARG)
        != 0) {
        service->Release();
        FreeLibrary(module);
        return 1;
    }

    result = service->ActivateEx(nullptr, TF_CLIENTID_NULL, 0);
    ITfTextLayoutSink* layout_sink = nullptr;
    const HRESULT layout_query = service->QueryInterface(IID_PPV_ARGS(&layout_sink));
    if (FAILED(layout_query)) {
        service->Release(); FreeLibrary(module); return fail(L"QueryInterface(ITfTextLayoutSink)", layout_query);
    }
    const HRESULT layout_result = layout_sink->OnLayoutChange(nullptr, TF_LC_CHANGE, nullptr);
    layout_sink->Release();
    if (expect_result(L"OnLayoutChange(NULL)", layout_result, E_INVALIDARG) != 0) {
        service->Release(); FreeLibrary(module); return 1;
    }
    if (expect_result(L"ITfTextInputProcessorEx::ActivateEx(NULL)", result, E_INVALIDARG) != 0) {
        service->Release();
        FreeLibrary(module);
        return 1;
    }
    result = service->Deactivate();
    if (FAILED(result)) {
        service->Release();
        FreeLibrary(module);
        return fail(L"ITfTextInputProcessor::Deactivate", result);
    }

    if ((broker_input || broker_rime_ice || broker_fault || broker_fault_rime_ice
             ? probe_broker_input(service, broker_rime_ice || broker_fault_rime_ice, false,
                 broker_fault || broker_fault_rime_ice ? arguments[3] : nullptr, activating_test_host)
             : probe_key_sink_activation(service))
        != 0) {
        service->Release();
        FreeLibrary(module);
        return 1;
    }

    service->Release();
    result = can_unload();
    if (result != S_OK) {
        FreeLibrary(module);
        return fail(L"DllCanUnloadNow", result);
    }

    FreeLibrary(module);
    std::wcout << (broker_fault || broker_fault_rime_ice
        ? L"Mo TIP Broker crash/restart probe passed (two exits, fail-open, no commit replay, EDIT/context).\n"
        : broker_rime_ice
        ? L"Mo TIP real rime-ice candidate window/mouse/layout/deferred cancellation/reconnect probe passed.\n"
        : broker_input
            ? L"Mo TIP fake candidate window/mouse/layout/deferred cancellation/reconnect probe passed.\n"
            : L"Mo TIP ABI probe passed (load, exports, class factory, Ex/key sink lifecycle, unload).\n");
    return 0;
}
