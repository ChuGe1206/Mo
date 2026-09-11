#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#define _WIN32_WINNT 0x0A00
#include <windows.h>

#include <msctf.h>

#include <iostream>

#include "mo_tip_ids.h"

namespace {

using DllGetClassObjectFunction = HRESULT(__stdcall*)(REFCLSID, REFIID, void**);
using DllCanUnloadNowFunction = HRESULT(__stdcall*)();

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

}  // namespace

int wmain(int argument_count, wchar_t** arguments) {
    if (argument_count != 2) {
        std::wcerr << L"Usage: mo_tip_abi_probe <absolute-path-to-mo_tip.dll>\n";
        return 2;
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

    service->Release();
    result = can_unload();
    if (result != S_OK) {
        FreeLibrary(module);
        return fail(L"DllCanUnloadNow", result);
    }

    FreeLibrary(module);
    std::wcout << L"Mo TIP ABI probe passed (load, exports, class factory, Ex interface, unload).\n";
    return 0;
}
