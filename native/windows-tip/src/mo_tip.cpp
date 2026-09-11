#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <msctf.h>
#include <new>

#include "mo_tip_ids.h"

namespace {

volatile LONG g_live_objects = 0;
volatile LONG g_server_locks = 0;

class TextService final : public ITfTextInputProcessorEx {
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

        thread_manager->AddRef();
        thread_manager_ = thread_manager;
        client_id_ = client_id;
        activation_flags_ = flags;

        // Phase 0 intentionally does not install key sinks, edit sessions,
        // language bars, UIElement providers, or broker IPC yet.
        return S_OK;
    }

    STDMETHODIMP Deactivate() noexcept override {
        if (thread_manager_ != nullptr) {
            thread_manager_->Release();
            thread_manager_ = nullptr;
        }
        client_id_ = TF_CLIENTID_NULL;
        activation_flags_ = 0;
        return S_OK;
    }

private:
    ~TextService() noexcept {
        Deactivate();
        InterlockedDecrement(&g_live_objects);
    }

    volatile LONG reference_count_ = 1;
    ITfThreadMgr* thread_manager_ = nullptr;
    TfClientId client_id_ = TF_CLIENTID_NULL;
    DWORD activation_flags_ = 0;
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
