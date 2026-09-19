#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <msctf.h>
#include <array>
#include <chrono>
#include <cstdint>
#include <filesystem>
#include <limits>
#include <new>
#include <string>
#include <utility>
#include <wrl/client.h>

#include "mo_broker_client.h"
#include "mo_candidate_window.h"
#include "mo_tip_ids.h"

namespace {

using Microsoft::WRL::ComPtr;

volatile LONG g_live_objects = 0;
volatile LONG g_server_locks = 0;
HINSTANCE g_module = nullptr;

constexpr DWORD kBrokerActivationTimeoutMs = 400;
constexpr DWORD kBrokerKeyTimeoutMs = 50;
constexpr ULONGLONG kBrokerReconnectBackoffMs = 250;
constexpr std::uint16_t kShiftModifier = 1U << 0U;
constexpr std::uint16_t kControlModifier = 1U << 1U;
constexpr std::uint16_t kAltModifier = 1U << 2U;
constexpr std::uint16_t kSuperModifier = 1U << 3U;
constexpr std::uint16_t kCapsLockModifier = 1U << 4U;

enum class CandidateReset : DWORD {
    HostTermination = 1, LayoutDestroy = 2, Transport = 3, KeyEditFailure = 4,
    MouseRequestFailure = 5, MouseEditFailure = 6, UpdateFailure = 7,
    CommitFailure = 8, ClearFailure = 9, Finished = 10, Cancelled = 11,
    SnapshotAllocation = 12,
};

#ifdef MO_LATENCY_TRACE
mo::windows_tip::TerminationFrame DescribeTerminationFrame(void* address) noexcept {
    mo::windows_tip::TerminationFrame frame{};
    HMODULE module = nullptr;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
        | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        reinterpret_cast<LPCWSTR>(address), &module)) { return frame; }
    const wchar_t* names[] = {L"msctf.dll", L"user32.dll", L"ntdll.dll", L"combase.dll",
        L"imm32.dll", L"win32u.dll", L"kernelbase.dll", L"kernel32.dll", L"mo_tip_abi_probe.exe",
        L"textinputframework.dll", L"msctfmonitor.dll", L"msutb.dll", L"ole32.dll",
        L"rpcrt4.dll", L"ucrtbase.dll", L"vcruntime140.dll", L"vcruntime140_1.dll"};
    if (module == g_module) { frame.module = 1; }
    else {
        for (DWORD i = 0; i < ARRAYSIZE(names); ++i) {
            if (module == GetModuleHandleW(names[i])) { frame.module = i + 2; break; }
        }
    }
    // Unknown modules have no exported address or path metadata.
    if (frame.module != 0) {
        frame.rva = static_cast<std::uint64_t>(reinterpret_cast<std::uintptr_t>(address)
            - reinterpret_cast<std::uintptr_t>(module));
    }
    return frame;
}

class MetadataDuration final {
public:
    explicit MetadataDuration(std::uint64_t* output) noexcept : output_(output) {}
    ~MetadataDuration() noexcept {
        *output_ = static_cast<std::uint64_t>(std::chrono::duration_cast<std::chrono::microseconds>(
            std::chrono::steady_clock::now() - started_).count());
    }
private:
    std::chrono::steady_clock::time_point started_ = std::chrono::steady_clock::now();
    std::uint64_t* output_;
};
#endif

bool PathComponentEquals(
    const std::filesystem::path& component,
    const wchar_t* expected) noexcept {
    return _wcsicmp(component.c_str(), expected) == 0;
}

std::wstring ExpectedBrokerPath() {
    std::array<wchar_t, 32768> module_path{};
    const DWORD length = GetModuleFileNameW(
        g_module,
        module_path.data(),
        static_cast<DWORD>(module_path.size()));
    if (length == 0 || length >= module_path.size()) {
        return {};
    }

    const std::filesystem::path module(module_path.data());
    const std::filesystem::path directory = module.parent_path();
    const auto is_known_architecture = [](const std::filesystem::path& component) noexcept {
        return PathComponentEquals(component, L"x64")
            || PathComponentEquals(component, L"x86")
            || PathComponentEquals(component, L"Win32");
    };

    // Installed layout: <root>\tip\<architecture>\mo-tip.dll and
    // <root>\bin\mo-broker.exe.
    const std::filesystem::path tip_directory = directory.parent_path();
    if (is_known_architecture(directory.filename())
        && PathComponentEquals(tip_directory.filename(), L"tip")) {
        return (tip_directory.parent_path() / L"bin" / L"mo-broker.exe").wstring();
    }

    // Repository-only layout used by the native probes. This exact component
    // check prevents a shipping DLL from honoring an environment or cwd-based
    // development override.
    const std::filesystem::path msbuild_directory = directory.parent_path().parent_path();
    const std::filesystem::path out_directory = msbuild_directory.parent_path();
    const std::filesystem::path windows_tip_directory = out_directory.parent_path();
    const std::filesystem::path native_directory = windows_tip_directory.parent_path();
    if (is_known_architecture(directory.parent_path().filename())
        && PathComponentEquals(directory.filename(), L"Release")
        && PathComponentEquals(msbuild_directory.filename(), L"msbuild")
        && PathComponentEquals(out_directory.filename(), L"out")
        && PathComponentEquals(windows_tip_directory.filename(), L"windows-tip")
        && PathComponentEquals(native_directory.filename(), L"native")) {
        return (native_directory.parent_path() / L"target" / L"debug" / L"mo-broker.exe")
            .wstring();
    }
    return {};
}

bool IsKeyDown(int virtual_key) noexcept {
    return (GetKeyState(virtual_key) & 0x8000) != 0;
}

std::uint16_t CurrentModifiers() noexcept {
    std::uint16_t modifiers = 0;
    if (IsKeyDown(VK_SHIFT)) {
        modifiers |= kShiftModifier;
    }
    if (IsKeyDown(VK_CONTROL)) {
        modifiers |= kControlModifier;
    }
    if (IsKeyDown(VK_MENU)) {
        modifiers |= kAltModifier;
    }
    if (IsKeyDown(VK_LWIN) || IsKeyDown(VK_RWIN)) {
        modifiers |= kSuperModifier;
    }
    if ((GetKeyState(VK_CAPITAL) & 1) != 0) {
        modifiers |= kCapsLockModifier;
    }
    return modifiers;
}

