# 开发态注册系统路由测试

此步骤只用于本机开发验收，不是发行安装器。当前证据仍不包含注册后运行成功：开发终端非管理员，机器 profile 尚未准备。请勿把脚本和策略测试通过当成普通软件验收。

## 1. 管理员准备

先完成普通权限双架构构建。随后**用户主动打开管理员 PowerShell**，执行：

```powershell
Set-Location 'E:\ChuGe\CodeProject\101_ProjectCollection\Mo'
.\tools\machine-profile.ps1 -Action Register
```

这一步只创建 Mo 的机器级 TSF profile/category，不注册用户 COM、不启用、不设默认。已有 Mo profile 时拒绝覆盖。注册 API 失败时不要继续用户测试；检查 `mo_tip_registrar.exe status` 和脚本报错。

不要从测试脚本自动启动 UAC，也不要在管理员终端启动 Broker/用户态测试。管理员准备完成后，回到普通权限 PowerShell 7（`pwsh`）终端；真实引擎脚本使用 PowerShell 7 的运行时接口。

## 2. 普通权限测试

先验证 fake 的真实系统路由：

```powershell
.\tools\tip-broker-smoke.ps1 -Architecture All -Registered
```

真实词库使用已经核验、锁定并部署的开发资产：

```powershell
.\tools\tip-rime-smoke.ps1 `
  -LibrimeDistDir <verified-librime-dist> `
  -SharedDataDir <pinned-rime-ice> `
  -UserDataDir <disposable-deployed-user-root> `
  -Architecture All -Registered
```

脚本要求 Mo HKCU x64/x86 COM 均缺席、profile 已注册但当前用户禁用且不活跃。临时注册双视图 COM 并启用后，分别通过 x64/Win32 的系统 `ITfThreadMgr`/`ITfKeystrokeMgr` 激活本进程 profile、发送输入、核对 EDIT 与 TSF context 的两轮提交。真实词库目标为 `你好你好`，fake 为 `mm`。Broker 始终普通权限，真实引擎探针使用独立可回收用户子目录。

临时用户态状态在 finally 中清理并读回。即使 setup 或 probe 失败，也尝试独立 disable/unregister；部分写入失败不会因为没有“成功返回”而跳过清理。任何清理错误/读回残留使整个测试失败，不会宣称成功。清理前发现非本次测试的 COM 路径会保留、停止删除并要求人工审查；这是并发变化防误删检查，不是原子注册事务。

## 3. 管理员清理

无论测试结果如何，最后在管理员 PowerShell 执行：

```powershell
Set-Location 'E:\ChuGe\CodeProject\101_ProjectCollection\Mo'
.\tools\machine-profile.ps1 -Action Unregister
```

最终 `status` 应为两视图 COM 缺席，以及 profile.registered/enabled/active 全 false。机器 profile 的管理归管理员阶段，用户态脚本不会冒充能够跨权限回滚它。如果用户脚本报告残留，先按错误审查所有权，不要直接递归清理注册表。

## 限制与后续

系统 key-route probe 仍是受控文本存储，不是 Notepad/浏览器/WinUI 的正式 composition 和候选窗验收。注册路由通过后才能进入这些真实软件矩阵；Windows 11、多屏 DPI、AppContainer、进程崩溃与安装回滚仍单独验收。

`tools\test-registered-tip-state.ps1` 的 16 个场景只用内存 registrar，覆盖解析、脏状态拒绝、部分写入、probe 失败、清理失败/静默残留和外来 COM 路径保护。它不写 Windows 注册表、不启用输入法，也不证明 Windows API 的实际注册行为。
