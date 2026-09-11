# ADR 0002：Windows-only Phase 0

- 状态：Accepted
- 日期：2026-09-11

## Context

多个平台同时启动会复制输入生命周期、安装、签名和候选 UI 风险，无法尽快验证产品闭环。

## Decision

- 当前只实现 Windows。
- 正式支持仍处于微软生命周期内的 Windows 11 x64。
- Windows 10 22H2 只做尽力兼容，不构成安全支持承诺。
- x64 Windows 上同时提供 x64 和 x86 TIP；ARM64/ARM64X 另行立项。
- 其他平台不创建空壳工程，只保留平台无关 domain、`mo-core-ffi`、pack format、用户数据格式和 golden corpus。

## Consequences

- Phase 0 到公开 Beta 的所有交付门只围绕 Windows。
- 任何其他平台工作必须经过 Windows Beta 后的单独确认。

