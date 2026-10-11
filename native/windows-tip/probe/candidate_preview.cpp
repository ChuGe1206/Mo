#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <windowsx.h>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <vector>
#include "mo_candidate_window.h"

namespace {
using namespace mo::windows_tip;
struct Actions { int count = 0; std::uint64_t revision = 0; std::uint32_t index = 0; };
void Action(void* context, std::uint64_t revision, CandidateAction action,
    std::uint32_t index) noexcept {
    auto& actions = *static_cast<Actions*>(context);
    if (action == CandidateAction::Select) {
        ++actions.count; actions.revision = revision; actions.index = index;
    }
}
HWND FindCandidate() {
    HWND result = nullptr;
    EnumThreadWindows(GetCurrentThreadId(), [](HWND window, LPARAM value) -> BOOL {
        wchar_t name[64]{};
        GetClassNameW(window, name, 64);
        if (std::wstring(name) == L"Mo.CandidateWindow.v1") {
            *reinterpret_cast<HWND*>(value) = window;
            return FALSE;
        }
        return TRUE;
    }, reinterpret_cast<LPARAM>(&result));
    return result;
}
bool Capture(HWND window, const std::filesystem::path& path) {
    RECT bounds{};
    if (!GetClientRect(window, &bounds)) { return false; }
    BITMAPINFO info{};
    info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    info.bmiHeader.biWidth = bounds.right;
    info.bmiHeader.biHeight = -bounds.bottom;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    void* pixels = nullptr;
    const HDC dc = CreateCompatibleDC(nullptr);
    const HBITMAP bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &pixels, nullptr, 0);
    if (dc == nullptr || bitmap == nullptr || pixels == nullptr) {
        if (bitmap != nullptr) { DeleteObject(bitmap); }
        if (dc != nullptr) { DeleteDC(dc); }
        return false;
    }
    const HGDIOBJ previous = SelectObject(dc, bitmap);
    SendMessageW(window, WM_PRINTCLIENT, reinterpret_cast<WPARAM>(dc), PRF_CLIENT);
    GdiFlush();
    const DWORD bytes = static_cast<DWORD>(bounds.right * bounds.bottom * 4);
    BITMAPFILEHEADER header{};
    header.bfType = 0x4D42;
    header.bfOffBits = sizeof(header) + sizeof(info.bmiHeader);
    header.bfSize = header.bfOffBits + bytes;
    std::ofstream file(path, std::ios::binary);
    file.write(reinterpret_cast<const char*>(&header), sizeof(header));
    file.write(reinterpret_cast<const char*>(&info.bmiHeader), sizeof(info.bmiHeader));
    file.write(static_cast<const char*>(pixels), bytes);
    const bool success = file.good();
    SelectObject(dc, previous);
    DeleteObject(bitmap);
    DeleteDC(dc);
    return success;
}
bool Run(HWND owner, const std::filesystem::path& output) {
    const HINSTANCE module = GetModuleHandleW(nullptr);
    const HWND focus = GetFocus();
    CandidateWindow candidate;
    Actions actions;
    BrokerSnapshot snapshot;
    snapshot.revision = 1;
    snapshot.composition = "chun feng";
    snapshot.candidates = {"春风", "春峰", "淳风", "春丰", "春锋"};
    snapshot.candidate_comments = {"chūn fēng", "拼音", "拼音", "拼音", "拼音"};
    const RECT anchor{200, 200, 202, 224};
    for (const auto theme : {CandidateTheme::Light, CandidateTheme::Dark, CandidateTheme::System}) {
        if (!candidate.Update(module, owner, anchor, snapshot, theme, true, Action, &actions)) {
            return false;
        }
        const HWND window = FindCandidate();
        if (window == nullptr || GetFocus() != focus
            || SendMessageW(window, WM_MOUSEACTIVATE, 0, 0) != MA_NOACTIVATE) { return false; }
        RECT before{};
        GetWindowRect(window, &before);
        for (const UINT message : {WM_SYSCOLORCHANGE, WM_SETTINGCHANGE, WM_THEMECHANGED}) {
            SendMessageW(window, message, 0, 0);
            UpdateWindow(window);
            RECT after{};
            GetWindowRect(window, &after);
            if (!EqualRect(&before, &after) || GetFocus() != focus) { return false; }
        }
        // Capture the actual native renderer, containing only this fixture's synthetic input.
        if (!output.empty()) {
            const wchar_t* name = theme == CandidateTheme::Light ? L"paper.bmp"
                : theme == CandidateTheme::Dark ? L"midnight.bmp" : L"system.bmp";
            if (!Capture(window, output / name)) { return false; }
        }
        const int dpi = static_cast<int>(GetDpiForWindow(owner));
        const auto click = [window, dpi](int x, int y) {
            const LPARAM point = MAKELPARAM(MulDiv(x, dpi, 96), MulDiv(y, dpi, 96));
            SendMessageW(window, WM_LBUTTONDOWN, MK_LBUTTON, point);
            SendMessageW(window, WM_LBUTTONUP, 0, point);
        };
        const int count = actions.count;
        click(4, 58);  // Outer padding must not commit.
        click(24, 80); // Four-pixel row gap must not commit.
        if (actions.count != count) { return false; }
        click(24, 99);
        if (actions.count != count + 1 || actions.index != 1 || actions.revision != 1) { return false; }
        const LPARAM first = MAKELPARAM(MulDiv(24, dpi, 96), MulDiv(58, dpi, 96));
        SendMessageW(window, WM_LBUTTONDOWN, MK_LBUTTON, first);
        snapshot.revision = 2;
        if (!candidate.Update(module, owner, anchor, snapshot, theme, false, Action, &actions)) {
            return false;
        }
        SendMessageW(window, WM_LBUTTONUP, 0, first);
        if (actions.count != count + 1 || GetFocus() != focus) { return false; }
        snapshot.revision = 1;
    }
    // Reusing the same DPI must not accumulate fonts, regions or brushes.
    const DWORD gdi_before = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
    for (int iteration = 0; iteration < 200; ++iteration) {
        if (!candidate.Update(module, owner, anchor, snapshot, CandidateTheme::Light,
            true, Action, &actions)) { return false; }
    }
    const DWORD gdi_after = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
    if (gdi_before == 0 || gdi_after > gdi_before + 2) { return false; }
    snapshot.candidates.assign(32, "合成超长候选用于窄屏截断检查");
    snapshot.candidate_labels.assign(32, "合成标签");
    snapshot.candidate_comments.assign(32, "合成注释");
    MONITORINFO monitor{};
    monitor.cbSize = sizeof(monitor);
    if (!GetMonitorInfoW(MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST), &monitor)) { return false; }
    const RECT corner{monitor.rcWork.right - 2, monitor.rcWork.bottom - 24,
        monitor.rcWork.right, monitor.rcWork.bottom};
    if (!candidate.Update(module, owner, corner, snapshot, CandidateTheme::Light,
        true, Action, &actions)) { return false; }
    const HWND window = FindCandidate();
    RECT bounds{};
    GetWindowRect(window, &bounds);
    if (bounds.left < monitor.rcWork.left || bounds.top < monitor.rcWork.top
        || bounds.right > monitor.rcWork.right || bounds.bottom > monitor.rcWork.bottom) { return false; }
    if (!output.empty() && !Capture(window, output / L"bounded-long.bmp")) { return false; }
    candidate.Destroy();
    return GetFocus() == focus;
}
}
int wmain(int argc, wchar_t** argv) {
    if (argc > 2) { return 2; }
    std::filesystem::path output;
    if (argc == 2) {
        output = argv[1];
        if (!std::filesystem::is_directory(output)) { return 2; }
    }
    const HWND owner = CreateWindowExW(0, L"STATIC", L"Mo synthetic render owner",
        WS_POPUP, -32000, -32000, 32, 32, nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
    if (owner == nullptr) { return 1; }
    const bool success = Run(owner, output);
    DestroyWindow(owner);
    if (!success) { std::wcerr << L"Candidate native render/interaction fixture failed.\n"; return 1; }
    std::wcout << L"Candidate native render/focus/gap/stale-revision/workarea/GDI fixture passed.\n";
    return 0;
}
