# ADR 0004：开源与上游边界

- 状态：Accepted
- 日期：2026-09-11

## Context

librime 使用 BSD-3-Clause；rime-ice 根许可证为 GPL-3.0-only，且包含多种来源许可；多个现有平台前端使用 GPL 系许可证。

## Decision

- Mo 自有 Rust/C++ 源码以 Apache-2.0 开源。
- 平台前端依据 Windows 官方接口自主实现，不复制 GPL 前端源码。
- librime 作为固定版本的 BSD-3-Clause 外部依赖。
- 官方 Windows 预构建包仅用于 Phase 0 本地验证，不进入 Mo 发行物；该包静态包含 GPL-3.0-only 的 librime-octagram。
- 正式发行从锁定源码自行构建 librime，插件采用允许列表；当前只允许核心模块与 rime-ice 必需的 BSD-3-Clause librime-lua。
- rime-ice 衍生资源必须成为边界明确的 GPL-3.0-only 包，携带精确对应源、构建脚本、修改记录和第三方通知。
- 默认安装包纳入该资源前必须完成逐文件 SBOM 和正式许可证审查。

## Consequences

- 重命名或拆目录不会消除上游义务。
- 预编译词典不能脱离对应源码和可复现构建信息发布。
- 本 ADR 是工程策略，不替代法律意见。
