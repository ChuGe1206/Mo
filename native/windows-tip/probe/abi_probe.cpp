#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#define _WIN32_WINNT 0x0A00
#include <windows.h>

#include <msctf.h>
#include <olectl.h>

#include <iostream>
#include <new>
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

}  // namespace

int wmain(int argument_count, wchar_t** arguments) {
    if (argument_count != 2) {
        std::wcerr << L"Usage: mo_tip_abi_probe <absolute-path-to-mo_tip.dll>\n";
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

    if (probe_key_sink_activation(service) != 0) {
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
    std::wcout
        << L"Mo TIP ABI probe passed (load, exports, class factory, Ex/key sink lifecycle, unload).\n";
    return 0;
}
