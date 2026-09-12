#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#define _WIN32_WINNT 0x0A00
#include <windows.h>

#include <msctf.h>
#include <olectl.h>
#include <textstor.h>

#include <iostream>
#include <limits>
#include <new>
#include <string>
#include <wrl/client.h>

#include "mo_tip_ids.h"

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
        LONG,
        LONG,
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
        *clipped = FALSE;
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

bool SendTestedKey(
    ITfKeyEventSink* key_sink,
    ITfContext* context,
    EditTextStore* text_store,
    WPARAM virtual_key) {
    const UINT scan_code = MapVirtualKeyW(static_cast<UINT>(virtual_key), MAPVK_VK_TO_VSC);
    const LPARAM key_data = 1 | (static_cast<LPARAM>(scan_code) << 16);
    BOOL tested_eaten = FALSE;
    HRESULT result = key_sink->OnTestKeyDown(context, virtual_key, key_data, &tested_eaten);
    if (FAILED(result) || tested_eaten == FALSE) {
        if (FAILED(result)) {
            fail(L"ITfKeyEventSink::OnTestKeyDown", result);
        } else {
            std::wcerr << L"OnTestKeyDown did not consume virtual key 0x"
                       << std::hex << virtual_key << L'\n';
        }
        return false;
    }
    BOOL handled_eaten = FALSE;
    result = key_sink->OnKeyDown(context, virtual_key, key_data, &handled_eaten);
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

int probe_broker_input(ITfTextInputProcessorEx* service, bool rime_ice) {
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
    result = thread_manager->Activate(&client_id);
    if (FAILED(result)) {
        return fail(L"ITfThreadMgr::Activate", result);
    }

    int outcome = 0;
    bool service_active = false;
    bool context_pushed = false;
    HWND edit_window = nullptr;
    auto* fake_manager = new (std::nothrow) FakeThreadManager();
    if (fake_manager == nullptr) {
        thread_manager->Deactivate();
        return fail(L"FakeThreadManager allocation", E_OUTOFMEMORY);
    }
    ComPtr<ITfThreadMgr> service_thread_manager;
    service_thread_manager.Attach(static_cast<ITfThreadMgr*>(fake_manager));
    ComPtr<ITextStoreACP> text_store;
    EditTextStore* edit_store = nullptr;
    ComPtr<ITfDocumentMgr> document_manager;
    ComPtr<ITfContext> context;
    do {
        edit_window = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            L"EDIT",
            L"",
            WS_POPUP | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL,
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
        // The real context recognizes the client id returned above. The fake
        // manager captures the service key sink so the probe can invoke it
        // deterministically without registering Mo as a system TIP.
        result = service->ActivateEx(service_thread_manager.Get(), client_id, 0);
        if (FAILED(result)) {
            outcome = fail(L"ITfTextInputProcessorEx::ActivateEx", result);
            break;
        }
        service_active = true;

        ComPtr<ITfKeyEventSink> key_sink;
        result = service->QueryInterface(IID_PPV_ARGS(key_sink.GetAddressOf()));
        if (FAILED(result)) {
            outcome = fail(L"QueryInterface(ITfKeyEventSink)", result);
            break;
        }
        result = key_sink->OnSetFocus(TRUE);
        if (FAILED(result)) {
            outcome = fail(L"ITfKeyEventSink::OnSetFocus", result);
            break;
        }

        const std::string input = rime_ice ? "NIHAO" : "M";
        bool keys_succeeded = true;
        for (const char key : input) {
            if (!SendTestedKey(
                    key_sink.Get(),
                    context.Get(),
                    edit_store,
                    static_cast<WPARAM>(key))) {
                keys_succeeded = false;
                break;
            }
        }
        if (!keys_succeeded
            || !SendTestedKey(key_sink.Get(), context.Get(), edit_store, VK_SPACE)) {
            outcome = 1;
            break;
        }

        wchar_t text_buffer[16]{};
        const int text_length = GetWindowTextW(edit_window, text_buffer, ARRAYSIZE(text_buffer));
        const std::wstring text(text_buffer, static_cast<std::size_t>(text_length));
        const std::wstring expected = rime_ice ? L"你好" : L"m";
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

    if (service_active) {
        result = service->Deactivate();
        if (FAILED(result) && outcome == 0) {
            outcome = fail(L"ITfTextInputProcessor::Deactivate", result);
        }
    }
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
    const bool broker_input = argument_count == 3
        && std::wstring(arguments[2]) == L"--broker-input";
    const bool broker_rime_ice = argument_count == 3
        && std::wstring(arguments[2]) == L"--broker-rime-ice";
    if (argument_count != 2 && !broker_input && !broker_rime_ice) {
        std::wcerr
            << L"Usage: mo_tip_abi_probe <absolute-path-to-mo_tip.dll> "
               L"[--broker-input|--broker-rime-ice]\n";
        return 2;
    }

    const ComApartment apartment;
    if (FAILED(apartment.result())) {
        return fail(L"CoInitializeEx", apartment.result());
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

    result = service->ActivateEx(nullptr, TF_CLIENTID_NULL, 0);
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

    if ((broker_input || broker_rime_ice
             ? probe_broker_input(service, broker_rime_ice)
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
    std::wcout << (broker_rime_ice
        ? L"Mo TIP Broker/librime/rime-ice edit-session commit probe passed.\n"
        : broker_input
            ? L"Mo TIP Broker key cache/edit-session commit probe passed.\n"
            : L"Mo TIP ABI probe passed (load, exports, class factory, Ex/key sink lifecycle, unload).\n");
    return 0;
}
