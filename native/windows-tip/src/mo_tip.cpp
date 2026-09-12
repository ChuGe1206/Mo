#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <msctf.h>
#include <cstdint>
#include <limits>
#include <new>
#include <string>
#include <wrl/client.h>

#include "mo_broker_client.h"
#include "mo_tip_ids.h"

namespace {

using Microsoft::WRL::ComPtr;

volatile LONG g_live_objects = 0;
volatile LONG g_server_locks = 0;

constexpr DWORD kBrokerActivationTimeoutMs = 400;
constexpr DWORD kBrokerKeyTimeoutMs = 50;
constexpr ULONGLONG kBrokerReconnectBackoffMs = 250;
constexpr std::uint16_t kShiftModifier = 1U << 0U;
constexpr std::uint16_t kControlModifier = 1U << 1U;
constexpr std::uint16_t kAltModifier = 1U << 2U;
constexpr std::uint16_t kSuperModifier = 1U << 3U;
constexpr std::uint16_t kCapsLockModifier = 1U << 4U;

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

class TextService final : public ITfTextInputProcessorEx, public ITfKeyEventSink {
public:
    TextService() noexcept { InterlockedIncrement(&g_live_objects); }

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
        }
        return S_OK;
    }

    STDMETHODIMP Deactivate() noexcept override {
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
        cached_key_.valid = false;
        return result;
    }

    STDMETHODIMP OnSetFocus(BOOL foreground) noexcept override {
        has_focus_ = foreground != FALSE;
        cached_key_.valid = false;
        if (!has_focus_) {
            CancelActiveComposition();
            broker_.Close(kBrokerActivationTimeoutMs);
        } else {
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

private:
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
        if (broker_.connected()) {
            return true;
        }
        const ULONGLONG now = GetTickCount64();
        if (now < next_reconnect_tick_) {
            return false;
        }
        if (broker_.ConnectAndOpen(timeout_ms)) {
            next_reconnect_tick_ = 0;
            return true;
        }
        next_reconnect_tick_ = now + kBrokerReconnectBackoffMs;
        return false;
    }

    void DisconnectBroker(DWORD timeout_ms) noexcept {
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
        cached_key_.valid = false;
        if (!has_focus_ || !EnsureBrokerConnected(kBrokerKeyTimeoutMs)) {
            return false;
        }
        if (!broker_.SendKey(
                static_cast<UINT>(virtual_key),
                ScanCode(key_data),
                CurrentModifiers(),
                key_down,
                IsRepeat(key_data),
                &cached_key_.snapshot,
                kBrokerKeyTimeoutMs)) {
            return false;
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
            DisconnectBroker(kBrokerKeyTimeoutMs);
            return S_OK;
        }
        HRESULT session_result = E_FAIL;
        const HRESULT request_result = context->RequestEditSession(
            client_id_,
            edit_session,
            TF_ES_SYNC | TF_ES_READWRITE,
            &session_result);
        const bool applied = edit_session->applied();
        edit_session->Release();
        cached_key_.valid = false;
        if (applied) {
            *eaten = TRUE;
        }
        if (FAILED(request_result) || FAILED(session_result)) {
            DisconnectBroker(kBrokerKeyTimeoutMs);
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

        if (snapshot.commit.has_value()) {
            return CommitText(context, edit_cookie, text, applied);
        }
        if (text.empty()) {
            return ClearComposition(context, edit_cookie, applied);
        }
        return UpdateComposition(context, edit_cookie, text, applied);
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
                composition_.Reset();
                active_range_.Reset();
                composition_context_.Reset();
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
        *applied = true;

        ComPtr<ITfComposition> composition;
        result = context_composition->StartComposition(
            edit_cookie,
            range.Get(),
            nullptr,
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
            composition_.Reset();
            active_range_.Reset();
            composition_context_.Reset();
            return result;
        }
        *applied = true;
        if (composition_ != nullptr) {
            result = composition_->EndComposition(edit_cookie);
        }
        composition_.Reset();
        active_range_.Reset();
        composition_context_.Reset();
        return result;
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
            composition_.Reset();
            active_range_.Reset();
            composition_context_.Reset();
            return result;
        }
        *applied = true;
        if (composition_ != nullptr) {
            result = composition_->EndComposition(edit_cookie);
        }
        composition_.Reset();
        active_range_.Reset();
        composition_context_.Reset();
        return result;
    }

    void CancelActiveComposition() noexcept {
        if (active_range_ == nullptr || composition_context_ == nullptr) {
            composition_.Reset();
            active_range_.Reset();
            composition_context_.Reset();
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
        composition_.Reset();
        active_range_.Reset();
        composition_context_.Reset();
    }

    volatile LONG reference_count_ = 1;
    ITfThreadMgr* thread_manager_ = nullptr;
    ITfKeystrokeMgr* keystroke_manager_ = nullptr;
    TfClientId client_id_ = TF_CLIENTID_NULL;
    DWORD activation_flags_ = 0;
    bool has_focus_ = false;
    ULONGLONG next_reconnect_tick_ = 0;
    mo::windows_tip::BrokerClient broker_;
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

        auto* service = new (std::nothrow) TextService();
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
