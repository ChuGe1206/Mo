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
bool FixtureFailure(int line) {
    std::wcerr << L"Candidate fixture failed at source line " << line << L".\n";
    return false;
}
}
namespace mo::windows_tip {
// Test-only friend: inspect the real renderer's measured text slots, not a copy
// of the layout calculation. Never exported by the TIP or used for user apps.
struct CandidateWindowFixture final {
    static RECT Candidate(const CandidateWindow& value, std::size_t index) {
        return value.compact_rects_[index];
    }
    static RECT Toolbar(const CandidateWindow& value, int index) { return value.ToolbarRect(index); }
    static bool FullCompactPage(const CandidateWindow& value) {
        if (value.compact_count_ != static_cast<int>(value.rows_.size())) { return FixtureFailure(__LINE__); }
        const HDC dc = GetDC(value.window_);
        if (dc == nullptr) { return FixtureFailure(__LINE__); }
        const auto old_font = SelectObject(dc, value.font_);
        bool success = true;
        for (std::size_t i = 0; i < value.rows_.size(); ++i) {
            RECT measured = value.compact_text_rects_[i];
            const RECT slot = measured;
            const int lines = DrawTextW(dc, value.rows_[i].text.data(), static_cast<int>(value.rows_[i].text.size()),
                &measured, DT_CALCRECT | DT_WORDBREAK | DT_EDITCONTROL | DT_NOPREFIX);
            if (lines <= 0 || measured.right > slot.right || measured.bottom > slot.bottom
                || slot.right > value.width_ - value.padding_ || slot.left < value.padding_) {
                success = false; break;
            }
        }
        SelectObject(dc, old_font); ReleaseDC(value.window_, dc);
        return success;
    }
    static bool ShortViewport(CandidateWindow& value) {
        value.work_.right = value.work_.left + value.Scale(240);
        value.work_.bottom = value.work_.top + value.Scale(100);
        return value.Reflow();
    }
    static bool ScrolledCandidateEnd(const CandidateWindow& value) {
        RECT client{};
        return GetClientRect(value.window_, &client) && value.compact_scroll_px_ > 0
            && value.compact_text_rects_[0].bottom <= client.bottom - value.padding_;
    }
    static bool NarrowWorkArea(CandidateWindow& value, int logical_width) {
        value.work_.right = value.work_.left + value.Scale(logical_width);
        return value.Reflow();
    }
};
}