bool Utf8ToUtf16(const std::string& input, std::wstring* output) noexcept {
    if (output == nullptr) {
        return false;
    }
    output->clear();
    if (input.empty()) {
        return true;
    }
    if (input.size() > static_cast<std::size_t>(std::numeric_limits<int>::max())) {
        return false;
    }
    const int input_length = static_cast<int>(input.size());
    const int required = MultiByteToWideChar(
        CP_UTF8,
        MB_ERR_INVALID_CHARS,
        input.data(),
        input_length,
        nullptr,
        0);
    if (required <= 0) {
        return false;
    }
    try {
        output->resize(static_cast<std::size_t>(required));
    } catch (...) {
        return false;
    }
    return MultiByteToWideChar(
               CP_UTF8,
               MB_ERR_INVALID_CHARS,
               input.data(),
               input_length,
               output->data(),
               required)
        == required;
}

class TextService final
    : public ITfTextInputProcessorEx,
      public ITfKeyEventSink,
      public ITfCompositionSink,
      public ITfTextLayoutSink
#ifdef MO_LATENCY_TRACE
      , public mo::windows_tip::IBrokerDiagnostics
#endif
      {
public:
    explicit TextService(std::wstring expected_broker_path) noexcept
        : broker_(std::move(expected_broker_path)) {
        InterlockedIncrement(&g_live_objects);
    }

    TextService(const TextService&) = delete;
    TextService& operator=(const TextService&) = delete;

    STDMETHODIMP QueryInterface(REFIID interface_id, void** object) noexcept override {
        if (object == nullptr) {
            return E_POINTER;
        }
        *object = nullptr;
        if (IsEqualIID(interface_id, IID_IUnknown)
            || IsEqualIID(interface_id, IID_ITfTextInputProcessor)
            || IsEqualIID(interface_id, IID_ITfTextInputProcessorEx)) {
            *object = static_cast<ITfTextInputProcessorEx*>(this);
            AddRef();
            return S_OK;
        }
        if (IsEqualIID(interface_id, IID_ITfKeyEventSink)) {
            *object = static_cast<ITfKeyEventSink*>(this);
            AddRef();
            return S_OK;
        }
        if (IsEqualIID(interface_id, IID_ITfCompositionSink)) {
            *object = static_cast<ITfCompositionSink*>(this);
            AddRef();
            return S_OK;
        }
        if (IsEqualIID(interface_id, IID_ITfTextLayoutSink)) {
            *object = static_cast<ITfTextLayoutSink*>(this); AddRef(); return S_OK;
        }
#ifdef MO_LATENCY_TRACE
        if (IsEqualIID(interface_id, __uuidof(mo::windows_tip::IBrokerDiagnostics))) {
            *object = static_cast<mo::windows_tip::IBrokerDiagnostics*>(this); AddRef(); return S_OK;
        }
#endif
        return E_NOINTERFACE;
    }
#ifdef MO_LATENCY_TRACE
    STDMETHODIMP ReadLastTiming(mo::windows_tip::BrokerTiming* timing) noexcept override {
        if (timing == nullptr) { return E_POINTER; }
        *timing = broker_.last_timing();
        timing->candidate_stage = candidate_trace_stage_;
        timing->candidate_result = candidate_trace_result_;
        timing->candidate_count = display_snapshot_.has_value()
            ? static_cast<DWORD>(display_snapshot_->candidates.size()) : 0;
        timing->candidate_focus = has_focus_ ? 1 : 0;
        timing->candidate_snapshot = display_snapshot_.has_value() ? 1 : 0;
        timing->candidate_reset = candidate_reset_;
        timing->candidate_reset_count = candidate_reset_count_;
        timing->edit_request = last_edit_request_;
        timing->edit_session = last_edit_session_;
        timing->termination_owner_active = termination_owner_active_;
        timing->termination_sent = termination_sent_;
        timing->termination_owner_foreground = termination_owner_foreground_;
        timing->dispatch_total_us = dispatch_total_us_;
        timing->dispatch_pre_send_us = dispatch_pre_send_us_;
        timing->dispatch_connect_us = dispatch_connect_us_;
        timing->dispatch_modifiers_us = dispatch_modifiers_us_;
        timing->termination_notification_count = termination_notification_count_;
        timing->termination_frame_count = termination_frame_count_;
        for (DWORD i = 0; i < termination_frame_count_; ++i) {
            timing->termination_frames[i] = DescribeTerminationFrame(termination_frames_[i]);
        }
        return S_OK;
    }
#endif

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

    STDMETHODIMP Activate(ITfThreadMgr* thread_manager, TfClientId client_id) noexcept override {
        return ActivateEx(thread_manager, client_id, 0);
    }

    STDMETHODIMP ActivateEx(
        ITfThreadMgr* thread_manager,
        TfClientId client_id,
        DWORD flags) noexcept override {
        if (thread_manager == nullptr) {
            return E_INVALIDARG;
        }
        if (thread_manager_ != nullptr) {
            return TF_E_ALREADY_EXISTS;
        }

        ITfKeystrokeMgr* keystroke_manager = nullptr;
        HRESULT result = thread_manager->QueryInterface(
            IID_ITfKeystrokeMgr,
            reinterpret_cast<void**>(&keystroke_manager));
        if (FAILED(result)) {
            return result;
        }
        result = keystroke_manager->AdviseKeyEventSink(client_id, this, TRUE);
        if (FAILED(result)) {
            keystroke_manager->Release();
            return result;
        }

        thread_manager->AddRef();
        thread_manager_ = thread_manager;
        keystroke_manager_ = keystroke_manager;
        client_id_ = client_id;
        activation_flags_ = flags;
        // Broker absence must never prevent TSF activation. The first bounded
        // connection attempt is best-effort; key callbacks remain fail-open.
        if (!broker_.ConnectAndOpen(kBrokerActivationTimeoutMs)) {
            next_reconnect_tick_ = GetTickCount64() + kBrokerReconnectBackoffMs;
        } else { broker_was_connected_ = true; }
        return S_OK;
    }

    STDMETHODIMP Deactivate() noexcept override {
        has_focus_ = false;
        cached_key_.valid = false;
        candidate_window_.Destroy();
        latest_revision_ = 0;
        CancelActiveComposition();
        HRESULT result = S_OK;
        if (keystroke_manager_ != nullptr) {
            result = keystroke_manager_->UnadviseKeyEventSink(client_id_);
            keystroke_manager_->Release();
            keystroke_manager_ = nullptr;
        }
        broker_.Close(kBrokerActivationTimeoutMs);
        if (thread_manager_ != nullptr) {
            thread_manager_->Release();
            thread_manager_ = nullptr;
        }
        client_id_ = TF_CLIENTID_NULL;
        activation_flags_ = 0;
        has_focus_ = false;
        next_reconnect_tick_ = 0;
        broker_was_connected_ = false;
        cached_key_.valid = false;
        return result;
    }

    STDMETHODIMP OnSetFocus(BOOL foreground) noexcept override {
        has_focus_ = foreground != FALSE;
        cached_key_.valid = false;
        if (!has_focus_) {
            candidate_window_.Hide();
            latest_revision_ = 0;
            CancelActiveComposition();
            broker_.Close(kBrokerActivationTimeoutMs);
        } else {
            next_reconnect_tick_ = 0;
            EnsureBrokerConnected(kBrokerActivationTimeoutMs);
        }
        return S_OK;
    }

    STDMETHODIMP OnTestKeyDown(
        ITfContext* context,
        WPARAM virtual_key,
        LPARAM key_data,
        BOOL* eaten) noexcept override {
        return TestKey(context, virtual_key, key_data, true, eaten);
    }

    STDMETHODIMP OnTestKeyUp(
        ITfContext* context,
        WPARAM virtual_key,
        LPARAM key_data,
        BOOL* eaten) noexcept override {
        return TestKey(context, virtual_key, key_data, false, eaten);
    }

    STDMETHODIMP OnKeyDown(
        ITfContext* context,
        WPARAM virtual_key,
        LPARAM key_data,
        BOOL* eaten) noexcept override {
        return HandleKey(context, virtual_key, key_data, true, eaten);
    }

    STDMETHODIMP OnKeyUp(
        ITfContext* context,
        WPARAM virtual_key,
        LPARAM key_data,
        BOOL* eaten) noexcept override {
        return HandleKey(context, virtual_key, key_data, false, eaten);
    }

    STDMETHODIMP OnPreservedKey(
        ITfContext* context,
        REFGUID,
        BOOL* eaten) noexcept override {
        return ValidateKeyArguments(context, eaten);
    }

    STDMETHODIMP OnCompositionTerminated(
        TfEditCookie edit_cookie,
        ITfComposition* composition) noexcept override {
        if (composition == nullptr) {
            return E_INVALIDARG;
        }
        if (composition_.Get() == composition) {
            if (handling_termination_) { return S_OK; }
            handling_termination_ = true;
            struct TerminationScope final {
                bool* flag;
                ~TerminationScope() noexcept { *flag = false; }
            } termination_scope{&handling_termination_};
            // Pin ownership before Win32 or text-store calls can reenter.
            ComPtr<ITfRange> terminated_range = active_range_;
            const auto still_owned = [this, composition, &terminated_range]() noexcept {
                return composition_.Get() == composition && active_range_.Get() == terminated_range.Get();
            };
#ifdef MO_LATENCY_TRACE
            ++termination_notification_count_;
            termination_frame_count_ = CaptureStackBackTrace(0,
                mo::windows_tip::kTerminationFrameCapacity, termination_frames_.data(), nullptr);
            termination_owner_active_ = candidate_owner_ != nullptr
                && GetActiveWindow() == candidate_owner_ ? 1 : 0;
            termination_sent_ = InSendMessageEx(nullptr);
            termination_owner_foreground_ = candidate_owner_ != nullptr
                && GetForegroundWindow() == candidate_owner_ ? 1 : 0;
#endif
            // TSF grants the terminating owner a write cookie. Discard only
            // our live, uncommitted range under that existing lock; dropping
            // range ownership first leaves literal preedit in the document.
            // Do not request a nested lock, call EndComposition again, commit
            // an uncertain engine candidate, or replay input on reconnect.
            InvalidateCandidateIdentity();
            cached_key_.valid = false;
            candidate_window_.Hide();
            if (!still_owned()) { return S_OK; }
            const HRESULT discarded = terminated_range != nullptr
                ? terminated_range->SetText(edit_cookie, 0, L"", 0) : S_OK;
            if (still_owned()) {
                ResetCompositionState(CandidateReset::HostTermination);
                DisconnectBroker(0, CandidateReset::HostTermination);
                TraceCandidate(12, discarded);
            }
            return discarded;
        }
        return S_OK;
    }

    STDMETHODIMP OnLayoutChange(ITfContext* context, TfLayoutCode code,
        ITfContextView*) noexcept override {
        if (context == nullptr) { return E_INVALIDARG; }
        if (context != composition_context_.Get()) { return S_OK; }
        InvalidateCandidateIdentity();
        candidate_window_.Hide();
        has_candidate_anchor_ = false;
        if (code == TF_LC_DESTROY) {
            ResetCompositionState(CandidateReset::LayoutDestroy);
            DisconnectBroker(kBrokerKeyTimeoutMs, CandidateReset::LayoutDestroy);
            TraceCandidate(13, S_OK);
        } else if (code == TF_LC_CHANGE) {
            QueueCandidateLayout(context);
        }
        return S_OK;
    }

private:
    struct CandidateIdentity final {
        ComPtr<ITfContext> context;
        ComPtr<ITfRange> range;
        std::uint64_t epoch, revision, generation, token;
    };

    CandidateIdentity CaptureCandidateIdentity(std::uint64_t revision) const noexcept {
        return {composition_context_, active_range_, candidate_epoch_, revision,
            broker_.generation(), broker_.session_token()};
    }

    bool CandidateIdentityMatches(const CandidateIdentity& identity) const noexcept {
        return !handling_termination_ && has_focus_ && broker_.connected()
            && identity.epoch != std::numeric_limits<std::uint64_t>::max()
            && identity.epoch == candidate_epoch_
            && identity.context != nullptr && identity.range != nullptr
            && identity.context.Get() == composition_context_.Get()
            && identity.range.Get() == active_range_.Get()
            && identity.revision == latest_revision_
            && identity.generation == broker_.generation() && identity.token == broker_.session_token()
            && display_snapshot_.has_value() && display_snapshot_->revision == identity.revision;
    }

    void InvalidateCandidateIdentity() noexcept {
        // Saturate instead of wrapping and accepting an ancient identity.
        if (candidate_epoch_ != std::numeric_limits<std::uint64_t>::max()) { ++candidate_epoch_; }
    }

    bool CheckCandidateIdentity(const CandidateIdentity& identity) noexcept {
        if (CandidateIdentityMatches(identity)) { return true; }
        has_candidate_anchor_ = false;
        candidate_window_.Hide();
        TraceCandidate(14, TF_E_DISCONNECTED);
        return false;
    }

    class CandidateLayoutEditSession final : public ITfEditSession {
    public:
        CandidateLayoutEditSession(TextService* owner, ITfContext* context, std::uint64_t request) noexcept
            : owner_(owner), context_(context), revision_(owner->latest_revision_),
              generation_(owner->broker_.generation()), token_(owner->broker_.session_token()), request_(request) {
            owner_->AddRef(); context_->AddRef();
        }
        STDMETHODIMP QueryInterface(REFIID id, void** object) noexcept override {
            if (object == nullptr) { return E_POINTER; }
            *object = nullptr;
            if (IsEqualIID(id, IID_IUnknown) || IsEqualIID(id, IID_ITfEditSession)) {
                *object = static_cast<ITfEditSession*>(this); AddRef(); return S_OK;
            }
            return E_NOINTERFACE;
        }
        STDMETHODIMP_(ULONG) AddRef() noexcept override {
            return static_cast<ULONG>(InterlockedIncrement(&references_));
        }
        STDMETHODIMP_(ULONG) Release() noexcept override {
            const LONG count = InterlockedDecrement(&references_);
            if (count == 0) { delete this; return 0; }
            return static_cast<ULONG>(count);
        }
        STDMETHODIMP DoEditSession(TfEditCookie cookie) noexcept override {
            if (owner_->pending_layout_request_ != request_) { return S_OK; }
            owner_->pending_layout_request_ = 0;
            if (owner_->has_focus_ && context_ == owner_->composition_context_.Get()
                && revision_ == owner_->latest_revision_
                && generation_ == owner_->broker_.generation() && token_ == owner_->broker_.session_token()
                && owner_->display_snapshot_.has_value()) {
                try {
                    const auto snapshot = owner_->display_snapshot_.value();
                    owner_->RefreshCandidateAnchor(context_, cookie, snapshot);
                } catch (...) { owner_->candidate_window_.Hide(); }
            } else if (owner_->has_focus_ && owner_->composition_context_ != nullptr
                && owner_->display_snapshot_.has_value()) {
                // A newer key/context superseded the queued read. Recompute
                // against the current identity instead of reviving the old UI.
                owner_->QueueCandidateLayout(owner_->composition_context_.Get());
            }
            return S_OK;
        }
    private:
        ~CandidateLayoutEditSession() noexcept { context_->Release(); owner_->Release(); }
        volatile LONG references_ = 1;
        TextService* owner_;
        ITfContext* context_;
        std::uint64_t revision_;
        std::uint64_t generation_;
        std::uint64_t token_;
        std::uint64_t request_;
    };

    // Mouse callbacks are not documented synchronous-write entry points.
    // Queue an owned action, not an engine commit or a borrowed key snapshot.
    // Revalidate identity and revision inside the edit lock before dispatch.
    class CandidateEditSession final : public ITfEditSession {
    public:
        CandidateEditSession(TextService* owner, ITfContext* context,
            std::uint64_t revision, mo::windows_tip::CandidateAction action,
            std::uint32_t index) noexcept
            : owner_(owner), context_(context), revision_(revision), action_(action), index_(index),
              generation_(owner->broker_.generation()), token_(owner->broker_.session_token()) {
            owner_->AddRef();
            context_->AddRef();
        }
        STDMETHODIMP QueryInterface(REFIID id, void** object) noexcept override {
            if (object == nullptr) { return E_POINTER; }
            *object = nullptr;
            if (IsEqualIID(id, IID_IUnknown) || IsEqualIID(id, IID_ITfEditSession)) {
                *object = static_cast<ITfEditSession*>(this); AddRef(); return S_OK;
            }
            return E_NOINTERFACE;
        }
        STDMETHODIMP_(ULONG) AddRef() noexcept override {
            return static_cast<ULONG>(InterlockedIncrement(&references_));
        }
        STDMETHODIMP_(ULONG) Release() noexcept override {
            const LONG count = InterlockedDecrement(&references_);
            if (count == 0) { delete this; return 0; }
            return static_cast<ULONG>(count);
        }
        STDMETHODIMP DoEditSession(TfEditCookie cookie) noexcept override {
            return owner_->ApplyCandidateAction(context_, cookie, generation_, token_, revision_, action_, index_);
        }
    private:
        ~CandidateEditSession() noexcept { context_->Release(); owner_->Release(); }
        volatile LONG references_ = 1;
        TextService* owner_;
        ITfContext* context_;
        std::uint64_t revision_;
        mo::windows_tip::CandidateAction action_;
        std::uint32_t index_;
        std::uint64_t generation_;
        std::uint64_t token_;
    };

    class SnapshotEditSession final : public ITfEditSession {
    public:
        SnapshotEditSession(
            TextService* owner,
            ITfContext* context,
            const mo::windows_tip::BrokerSnapshot* snapshot) noexcept
            : owner_(owner), context_(context), snapshot_(snapshot) {
            owner_->AddRef();
            context_->AddRef();
        }

        SnapshotEditSession(const SnapshotEditSession&) = delete;
        SnapshotEditSession& operator=(const SnapshotEditSession&) = delete;

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
            if (snapshot_ == nullptr) {
                return owner_->ClearComposition(context_, edit_cookie, &applied_);
            }
            return owner_->ApplySnapshot(context_, edit_cookie, *snapshot_, &applied_);
        }

        bool applied() const noexcept { return applied_; }

    private:
        ~SnapshotEditSession() noexcept {
            context_->Release();
            owner_->Release();
        }

        volatile LONG reference_count_ = 1;
        TextService* owner_;
        ITfContext* context_;
        const mo::windows_tip::BrokerSnapshot* snapshot_;
        bool applied_ = false;
    };

    struct CachedKey final {
        bool valid = false;
        ITfContext* context = nullptr;
        WPARAM virtual_key = 0;
        LPARAM key_data = 0;
        bool key_down = false;
        mo::windows_tip::BrokerSnapshot snapshot;
    };

    ~TextService() noexcept {
        Deactivate();
        composition_.Reset();
        active_range_.Reset();
        composition_context_.Reset();
        InterlockedDecrement(&g_live_objects);
    }

    static HRESULT ValidateKeyArguments(ITfContext* context, BOOL* eaten) noexcept {
        if (eaten == nullptr) {
            return E_POINTER;
        }
        *eaten = FALSE;
        return context == nullptr ? E_INVALIDARG : S_OK;
    }

    static UINT ScanCode(LPARAM key_data) noexcept {
        return static_cast<UINT>((static_cast<ULONG_PTR>(key_data) >> 16U) & 0xffU);
    }

    static bool IsRepeat(LPARAM key_data) noexcept {
        return (static_cast<ULONG_PTR>(key_data) & (ULONG_PTR{1} << 30U)) != 0;
    }

    bool EnsureBrokerConnected(DWORD timeout_ms) noexcept {
        return EnsureBrokerConnectedUntil(mo::windows_tip::DeadlineFromNow(timeout_ms));
    }

    bool EnsureBrokerConnectedUntil(mo::windows_tip::Deadline deadline) noexcept {
        if (broker_.connected()) {
            return true;
        }
        const ULONGLONG now = GetTickCount64();
        if (now < next_reconnect_tick_) {
            return false;
        }
        if (broker_.ConnectAndOpenUntil(deadline, broker_was_connected_)) {
            broker_was_connected_ = true;
            next_reconnect_tick_ = 0;
            return true;
        }
        next_reconnect_tick_ = now + kBrokerReconnectBackoffMs;
        return false;
    }

    void DisconnectBroker(DWORD timeout_ms, CandidateReset cause = CandidateReset::Transport) noexcept {
        InvalidateCandidateIdentity();
        TraceReset(cause);
        candidate_window_.Hide();
        latest_revision_ = 0;
        display_snapshot_.reset();
        pending_layout_request_ = 0;
        broker_.Close(timeout_ms);
        next_reconnect_tick_ = GetTickCount64() + kBrokerReconnectBackoffMs;
    }

    bool CachedKeyMatches(
        ITfContext* context,
        WPARAM virtual_key,
        LPARAM key_data,
        bool key_down) const noexcept {
        return cached_key_.valid
            && cached_key_.context == context
            && cached_key_.virtual_key == virtual_key
            && cached_key_.key_data == key_data
            && cached_key_.key_down == key_down;
    }

    bool DispatchKey(
        ITfContext* context,
        WPARAM virtual_key,
        LPARAM key_data,
        bool key_down) noexcept {
#ifdef MO_LATENCY_TRACE
        dispatch_pre_send_us_ = dispatch_connect_us_ = dispatch_modifiers_us_ = 0;
        const auto dispatch_started = std::chrono::steady_clock::now();
        const MetadataDuration dispatch_duration(&dispatch_total_us_);
#endif
        const auto deadline = mo::windows_tip::DeadlineFromNow(kBrokerKeyTimeoutMs);
        cached_key_.valid = false;
        if (composition_context_ != nullptr && composition_context_.Get() != context) {
            candidate_window_.Hide();
            CancelActiveComposition();
            // Dropping the old connection reclaims the server session. Do not
            // add another round-trip budget before reconnecting for this key.
            broker_.Close(0);
            latest_revision_ = 0;
        }
        bool ready = false;
        {
#ifdef MO_LATENCY_TRACE
            const MetadataDuration connect_duration(&dispatch_connect_us_);
#endif
            ready = has_focus_ && EnsureBrokerConnectedUntil(deadline);
        }
        if (!ready) { return false; }
        std::uint16_t modifiers = 0;
        {
#ifdef MO_LATENCY_TRACE
            const MetadataDuration modifiers_duration(&dispatch_modifiers_us_);
#endif
            modifiers = CurrentModifiers();
        }
#ifdef MO_LATENCY_TRACE
        dispatch_pre_send_us_ = static_cast<std::uint64_t>(std::chrono::duration_cast<std::chrono::microseconds>(
            std::chrono::steady_clock::now() - dispatch_started).count());
#endif
        if (!broker_.SendKeyUntil(
                static_cast<UINT>(virtual_key),
                ScanCode(key_data),
                modifiers,
                key_down,
                IsRepeat(key_data),
                &cached_key_.snapshot,
                deadline)) {
            DisconnectBroker(0);
            return false;
        }
        latest_revision_ = cached_key_.snapshot.revision;
        StoreCandidateSnapshot(cached_key_.snapshot);
        if (cached_key_.snapshot.handled) {
            candidate_window_.Hide();
        } else if (composition_context_.Get() == context && has_candidate_anchor_) {
            // Unconsumed key-up/modifier events still carry a new authoritative
            // page revision. Refresh without borrowing an expired edit cookie.
            ShowCandidateWindow(cached_key_.snapshot);
        }
        cached_key_.context = context;
        cached_key_.virtual_key = virtual_key;
        cached_key_.key_data = key_data;
        cached_key_.key_down = key_down;
        cached_key_.valid = true;
        return true;
    }

    HRESULT TestKey(
        ITfContext* context,
        WPARAM virtual_key,
        LPARAM key_data,
        bool key_down,
        BOOL* eaten) noexcept {
        const HRESULT result = ValidateKeyArguments(context, eaten);
        if (FAILED(result)) {
            return result;
        }
        if (handling_termination_) { return S_OK; }
        if (!CachedKeyMatches(context, virtual_key, key_data, key_down)
            && !DispatchKey(context, virtual_key, key_data, key_down)) {
            return S_OK;
        }
        *eaten = cached_key_.snapshot.handled ? TRUE : FALSE;
        return S_OK;
    }

    HRESULT HandleKey(
        ITfContext* context,
        WPARAM virtual_key,
        LPARAM key_data,
        bool key_down,
        BOOL* eaten) noexcept {
        const HRESULT result = ValidateKeyArguments(context, eaten);
        if (FAILED(result)) {
            return result;
        }
        if (handling_termination_) { return S_OK; }
        if (!CachedKeyMatches(context, virtual_key, key_data, key_down)
            && !DispatchKey(context, virtual_key, key_data, key_down)) {
            return S_OK;
        }
        if (!cached_key_.snapshot.handled) {
            cached_key_.valid = false;
            return S_OK;
        }

        auto* edit_session = new (std::nothrow)
            SnapshotEditSession(this, context, &cached_key_.snapshot);
        if (edit_session == nullptr) {
            cached_key_.valid = false;
            DisconnectBroker(0);
            return S_OK;
        }
        HRESULT session_result = E_FAIL;
        const HRESULT request_result = context->RequestEditSession(
            client_id_,
            edit_session,
            TF_ES_SYNC | TF_ES_READWRITE,
            &session_result);
        const bool applied = edit_session->applied();
#ifdef MO_LATENCY_TRACE
        last_edit_request_ = request_result;
        last_edit_session_ = session_result;
#endif
        edit_session->Release();
        cached_key_.valid = false;
        if (applied) {
            *eaten = TRUE;
        }
        if (FAILED(request_result) || FAILED(session_result)) {
            DisconnectBroker(0, CandidateReset::KeyEditFailure);
        }
        return S_OK;
    }

    HRESULT ApplySnapshot(
        ITfContext* context,
        TfEditCookie edit_cookie,
        const mo::windows_tip::BrokerSnapshot& snapshot,
        bool* applied) noexcept {
        *applied = false;
        std::wstring text;
        const std::string& utf8 = snapshot.commit.has_value()
            ? snapshot.commit.value()
            : snapshot.composition;
        if (!Utf8ToUtf16(utf8, &text)
            || text.size() > static_cast<std::size_t>(std::numeric_limits<LONG>::max())) {
            return E_INVALIDARG;
        }

        HRESULT result;
        if (snapshot.commit.has_value()) {
            result = CommitText(context, edit_cookie, text, applied);
            if (SUCCEEDED(result) && !snapshot.composition.empty()) {
                // Some schemas commit one segment while retaining a new
                // preedit. The committed prefix must not be lost or duplicated.
                if (!Utf8ToUtf16(snapshot.composition, &text)) { return E_INVALIDARG; }
                bool preedit_applied = false;
                result = UpdateComposition(context, edit_cookie, text, &preedit_applied);
            }
        } else if (text.empty()) {
            result = ClearComposition(context, edit_cookie, applied);
        } else {
            result = UpdateComposition(context, edit_cookie, text, applied);
        }
        if (SUCCEEDED(result)) {
            StoreCandidateSnapshot(snapshot);
            RefreshCandidateAnchor(context, edit_cookie, snapshot);
        }
        else { candidate_window_.Hide(); }
        return result;
    }

    void ShowCandidateWindow(const mo::windows_tip::BrokerSnapshot& snapshot) noexcept {
        if (!has_focus_ || !has_candidate_anchor_
            || (activation_flags_ & TF_TMAE_UIELEMENTENABLEDONLY) != 0
            || !broker_.candidate_actions_supported()) {
            TraceCandidate(9, S_FALSE);
            candidate_window_.Hide();
            return;
        }
        const auto identity = CaptureCandidateIdentity(snapshot.revision);
        if (!CheckCandidateIdentity(identity)) { return; }
        const bool shown = candidate_window_.Update(g_module, candidate_owner_, candidate_anchor_, snapshot,
            CandidateActionCallback, this);
        // Win32 show/owner/capture calls may synchronously reenter the host.
        if (!CheckCandidateIdentity(identity)) { return; }
        TraceCandidate(shown ? 11 : 10, shown ? S_OK : E_FAIL);
    }

    void TraceCandidate(DWORD stage, HRESULT result) noexcept {
#ifdef MO_LATENCY_TRACE
        candidate_trace_stage_ = stage; candidate_trace_result_ = result;
#else
        (void)stage; (void)result;
#endif
    }

    void TraceReset(CandidateReset cause) noexcept {
#ifdef MO_LATENCY_TRACE
        candidate_reset_ = static_cast<DWORD>(cause);
        if (candidate_reset_count_ != std::numeric_limits<std::uint64_t>::max()) { ++candidate_reset_count_; }
#else
        (void)cause;
#endif
    }

    void StoreCandidateSnapshot(const mo::windows_tip::BrokerSnapshot& snapshot) noexcept {
        try { display_snapshot_ = snapshot; }
        catch (...) { TraceReset(CandidateReset::SnapshotAllocation); display_snapshot_.reset(); candidate_window_.Hide(); }
    }

    void QueueCandidateLayout(ITfContext* context) noexcept {
        if (pending_layout_request_ != 0 || !has_focus_ || !display_snapshot_.has_value()
            || active_range_ == nullptr || layout_request_id_ == std::numeric_limits<std::uint64_t>::max()) { return; }
        const auto request = ++layout_request_id_;
        auto* session = new (std::nothrow) CandidateLayoutEditSession(this, context, request);
        if (session == nullptr) { return; }
        pending_layout_request_ = request;
        HRESULT session_result = E_FAIL;
        const HRESULT result = context->RequestEditSession(client_id_, session,
            TF_ES_ASYNC | TF_ES_READ, &session_result);
        session->Release();
        if ((FAILED(result) || FAILED(session_result)) && pending_layout_request_ == request) {
            pending_layout_request_ = 0;
        }
    }

    void AdviseCandidateLayout(ITfContext* context) noexcept {
        if (layout_source_ != nullptr) { return; }
        ComPtr<ITfSource> source;
        DWORD cookie = TF_INVALID_COOKIE;
        if (SUCCEEDED(context->QueryInterface(IID_PPV_ARGS(source.GetAddressOf())))
            && SUCCEEDED(source->AdviseSink(IID_ITfTextLayoutSink,
                static_cast<ITfTextLayoutSink*>(this), &cookie))) {
            layout_source_ = source;
            layout_cookie_ = cookie;
        }
    }

    void ResetCompositionState(CandidateReset cause) noexcept {
        InvalidateCandidateIdentity();
        TraceReset(cause);
        composition_.Reset();
        active_range_.Reset();
        composition_context_.Reset();
        has_candidate_anchor_ = false;
        display_snapshot_.reset();
        pending_layout_request_ = 0;
        ComPtr<ITfSource> source = layout_source_;
        const DWORD cookie = layout_cookie_;
        layout_source_.Reset();
        layout_cookie_ = TF_INVALID_COOKIE;
        if (source != nullptr && cookie != TF_INVALID_COOKIE) { source->UnadviseSink(cookie); }
    }

    void RefreshCandidateAnchor(ITfContext* context, TfEditCookie cookie,
        const mo::windows_tip::BrokerSnapshot& snapshot) noexcept {
        has_candidate_anchor_ = false;
        if (active_range_ == nullptr || composition_context_.Get() != context) {
            TraceCandidate(1, TF_E_DISCONNECTED);
            candidate_window_.Hide(); return;
        }
        const auto identity = CaptureCandidateIdentity(snapshot.revision);
        if (!CheckCandidateIdentity(identity)) { return; }
        ComPtr<ITfContextView> view;
        ComPtr<ITfRange> caret;
        BOOL clipped = TRUE;
        HWND owner = nullptr;
        RECT anchor{};
        const auto checked = [this](DWORD stage, HRESULT result) noexcept {
            if (SUCCEEDED(result)) { return true; }
            TraceCandidate(stage, result); candidate_window_.Hide(); return false;
        };
        if (!checked(2, context->GetActiveView(view.GetAddressOf()))) { return; }
        if (view == nullptr) { TraceCandidate(2, E_UNEXPECTED); candidate_window_.Hide(); return; }
        // Keep range/context alive across host COM calls, which may clear the
        // service's members. Never dereference a member after reentrant reset.
        if (!checked(3, identity.range->Clone(caret.GetAddressOf()))
            || !checked(4, caret->Collapse(cookie, TF_ANCHOR_START))
            || !checked(5, view->GetTextExt(cookie, caret.Get(), &anchor, &clipped))) { return; }
        if (!CheckCandidateIdentity(identity)) { return; }
        if (clipped || anchor.bottom <= anchor.top || anchor.right < anchor.left) {
            TraceCandidate(clipped ? 6 : 7, S_FALSE); candidate_window_.Hide(); return;
        }
        if (!checked(8, view->GetWnd(&owner))) { return; }
        if (!CheckCandidateIdentity(identity)) { return; }
        if (!IsWindow(owner)) { TraceCandidate(8, E_HANDLE); candidate_window_.Hide(); return; }
        candidate_anchor_ = anchor;
        candidate_owner_ = owner;
        has_candidate_anchor_ = true;
        ShowCandidateWindow(snapshot);
    }

    static void CandidateActionCallback(void* context, std::uint64_t revision,
        mo::windows_tip::CandidateAction action, std::uint32_t index) noexcept {
        auto* owner = static_cast<TextService*>(context);
        owner->AddRef();
        owner->QueueCandidateAction(revision, action, index);
        owner->Release();
    }

    void QueueCandidateAction(std::uint64_t revision, mo::windows_tip::CandidateAction action,
        std::uint32_t index) noexcept {
        if (handling_termination_ || !has_focus_ || !broker_.connected() || revision != latest_revision_
            || composition_context_ == nullptr || active_range_ == nullptr) { return; }
        auto* session = new (std::nothrow) CandidateEditSession(this, composition_context_.Get(), revision, action, index);
        if (session == nullptr) { return; }
        candidate_window_.Hide();
        HRESULT session_result = E_FAIL;
        const HRESULT result = composition_context_->RequestEditSession(client_id_, session,
            TF_ES_ASYNCDONTCARE | TF_ES_READWRITE, &session_result);
        session->Release();
        if (FAILED(result) || FAILED(session_result)) { DisconnectBroker(kBrokerKeyTimeoutMs, CandidateReset::MouseRequestFailure); }
    }

    HRESULT ApplyCandidateAction(ITfContext* context, TfEditCookie cookie,
        std::uint64_t generation, std::uint64_t token, std::uint64_t revision,
        mo::windows_tip::CandidateAction action, std::uint32_t index) noexcept {
        if (handling_termination_ || !has_focus_ || !broker_.connected() || generation != broker_.generation()
            || token != broker_.session_token() || revision != latest_revision_
            || composition_context_.Get() != context || active_range_ == nullptr) { return S_OK; }
        cached_key_.valid = false;
        mo::windows_tip::BrokerSnapshot snapshot;
        if (!broker_.SendCandidateAction(revision, action, index, &snapshot, kBrokerKeyTimeoutMs)) {
            // We already own the RW cookie. Discard ONLY our uncommitted range
            // here, never request a nested lock or return a transport failure
            // that lets TSF abandon the composition with literal preedit left
            // behind. The uncertain engine action is not replayed on reconnect.
            bool discarded = false;
            const HRESULT cleared = ClearComposition(context, cookie, &discarded);
            DisconnectBroker(kBrokerKeyTimeoutMs);
            return cleared;
        }
        latest_revision_ = snapshot.revision;
        bool applied = false;
        const HRESULT result = ApplySnapshot(context, cookie, snapshot, &applied);
        if (FAILED(result)) { DisconnectBroker(kBrokerKeyTimeoutMs, CandidateReset::MouseEditFailure); }
        return result;
    }

    HRESULT UpdateComposition(
        ITfContext* context,
        TfEditCookie edit_cookie,
        const std::wstring& text,
        bool* applied) noexcept {
        if (active_range_ != nullptr) {
            if (composition_context_.Get() != context) {
                return TF_E_DISCONNECTED;
            }
            const HRESULT result = active_range_->SetText(
                edit_cookie,
                0,
                text.data(),
                static_cast<LONG>(text.size()));
            if (FAILED(result)) {
                ResetCompositionState(CandidateReset::UpdateFailure);
            }
            *applied = SUCCEEDED(result);
            return result;
        }

        ComPtr<ITfInsertAtSelection> insertion;
        HRESULT result = context->QueryInterface(IID_PPV_ARGS(insertion.GetAddressOf()));
        if (FAILED(result)) {
            return result;
        }
        ComPtr<ITfContextComposition> context_composition;
        result = context->QueryInterface(IID_PPV_ARGS(context_composition.GetAddressOf()));
        if (FAILED(result)) {
            return result;
        }
        ComPtr<ITfRange> range;
        result = insertion->InsertTextAtSelection(
            edit_cookie,
            TF_IAS_NO_DEFAULT_COMPOSITION,
            text.data(),
            static_cast<LONG>(text.size()),
            range.GetAddressOf());
        if (FAILED(result)) {
            return result;
        }

        // Keep the range even if this host rejects a formal composition. It
        // provides a deterministic fallback for restricted or legacy text
        // stores without letting an already-handled key leak to the host.
        active_range_ = range;
        composition_context_ = context;
        AdviseCandidateLayout(context);
        *applied = true;

        ComPtr<ITfComposition> composition;
        result = context_composition->StartComposition(
            edit_cookie,
            range.Get(),
            static_cast<ITfCompositionSink*>(this),
            composition.GetAddressOf());
        if (SUCCEEDED(result) && composition != nullptr) {
            composition_ = composition;
        }
        return S_OK;
    }

    HRESULT CommitText(
        ITfContext* context,
        TfEditCookie edit_cookie,
        const std::wstring& text,
        bool* applied) noexcept {
        if (active_range_ == nullptr) {
            ComPtr<ITfInsertAtSelection> insertion;
            HRESULT result = context->QueryInterface(IID_PPV_ARGS(insertion.GetAddressOf()));
            if (FAILED(result)) {
                return result;
            }
            result = insertion->InsertTextAtSelection(
                edit_cookie,
                TF_IAS_NOQUERY,
                text.data(),
                static_cast<LONG>(text.size()),
                nullptr);
            *applied = SUCCEEDED(result);
            return result;
        }
        if (composition_context_.Get() != context) {
            return TF_E_DISCONNECTED;
        }
        HRESULT result = active_range_->SetText(
            edit_cookie,
            0,
            text.data(),
            static_cast<LONG>(text.size()));
        if (FAILED(result)) {
            ResetCompositionState(CandidateReset::CommitFailure);
            return result;
        }
        *applied = true;
        return EndOwnedComposition(edit_cookie);
    }

    HRESULT ClearComposition(
        ITfContext* context,
        TfEditCookie edit_cookie,
        bool* applied) noexcept {
        if (active_range_ == nullptr) {
            *applied = true;
            return S_OK;
        }
        if (composition_context_.Get() != context) {
            return TF_E_DISCONNECTED;
        }
        HRESULT result = active_range_->SetText(edit_cookie, 0, nullptr, 0);
        if (FAILED(result)) {
            ResetCompositionState(CandidateReset::ClearFailure);
            return result;
        }
        *applied = true;
        return EndOwnedComposition(edit_cookie);
    }

    HRESULT EndOwnedComposition(TfEditCookie edit_cookie) noexcept {
        ComPtr<ITfComposition> ending = composition_;
        ResetCompositionState(CandidateReset::Finished);
        return ending != nullptr ? ending->EndComposition(edit_cookie) : S_OK;
    }

    void CancelActiveComposition() noexcept {
        candidate_window_.Hide();
        has_candidate_anchor_ = false;
        if (active_range_ == nullptr || composition_context_ == nullptr) {
            ResetCompositionState(CandidateReset::Cancelled);
            return;
        }
        auto* edit_session = new (std::nothrow)
            SnapshotEditSession(this, composition_context_.Get(), nullptr);
        if (edit_session != nullptr) {
            HRESULT session_result = E_FAIL;
            composition_context_->RequestEditSession(
                client_id_,
                edit_session,
                TF_ES_SYNC | TF_ES_READWRITE,
                &session_result);
            edit_session->Release();
        }
        ResetCompositionState(CandidateReset::Cancelled);
    }

    volatile LONG reference_count_ = 1;
    ITfThreadMgr* thread_manager_ = nullptr;
    ITfKeystrokeMgr* keystroke_manager_ = nullptr;
    TfClientId client_id_ = TF_CLIENTID_NULL;
    DWORD activation_flags_ = 0;
    bool has_focus_ = false;
    bool handling_termination_ = false;
    bool broker_was_connected_ = false;
    ULONGLONG next_reconnect_tick_ = 0;
    mo::windows_tip::BrokerClient broker_;
    mo::windows_tip::CandidateWindow candidate_window_;
