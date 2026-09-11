# ADR 0001：Rust-first Windows 架构

- 状态：Accepted
- 日期：2026-09-11

## Context

Mo 需要自主实现 Windows 输入法前端，同时复用 C++ 的 librime。TSF DLL 会被加载进 Office、浏览器、Explorer 等宿主进程，任何崩溃、死锁或不正确的 COM 生命周期都会影响宿主。

## Decision

- Mo 的领域模型、引擎调度、Broker、候选窗、配置、包管理、更新和诊断以 Rust 2024 实现。
- librime 保持上游 C++17，通过官方 C API 进入唯一的 `mo-rime-sys` unsafe 边界。
- Windows v1 使用 Mo 自主实现的极薄 C++/WRL `MoTip.dll`，只承担 TSF/COM 生命周期、edit session、UIElement 和有界 IPC。
- 所有 librime 调用由单线程 Engine Actor 独占。
- 纯 Rust TSF 只作为 Phase 0 spike；通过同一兼容和稳定性门后才允许替换。

## Consequences

- Rust 承担可复用的长期产品逻辑，同时把宿主进程风险限制在最小代码面。
- 构建链需要 Rust/MSVC 和 C++/MSVC。
- C++ 壳与 Rust Broker 之间必须维护版本化 wire contract。
- 任何平台层绕过 Engine Actor 直接调用 librime 都属于架构违规。