namespace {
using namespace mo::windows_tip;
enum class PreviewToolbar : int { Previous, Next, Details, Symbols };
constexpr int kFixtureTabCenterY = 78;
constexpr int kFixtureFirstSymbolY = 118;
constexpr int kFixtureEmojiToggleRightInsetPx = 22;
void ClickToolbar(HWND window, const CandidateWindow& candidate, PreviewToolbar item) {
    const RECT box = CandidateWindowFixture::Toolbar(candidate, static_cast<int>(item));
    const LPARAM point = MAKELPARAM((box.left + box.right) / 2, (box.top + box.bottom) / 2);
    SendMessageW(window, WM_LBUTTONDOWN, MK_LBUTTON, point);
    SendMessageW(window, WM_LBUTTONUP, 0, point);
}
struct Actions {
    int count = 0;
    int total = 0;
    CandidateAction last = CandidateAction::Select;
    std::uint64_t revision = 0;
    std::uint32_t index = 0;
};
void Action(void* context, std::uint64_t revision, CandidateAction action,
    std::uint32_t index) noexcept {
    auto& actions = *static_cast<Actions*>(context);
    ++actions.total;
    actions.last = action; actions.revision = revision; actions.index = index;
    if (action == CandidateAction::Select) { ++actions.count; }
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
    if (!GetClientRect(window, &bounds)) { return FixtureFailure(__LINE__); }
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
        return FixtureFailure(__LINE__);
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
    const HRSRC notice = FindResourceW(module, MAKEINTRESOURCEW(306), RT_RCDATA);
    if (notice == nullptr) { return FixtureFailure(__LINE__); }
    const DWORD notice_size = SizeofResource(module, notice);
    const HGLOBAL notice_data = LoadResource(module, notice);
    const char* notice_bytes = static_cast<const char*>(LockResource(notice_data));
    if (notice_bytes == nullptr || notice_size < 1000 || notice_size > 4096) { return FixtureFailure(__LINE__); }
    const std::string notice_text(notice_bytes, notice_size);
    if (notice_text.find("Copyright (c) 2024 Figma") == std::string::npos
        || notice_text.find("Permission is hereby granted") == std::string::npos
        || notice_text.find("THE SOFTWARE IS PROVIDED") == std::string::npos) { return FixtureFailure(__LINE__); }

    const HWND focus = GetFocus();
    CandidateWindow candidate;
    Actions actions;
    BrokerSnapshot snapshot;
    snapshot.revision = 1;
    snapshot.composition = "qing";
    snapshot.symbols_supported = true;
    snapshot.quick_settings_supported = true;
    snapshot.page = CandidatePage{0, false, true, std::uint8_t{0}};
    snapshot.candidates = {"清", "请", "情", "晴", "青", "庆"};
    snapshot.candidate_comments = {"chūn fēng", "拼音", "拼音", "拼音", "拼音"};
    const RECT anchor{200, 200, 202, 224};
    for (const auto theme : {CandidateTheme::Light, CandidateTheme::Dark, CandidateTheme::System}) {
        if (!candidate.Update(module, owner, anchor, snapshot, theme, true, Action, &actions)) {
            return FixtureFailure(__LINE__);
        }
        const HWND window = FindCandidate();
        if (window == nullptr || GetFocus() != focus
            || SendMessageW(window, WM_MOUSEACTIVATE, 0, 0) != MA_NOACTIVATE) { return FixtureFailure(__LINE__); }
        RECT before{};
        GetWindowRect(window, &before);
        for (const UINT message : {WM_SYSCOLORCHANGE, WM_SETTINGCHANGE, WM_THEMECHANGED}) {
            SendMessageW(window, message, 0, 0);
            UpdateWindow(window);
            RECT after{};
            GetWindowRect(window, &after);
            if (!EqualRect(&before, &after) || GetFocus() != focus) { return FixtureFailure(__LINE__); }
        }
        // Capture the actual native renderer, containing only this fixture's synthetic input.
        if (!output.empty()) {
            const wchar_t* name = theme == CandidateTheme::Light ? L"paper.bmp"
                : theme == CandidateTheme::Dark ? L"midnight.bmp" : L"system.bmp";
            if (!Capture(window, output / name)) { return FixtureFailure(__LINE__); }
        }
        const int dpi = static_cast<int>(GetDpiForWindow(owner));
        const auto click = [window, dpi](int x, int y) {
                const LPARAM point = MAKELPARAM(MulDiv(x, dpi, 96), MulDiv(y, dpi, 96));
            SendMessageW(window, WM_LBUTTONDOWN, MK_LBUTTON, point);
            SendMessageW(window, WM_LBUTTONUP, 0, point);
        };
        if (!output.empty() && theme == CandidateTheme::Light) {
            ClickToolbar(window, candidate, PreviewToolbar::Details);
            if (!Capture(window, output / L"expanded.bmp")) { return FixtureFailure(__LINE__); }
            ClickToolbar(window, candidate, PreviewToolbar::Symbols);
            if (!Capture(window, output / L"emoji.bmp")) { return FixtureFailure(__LINE__); }
            click(224, kFixtureTabCenterY);
            if (!Capture(window, output / L"kaomoji.bmp")) { return FixtureFailure(__LINE__); }
            if (!candidate.ClosePanel()) { return FixtureFailure(__LINE__); }
        }
        const int count = actions.count;
        if (!CandidateWindowFixture::FullCompactPage(candidate)) { return FixtureFailure(__LINE__); }
        const RECT first_box = CandidateWindowFixture::Candidate(candidate, 0);
        const RECT second_box = CandidateWindowFixture::Candidate(candidate, 1);
        const auto press_at = [window](int px, int py) {
            const LPARAM point = MAKELPARAM(px, py);
            SendMessageW(window, WM_LBUTTONDOWN, MK_LBUTTON, point);
            SendMessageW(window, WM_LBUTTONUP, 0, point);
        };
        press_at(1, (first_box.top + first_box.bottom) / 2); // Outer inset cannot commit.
        press_at(first_box.right + 1, (first_box.top + first_box.bottom) / 2); // Gap cannot commit.
        if (actions.count != count) { return FixtureFailure(__LINE__); }
        press_at((second_box.left + second_box.right) / 2, (second_box.top + second_box.bottom) / 2);
        if (actions.count != count + 1 || actions.index != 1 || actions.revision != 1) { return FixtureFailure(__LINE__); }
        const LPARAM first = MAKELPARAM((first_box.left + first_box.right) / 2, (first_box.top + first_box.bottom) / 2);
        SendMessageW(window, WM_LBUTTONDOWN, MK_LBUTTON, first);
        snapshot.revision = 2;
        if (!candidate.Update(module, owner, anchor, snapshot, theme, false, Action, &actions)) {
            return FixtureFailure(__LINE__);
        }
        SendMessageW(window, WM_LBUTTONUP, 0, first);
        if (actions.count != count + 1 || GetFocus() != focus) { return FixtureFailure(__LINE__); }
        snapshot.revision = 1;
    }
    // Disabled affordances must not open panels or dispatch requests to older peers.
    const int dpi = static_cast<int>(GetDpiForWindow(owner));
    const HWND window = FindCandidate();
    const auto click = [window, dpi](int x, int y) {
        const LPARAM point = MAKELPARAM(MulDiv(x, dpi, 96), MulDiv(y, dpi, 96));
        SendMessageW(window, WM_LBUTTONDOWN, MK_LBUTTON, point);
        SendMessageW(window, WM_LBUTTONUP, 0, point);
    };
    snapshot.page.reset();
    snapshot.symbols_supported = false;
    if (!candidate.Update(module, owner, anchor, snapshot, CandidateTheme::Light,
        true, Action, &actions)) { return FixtureFailure(__LINE__); }
    int total = actions.total;
    ClickToolbar(window, candidate, PreviewToolbar::Previous); ClickToolbar(window, candidate, PreviewToolbar::Next); ClickToolbar(window, candidate, PreviewToolbar::Symbols);
    if (actions.total != total || candidate.panel_open()) { return FixtureFailure(__LINE__); }
    snapshot.page = CandidatePage{0, false, true, std::uint8_t{0}};
    snapshot.symbols_supported = true;
    snapshot.quick_settings_supported = false;
    if (!candidate.Update(module, owner, anchor, snapshot, CandidateTheme::Light,
        true, Action, &actions)) { return FixtureFailure(__LINE__); }
    ClickToolbar(window, candidate, PreviewToolbar::Next);
    if (actions.total != ++total || actions.last != CandidateAction::NextPage) { return FixtureFailure(__LINE__); }
    ClickToolbar(window, candidate, PreviewToolbar::Symbols);
    RECT symbol_client{};
    if (!GetClientRect(window, &symbol_client)) { return FixtureFailure(__LINE__); }
    click(MulDiv(symbol_client.right, 96, dpi) - kFixtureEmojiToggleRightInsetPx,
        kFixtureTabCenterY);
    if (!candidate.panel_open() || actions.total != total) { return FixtureFailure(__LINE__); }
    click(42, kFixtureTabCenterY); click(42, kFixtureFirstSymbolY);
    if (actions.total != ++total || actions.last != CandidateAction::InsertSymbol
        || actions.index != 0 || actions.revision != snapshot.revision) { return FixtureFailure(__LINE__); }
    click(224, kFixtureTabCenterY); click(42, kFixtureFirstSymbolY);
    if (actions.total != ++total || actions.last != CandidateAction::InsertSymbol
        || actions.index != 24) { return FixtureFailure(__LINE__); }
    if (!candidate.ClosePanel()) { return FixtureFailure(__LINE__); }
    BrokerSettings settings;
    for (std::uint8_t skin = 1; skin <= 6; ++skin) {
        settings.skin = skin;
        settings.font_size = skin == 6 ? 24 : 16;
        settings.background = 0xF4F4EA; settings.accent = 0x526143; settings.text = 0x303C2A;
        settings.radius = 20;
        if (!candidate.Update(module, owner, anchor, snapshot, CandidateTheme::Light,
            true, Action, &actions, &settings)) { return FixtureFailure(__LINE__); }
        RECT bounds{}; GetClientRect(window, &bounds);
        if (bounds.right > MulDiv(720, dpi, 96) || bounds.bottom > MulDiv(90, dpi, 96)
            || !CandidateWindowFixture::FullCompactPage(candidate) || GetFocus() != focus) { return FixtureFailure(__LINE__); }
        if (!output.empty() && !Capture(window, output / (L"skin-" + std::to_wstring(skin) + L".bmp"))) { return FixtureFailure(__LINE__); }
    }
    settings.vertical = true;
    if (!candidate.Update(module, owner, anchor, snapshot, CandidateTheme::Light,
        true, Action, &actions, &settings)) { return FixtureFailure(__LINE__); }
    RECT vertical{}; GetClientRect(window, &vertical);
    if (vertical.right > MulDiv(720, dpi, 96) || vertical.bottom <= MulDiv(60, dpi, 96)) { return FixtureFailure(__LINE__); }
    if (!output.empty() && !Capture(window, output / L"custom-vertical.bmp")) { return FixtureFailure(__LINE__); }
    const int count = actions.count;
    click(40, 78);
    if (actions.count != count + 1 || actions.index != 1) { return FixtureFailure(__LINE__); }
    // Ending a composition resets expansion even when the next preedit is identical.
    settings.vertical = false;
    if (!candidate.Update(module, owner, anchor, snapshot, CandidateTheme::Light,
        true, Action, &actions, &settings)) { return FixtureFailure(__LINE__); }
    ClickToolbar(window, candidate, PreviewToolbar::Details);
    candidate.Hide();
    if (!candidate.Update(module, owner, anchor, snapshot, CandidateTheme::Light,
        true, Action, &actions)) { return FixtureFailure(__LINE__); }
    if (candidate.panel_open()) { return FixtureFailure(__LINE__); }
    snapshot.composition = "nihao";
    snapshot.candidates = {"你好", "拟好", "你号", "泥好", "逆耗"};
    snapshot.candidate_labels.clear(); snapshot.candidate_comments.clear();
    settings.skin = 1; settings.vertical = false;
    if (!candidate.Update(module, owner, anchor, snapshot, CandidateTheme::Light,
        true, Action, &actions, &settings) || !CandidateWindowFixture::FullCompactPage(candidate)
        || candidate.panel_open()) { return FixtureFailure(__LINE__); }
    RECT nihao{}; GetClientRect(window, &nihao);
    if (nihao.right > MulDiv(340, dpi, 96) || nihao.bottom > MulDiv(64, dpi, 96)) { return FixtureFailure(__LINE__); }
    if (!output.empty() && !Capture(window, output / L"nihao-full-page.bmp")) { return FixtureFailure(__LINE__); }
    if (!CandidateWindowFixture::NarrowWorkArea(candidate, 240)
        || !CandidateWindowFixture::FullCompactPage(candidate)) { return FixtureFailure(__LINE__); }
    RECT narrow{}; GetClientRect(window, &narrow);
    if (narrow.bottom <= nihao.bottom || narrow.right > MulDiv(240, dpi, 96)) { return FixtureFailure(__LINE__); }
    if (!output.empty() && !Capture(window, output / L"nihao-auto-wrap.bmp")) { return FixtureFailure(__LINE__); }
    settings.skin = 6; settings.font_size = 24;
    if (!candidate.Update(module, owner, anchor, snapshot, CandidateTheme::Light,
        true, Action, &actions, &settings) || !CandidateWindowFixture::FullCompactPage(candidate)) { return FixtureFailure(__LINE__); }
    if (!output.empty() && !Capture(window, output / L"nihao-large-font.bmp")) { return FixtureFailure(__LINE__); }
    snapshot.candidates = {"合成测试中的超长候选也应在屏幕范围内自动换行并保持文字完整", "你好"};
    if (!candidate.Update(module, owner, anchor, snapshot, CandidateTheme::Light,
        true, Action, &actions, &settings) || !CandidateWindowFixture::NarrowWorkArea(candidate, 240)
        || !CandidateWindowFixture::FullCompactPage(candidate)) { return FixtureFailure(__LINE__); }
    if (!output.empty() && !Capture(window, output / L"long-candidate-wrap.bmp")) { return FixtureFailure(__LINE__); }
    for (const std::string& text : {std::string(80, 'a'), std::string("你好😀你好😀你好😀你好😀你好😀你好😀你好😀")}) {
        snapshot.candidates = {text, "你好"};
        if (!candidate.Update(module, owner, anchor, snapshot, CandidateTheme::Light,
            true, Action, &actions, &settings) || !CandidateWindowFixture::NarrowWorkArea(candidate, 240)
            || !CandidateWindowFixture::FullCompactPage(candidate)) { return FixtureFailure(__LINE__); }
    }
    snapshot.candidates = {std::string(512, 'a')};
    snapshot.candidate_comments.clear(); snapshot.candidate_labels.clear();
    if (!candidate.Update(module, owner, anchor, snapshot, CandidateTheme::Light,
        true, Action, &actions, &settings) || !CandidateWindowFixture::ShortViewport(candidate)) { return FixtureFailure(__LINE__); }
    for (int step = 0; step < 200; ++step) {
        SendMessageW(window, WM_MOUSEWHEEL, MAKEWPARAM(0, -WHEEL_DELTA), 0);
    }
    if (!CandidateWindowFixture::ScrolledCandidateEnd(candidate)
        || !CandidateWindowFixture::FullCompactPage(candidate) || GetFocus() != focus) { return FixtureFailure(__LINE__); }
    if (!output.empty() && !Capture(window, output / L"single-long-scroll-end.bmp")) { return FixtureFailure(__LINE__); }
    // Reusing the same DPI must not accumulate fonts, regions or brushes.
    const DWORD gdi_before = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
    for (int iteration = 0; iteration < 200; ++iteration) {
        if (!candidate.Update(module, owner, anchor, snapshot, CandidateTheme::Light,
            true, Action, &actions)) { return FixtureFailure(__LINE__); }
    }
    const DWORD gdi_after = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
    if (gdi_before == 0 || gdi_after > gdi_before + 2) { return FixtureFailure(__LINE__); }
    snapshot.candidates.assign(32, "合成超长候选用于窄屏截断检查");
    snapshot.candidate_labels.assign(32, "合成标签");
    snapshot.candidate_comments.assign(32, "合成注释");
    MONITORINFO monitor{};
    monitor.cbSize = sizeof(monitor);
    if (!GetMonitorInfoW(MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST), &monitor)) { return FixtureFailure(__LINE__); }
    const RECT corner{monitor.rcWork.right - 2, monitor.rcWork.bottom - 24,
        monitor.rcWork.right, monitor.rcWork.bottom};
    if (!candidate.Update(module, owner, corner, snapshot, CandidateTheme::Light,
        true, Action, &actions)) { return FixtureFailure(__LINE__); }
    RECT bounds{};
    GetWindowRect(window, &bounds);
    if (bounds.left < monitor.rcWork.left || bounds.top < monitor.rcWork.top
        || bounds.right > monitor.rcWork.right || bounds.bottom > monitor.rcWork.bottom) { return FixtureFailure(__LINE__); }
    if (!output.empty() && !Capture(window, output / L"bounded-long.bmp")) { return FixtureFailure(__LINE__); }
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