#ifdef MO_LATENCY_TRACE
    DWORD candidate_trace_stage_ = 0;
    HRESULT candidate_trace_result_ = S_OK;
    DWORD candidate_reset_ = 0;
    std::uint64_t candidate_reset_count_ = 0;
    HRESULT last_edit_request_ = S_OK, last_edit_session_ = S_OK;
    DWORD termination_owner_active_ = 0, termination_sent_ = 0, termination_owner_foreground_ = 0;
    std::uint64_t dispatch_total_us_ = 0, dispatch_pre_send_us_ = 0, dispatch_connect_us_ = 0, dispatch_modifiers_us_ = 0;
    std::uint64_t termination_notification_count_ = 0;
    DWORD termination_frame_count_ = 0;
    std::array<void*, mo::windows_tip::kTerminationFrameCapacity> termination_frames_{};
#endif
    std::uint64_t latest_revision_ = 0;
    std::uint64_t candidate_epoch_ = 0;
    RECT candidate_anchor_{};
    HWND candidate_owner_ = nullptr;
    bool has_candidate_anchor_ = false;
    std::optional<mo::windows_tip::BrokerSnapshot> display_snapshot_;
    ComPtr<ITfSource> layout_source_;
    DWORD layout_cookie_ = TF_INVALID_COOKIE;
    std::uint64_t layout_request_id_ = 0;
    std::uint64_t pending_layout_request_ = 0;
    CachedKey cached_key_;
    ComPtr<ITfComposition> composition_;
    ComPtr<ITfRange> active_range_;
    ComPtr<ITfContext> composition_context_;
};

