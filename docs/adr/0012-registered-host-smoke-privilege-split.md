# ADR 0012：注册宿主探针的权限拆分

- 状态：接受
- 日期：2026-09-15

## 背景

验证真实 TSF 路由必须让系统可发现 Mo 的 COM 类、language profile 与 keyboard category，并为测试用户启用该 profile。profile/category 注册需要机器级写入；Broker、探针和日常宿主则不应以管理员身份运行。自动拉起不可见 UAC 既无法可靠交互，也会把 Broker 测成与真实产品不同的高权限进程。

## 决策

- 机器级 setup/cleanup 由 `tools\machine-profile.ps1` 显式承担，且只允许在用户主动打开的提升 PowerShell 中运行。
- 普通权限的 `tools\tip-broker-smoke.ps1 -Registered` 要求机器 profile 已存在且当前用户状态干净；随后临时注册 HKCU x64/x86 COM、启用当前用户 profile，并通过真实 `ITfThreadMgr`/`ITfKeystrokeMgr` 发送按键。
- 普通权限脚本只清理由它创建的 HKCU COM 和当前用户启用状态；机器 profile 留给提升脚本显式移除，避免跨权限所有权模糊。
- 任何前置状态不符都拒绝运行。用户态临时状态由 `finally` 清理，profile 注册默认禁用，并且任何阶段都不把 Mo 设为默认输入法。
- CI 和非交互开发环境不得自动触发 UAC；没有提升窗口时只验证编译、权限门和无残留状态，并把真实注册宿主运行保留为人工验收步骤。

## 结果

真实系统 key route 有了可重复、可审计的验收入口，同时 Broker 保持普通用户权限。流程多出两个明确的管理员命令，但权限边界、状态所有权和失败清理均可预测，后续 MSI/Bootstrapper 可沿用相同的机器阶段与用户阶段拆分。

## 依据

- [Microsoft：Text Service Registration](https://learn.microsoft.com/en-us/windows/win32/tsf/text-service-registration)
- [Microsoft：64-bit Platform Considerations](https://learn.microsoft.com/en-us/windows/win32/tsf/64-bit-platform-considerations)
- [Microsoft：ITfInputProcessorProfileMgr::ActivateProfile](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofilemgr-activateprofile)
