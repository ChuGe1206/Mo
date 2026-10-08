# Win10 0.0.13.0 高对比度修复整合包

日期：2026-10-08。develop；产品源 commit 0babd752852db62b3044400485e56ee94a2a1982。
本轮将 [ADR 0058](../adr/0058-candidate-system-high-contrast.md) 的候选高对比度
及外观重绘修复编入新开发包，保留已整合的 ABI v3、直接退出 watchdog 和设置冲突保护。

## 构建与身份

从 89 文件源码快照 fresh 构建；Rust 1.97.1、静态 CRT release，
debug_assertions=false、latency_trace=false，双架构 /W4 /WX TIP/registrar。
新增 palette 头、修正后的 Unicode 查询和三个外观消息处理均与源码及回执匹配。
132-file payload / 138 stage entries 完整校验。相对 0.0.12.0，本次重建的五个
Mo PE 字节均变化；资源和 librime 字节保持。重建身份不代表五处功能都改变。

| 项目 | 身份 |
| --- | --- |
| 版本 | 0.0.13.0 unsigned DevelopmentTest |
| ProductCode | {316BB0EA-067B-486B-A270-A3C0E872DEF9} |
| Stage SHA-256 | 02CEC0FFA4D2C889FBA04C0C484065543E574DF4D42C59E48FD0929578244590 |
| Bundle SHA-256 | 9FE40F9052A7B3BEE6F2262D94E7935FCA7394A7E3F5D1057F8275D023E1A4F4 |
| MSI SHA-256 | B6AB90623DE7F2CC4B48B5A4321087AFC127F4AA57E249600A0608107BE8B11D |
| librime SHA-256 | BE5E3E374D9FBE12381A0E3D522079033D85C08F4675F7A9A556A6DC3D30D564 |
| Upgrade kit SHA-256 | CD8BB51423AA7606BDC510DFFF986F8C2CF1CCE6AF3DF87816C9B89A290C8E25 |

继续使用已验收的 strict-open ABI v3 运行时，未重建 librime。
开发故障注入在 registrar/MSI/Burn 中保留；包未签名、不可分发，不是正式发行版。

- Stage：build/mo-windows-stage-win10-contrast-abi3-v1/stage
- MSI/Burn：build/mo-linked-win10-contrast-abi3-0130-v1
- 变更载荷 pair：build/mo-win10-changed-pair-0110-0130-v1
- Guest kit：build/mo-vm-changed-upgrade-kit-0110-0130-v1

## 本轮检查

- 锁定 rime-ice tar 的全新资源部署通过，来源/输出与 ABI v3 runtime 绑定。
- Release Broker 拒绝 --fake；双架构 ABI/COM 生命周期、配色策略和 registrar
  机器事务/用户 finalizer 自测通过。仅非变更策略/自有 marker 测试。
- 实际 staged TIP DLL 原字节复制到隔离安装形状目录，配对自有诊断 fake Broker：
  三个外观消息触发重绘且焦点/文本/矩形不变，候选交互、提交及每架构两次
  Broker 退出/恢复通过。临时树和自有进程正常清理。
  这项检查没有把 staged release Broker 作为已安装输入服务，也不是系统注册路由。
- MSI/Burn 反向校验与无警告 MSI ICE 通过；132-file MSI 清单及 Bundle 内嵌
  MSI/registrar/failure injector 与输入一致。已知 mixed-scope WIX1140 抑制不变。
- 实际 Changed pair 0.0.11.0 → 0.0.13.0 通过，使用独立 stage 清单与不同产品身份。
  12 文件升级 kit 加一份 manifest 绑定全部输入和当前 format 2 最终审计 driver。
- WinPS 5.1.19041.6456 校验实际 kit 哈希/四份最终脚本；
  实际 kit driver 的缺执行开关、缺 sentinel 两项 host 拒绝通过，
  未创建安装证据目录、未调用安装器。
- Rust 产品代码本轮没有改动；fmt/Clippy/workspace 的 163 项通过结果属于
  产品源 commit 0babd75 的前轮验证，不声称本轮重跑。

真实 Rime 输入烟测、系统高对比度切换、注册/真实宿主、安装态设置 GUI、
升级/loaded TIP/登录重启本轮未执行。资源部署通过不能替代这些验收。
50 ms key / 400 ms activation、G2/G3 仍开放；时延专项维持暂缓优先级。
远端 CI 未核验。

## 封存与 VM 入口

[Git 摘要](evidence/WIN10-PACKAGED-0130-20261008.json)：
build/win10-evidence-clean-v1/Package0130-v1，160 封存文件、
140 外部输入。
manifest SHA-256：1451CC7F0B477CF21355C3168182F42F0293C136024F4C919528B101370F0298。
包含产品源、实际 PE、MSI、双 Bundle kit、双阶段/linked/pair 回执、
构建/烟测/WinPS 日志与精确 harness；一致性哈希不是签名。
旧 0.0.11.0/0.0.12.0 stage manifest 保持原身份，历史档案继续保留。

最近已验证的 VM 安装仍是 0.0.11.0 / ABI v2；本轮没有新 VM 访问尝试。
既有 Computer Use 截图超时和无密码 guest-command 账号限制尚未解决，
这些访问错误不提供 VM 性能结论。
恢复可操作桌面或由 VM 用户执行回传后，再核验真实安装。

本 kit 只接受旧 0.0.11.0 基线。若已使用前一 0.0.12.0 kit，先回传其证据，
按该实际安装基线准备下一升级 pair。

使用现有 MoEvidence 共享目录和原已初始化 disposable VM 普通用户，
打开 64-bit Windows PowerShell；关闭测试中的 Mo Broker/settings，
保留原 sentinel、启用且未活动的 Mo 和默认输入法。以下本地目录须尚未存在：

~~~powershell
Copy-Item -LiteralPath '\\vboxsvr\MoEvidence\Package0130-v1\kit' -Destination 'C:\MoChanged0130' -Recurse
powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\MoChanged0130\run-vm-changed-payload-upgrade.ps1 -DisposableVm -AllowInstallerExecution -EvidenceDirectory C:\MoChangedEvidence0130
Copy-Item -Path 'C:\MoChangedEvidence0130\*' -Destination '\\vboxsvr\MoEvidence\ChangedUpgradeGuest0130-v1'
~~~

UAC 由 VM 用户处理。保留 upgrade-state.json 和 Burn 日志，失败结果也回传；
完成门要求升级后载荷/ACL/COM/MSI、settings/default 保全和最终 registrar
Installed 状态均通过。回传目录与不可覆盖的封存目录分开。

升级成功后，先在 Notepad 用 Light/Dark 候选主题核验系统高对比度。
记录原系统开关和 Mo 主题，在 Win10 辅助功能中开启高对比度，再回到 Notepad
生成候选：预编辑、正文、页脚应采用系统前景/背景，按压采用系统高亮。
关闭高对比度后应恢复保存主题，核对提交和候选分页；验证结束恢复原选择。
另补安装态多窗口设置的冲突拒绝、重新读取与保存。