class ClassFactory final : public IClassFactory {
public:
    ClassFactory() noexcept { InterlockedIncrement(&g_live_objects); }

    ClassFactory(const ClassFactory&) = delete;
    ClassFactory& operator=(const ClassFactory&) = delete;

    STDMETHODIMP QueryInterface(REFIID interface_id, void** object) noexcept override {
        if (object == nullptr) {
            return E_POINTER;
        }
        *object = nullptr;
        if (IsEqualIID(interface_id, IID_IUnknown)
            || IsEqualIID(interface_id, IID_IClassFactory)) {
            *object = static_cast<IClassFactory*>(this);
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

    STDMETHODIMP CreateInstance(
        IUnknown* outer,
        REFIID interface_id,
        void** object) noexcept override {
        if (object == nullptr) {
            return E_POINTER;
        }
        *object = nullptr;
        if (outer != nullptr) {
            return CLASS_E_NOAGGREGATION;
        }

        std::wstring expected_broker_path;
        try {
            expected_broker_path = ExpectedBrokerPath();
        } catch (...) {
            // An unrecognized or unrepresentable module location keeps the TIP
            // loadable but makes Broker activation fail closed and key input
            // fail open.
        }
        auto* service = new (std::nothrow) TextService(std::move(expected_broker_path));
        if (service == nullptr) {
            return E_OUTOFMEMORY;
        }
        const HRESULT result = service->QueryInterface(interface_id, object);
        service->Release();
        return result;
    }

    STDMETHODIMP LockServer(BOOL lock) noexcept override {
        if (lock != FALSE) {
            InterlockedIncrement(&g_server_locks);
        } else {
            const LONG remaining = InterlockedDecrement(&g_server_locks);
            if (remaining < 0) {
                InterlockedExchange(&g_server_locks, 0);
                return E_UNEXPECTED;
            }
        }
        return S_OK;
    }

private:
    ~ClassFactory() noexcept { InterlockedDecrement(&g_live_objects); }

    volatile LONG reference_count_ = 1;
};

}  // namespace

extern "C" BOOL WINAPI DllMain(HINSTANCE module, DWORD reason, void*) noexcept {
    if (reason == DLL_PROCESS_ATTACH) {
        g_module = module;
        DisableThreadLibraryCalls(module);
    }
    return TRUE;
}

STDAPI DllGetClassObject(
    REFCLSID class_id,
    REFIID interface_id,
    void** object) {
    if (object == nullptr) {
        return E_POINTER;
    }
    *object = nullptr;
    if (!IsEqualCLSID(class_id, mo::windows_tip::kTextServiceClsid)) {
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    auto* factory = new (std::nothrow) ClassFactory();
    if (factory == nullptr) {
        return E_OUTOFMEMORY;
    }
    const HRESULT result = factory->QueryInterface(interface_id, object);
    factory->Release();
    return result;
}

STDAPI DllCanUnloadNow() {
    return InterlockedCompareExchange(&g_live_objects, 0, 0) == 0
                   && InterlockedCompareExchange(&g_server_locks, 0, 0) == 0
               ? S_OK
               : S_FALSE;
}
