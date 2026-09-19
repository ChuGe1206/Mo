#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <dia2.h>

#include <cerrno>
#include <cstdint>
#include <iostream>
#include <limits>
#include <wrl/client.h>

namespace {
using Microsoft::WRL::ComPtr;
using DllGetClassObjectFunction = HRESULT(__stdcall*)(REFCLSID, REFIID, void**);

bool ParseRva(const wchar_t* value, DWORD64* result) noexcept {
    if (value == nullptr || result == nullptr || *value == L'\0' || *value == L'-') { return false; }
    wchar_t* end = nullptr;
    errno = 0;
    const unsigned long long parsed = wcstoull(value, &end, 0);
    if (errno != 0 || end == value || *end != L'\0' || parsed > std::numeric_limits<DWORD>::max()) {
        return false;
    }
    *result = static_cast<DWORD64>(parsed);
    return true;
}
}  // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc < 5) {
        std::wcerr << L"usage: mo_stack_symbol_resolver.exe <image> <matching-pdb> <msdia140.dll> <rva> [...]\n";
        return 2;
    }
    const HRESULT initialized = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    if (FAILED(initialized)) {
        std::wcerr << L"CoInitializeEx failed: 0x" << std::hex << initialized << L'\n';
        return 1;
    }
    int outcome = 1;
    HMODULE dia_module = nullptr;
    do {
    // Load the explicitly supplied PDB with DIA. The caller must obtain it
    // using the image's CodeView GUID+age; no ambient server is consulted.
    if (GetFileAttributesW(argv[1]) == INVALID_FILE_ATTRIBUTES
        || GetFileAttributesW(argv[2]) == INVALID_FILE_ATTRIBUTES
        || GetFileAttributesW(argv[3]) == INVALID_FILE_ATTRIBUTES) {
        std::wcerr << L"Image, matching PDB or DIA runtime does not exist.\n";
        break;
    }
    dia_module = LoadLibraryExW(argv[3], nullptr,
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
    const auto get_class_object = dia_module != nullptr
        ? reinterpret_cast<DllGetClassObjectFunction>(GetProcAddress(dia_module, "DllGetClassObject")) : nullptr;
    if (get_class_object == nullptr) {
        std::wcerr << L"Could not load the explicit DIA runtime: " << GetLastError() << L'\n';
        break;
    }
    ComPtr<IDiaDataSource> source;
    ComPtr<IClassFactory> factory;
    HRESULT result = get_class_object(CLSID_DiaSource, IID_PPV_ARGS(&factory));
    if (SUCCEEDED(result)) { result = factory->CreateInstance(nullptr, IID_PPV_ARGS(&source)); }
    if (FAILED(result)) {
        std::wcerr << L"Create DIA data source failed: 0x" << std::hex << result << L'\n';
        break;
    }
    result = source->loadDataFromPdb(argv[2]);
    if (FAILED(result)) {
        std::wcerr << L"IDiaDataSource::loadDataFromPdb failed: 0x" << std::hex << result << L'\n';
        break;
    }
    ComPtr<IDiaSession> session;
    result = source->openSession(&session);
    if (FAILED(result)) {
        std::wcerr << L"IDiaDataSource::openSession failed: 0x" << std::hex << result << L'\n';
        break;
    }
    for (int index = 4; index < argc; ++index) {
        DWORD64 rva = 0;
        if (!ParseRva(argv[index], &rva)) {
            std::wcerr << L"Invalid RVA: " << argv[index] << L'\n';
            outcome = 2; break;
        }
        ComPtr<IDiaSymbol> symbol;
        result = session->findSymbolByRVA(static_cast<DWORD>(rva), SymTagFunction, &symbol);
        if (result != S_OK) {
            result = session->findSymbolByRVA(static_cast<DWORD>(rva), SymTagPublicSymbol, &symbol);
        }
        if (result != S_OK || symbol == nullptr) {
            std::wcout << L"MO_SYMBOL rva=0x" << std::hex << rva << std::dec
                << L" unresolved=1 result=0x" << std::hex << result << std::dec << L'\n';
            continue;
        }
        BSTR name = nullptr;
        DWORD symbol_rva = 0;
        result = symbol->get_undecoratedName(&name);
        if (result != S_OK || name == nullptr) {
            if (name != nullptr) { SysFreeString(name); name = nullptr; }
            result = symbol->get_name(&name);
        }
        if (FAILED(result) || FAILED(symbol->get_relativeVirtualAddress(&symbol_rva)) || name == nullptr) {
            if (name != nullptr) { SysFreeString(name); }
            std::wcout << L"MO_SYMBOL rva=0x" << std::hex << rva << std::dec << L" metadata_failed=1\n";
            continue;
        }
        std::wcout << L"MO_SYMBOL rva=0x" << std::hex << rva
            << L" displacement=0x" << (rva - symbol_rva) << std::dec << L" name=";
        const UINT length = SysStringLen(name);
        for (UINT character = 0; character < length; ++character) {
            const wchar_t value = name[character];
            std::wcout << (value >= 0x20 && value <= 0x7e ? value : L'?');
        }
        std::wcout << L'\n';
        SysFreeString(name);
    }
    if (outcome != 2) { outcome = 0; }
    } while (false);
    if (dia_module != nullptr) { FreeLibrary(dia_module); }
    CoUninitialize();
    return outcome;
}
