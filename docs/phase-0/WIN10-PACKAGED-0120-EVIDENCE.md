# Win10 0.0.12.0 开发包整合

日期：2026-10-07。继续 develop，Win10 优先。按用户要求暂停扩展时延专项，
本轮将 ABI v3、直接退出 watchdog 和设置冲突保护纳入同一开发安装包。

## 构建与包身份

产品源码基于 6aebe7cebbde8fe401e20b498c03609e7973a001；锁定 Rust 1.97.1，fresh release 源快照，
debug_assertions=false、latency_trace=false。settings 两项源码与 build receipt
逐项比对；两种 TIP 严格编译和 ABI probe 通过。共 132 个安装载荷文件，
stage 的 138 个条目另含六项构建证据。

| 项目 | 身份 |
| --- | --- |
| 产品版本 | 0.0.12.0 unsigned DevelopmentTest |
| ProductCode | {E90EAFF7-050B-4127-B93C-7F0730F298BB} |
| Stage manifest SHA-256 | 7B99C1EA30F45E1B7559F85C28AFDD2DFF72FA747AF47EA21C033091420DAFFF |
| Bundle SHA-256 | B3C9475F0533F616FE4A5662F2723B9CD486BA1F05A6CA51A7EA6F8929A8F594 |
| MSI SHA-256 | 6FDA4592D85CBF8776D1099196F3FEF8468D723C06A5158BA0E02FDE93C74FB6 |
| librime SHA-256 | BE5E3E374D9FBE12381A0E3D522079033D85C08F4675F7A9A556A6DC3D30D564 |

DLL 是已验收的 strict-open ABI v3 运行时。exports 检查只有
mo_rime_prepare_resources_v3 准备入口，诊断 trace/latency/prefetch 导出缺席。
隔离耗时诊断 DLL 没有进入此包。源码和产物分别绑定；不将不同重建产物称为字节相同。

linked MSI/Burn 反向校验和无警告 MSI ICE 通过。kit 的文件集合和全部哈希、
stage inventory、release 标记再次检查通过。原始日志在
build/win10-settings-abi3-*.log；输入/产物目录分别为：

- build/mo-windows-stage-win10-settings-abi3-v1/stage
- build/mo-linked-win10-settings-abi3-0120-v1
- build/mo-vm-kit-win10-settings-abi3-0120-v1

包仍是带开发故障注入、未签名且不可分发的测试构建；结构和 ICE 检查没有执行安装器。

## 变更载荷升级入口

verify-linked-upgrade-pair.ps1 新增显式 PayloadMode Changed。默认 Same 仍要求
stage 相同，原 format 2、17 字段回执保持。Changed 要求 stage 不同，
输出独立 kind 的 format 3、20 字段回执，分别绑定旧、新 stage 及源 linked
回执哈希。前三段 MSI 版本递增、相同升级家族、不同产品/Bundle/包身份继续检查。

14 项合成策略测试通过并接入 CI；原包作者 21 项、VM 生命周期策略 39 项通过。
真实 0.0.11.0 → 0.0.12.0 pair 通过，默认 Same 模式拒绝它；现有同载荷
prepare-vm-matrix-test-kit.ps1 也拒绝 Changed 回执。输出位于
build/mo-win10-changed-pair-0110-0120-v1/linked-upgrade-pair-evidence.json。
初次调用用了相对路径，被策略拒绝；原始拒绝日志保留，改用绝对路径后通过。
本轮未创建可执行的变更载荷升级 driver，后续仍需独立 guest harness。

两项修改脚本 AST、十个非空 CI PowerShell block AST 和唯一测试入口检查通过。
远端 CI 尚未核验。只有安装工具与文档改动，本轮没有修改 Rust/TSF 产品代码；
此前设置修复的 workspace 验证见 [ADR 0057](../adr/0057-settings-checked-save.md)。

## VM 当前状态与阻碍

VirtualBox win10 正在运行，2 vCPU、4096 MB，Guest Additions 在线。只读 guest
command 使用用户说明的无密码账户，被拒绝为：
The specified user account on the guest is restricted and can't be used to logon。
Computer Use 按技能流程重新识别窗口后重试，连续两次
failed to activate captured window，未能取得桌面或执行输入。

本轮没有执行新包升级或设置 GUI 测试。最近一次已验证的 VM 安装版本仍是
0.0.11.0、旧 ABI v2；不能将新包结构验证写成 VM ABI v3 安装态通过。
上述访问失败不提供 VM 性能证据。近期时延在开发主机也复现；
50/400 ms、G2/G3 的开放项保持。

下一步恢复 VM 桌面访问，先独立核验 0.0.11.0 原安装树和设置/默认状态，
再按不同 stage 执行 0.0.12.0 升级，检查新旧 MSI、全部载荷、ACL、COM、
profile/finalizer 与设置/默认保护；随后验证安装态多窗口设置冲突/重新读取、
x64/x86 真实宿主，再推进 loaded TIP 与登录重启。已有时延负面证据保留。

## 封存与交接

[Git 摘要](evidence/WIN10-PACKAGED-0120-20261007.json) 绑定
build/win10-evidence-clean-v1/Package0120-v1：
123 个封存文件、139 个外部输入，
manifest SHA-256 为 62E9E136E1DE7AA968A9FDF039DF6DC4BC0D0DDBCDD93DEE76DF2285F22A625F。
其中包含产品源快照、实际 Broker/settings/TIP/registrar/Rime、Bundle/MSI/kit、
linked 回执及 pair、正负日志和 VM 访问失败状态。外部 stage 文件逐项哈希校验；
旧档案未改。哈希是未签名的一致性证据。

## 2026-10-08 后续入口

独立双清单 kit 与 guest upgrade driver 已补齐，实际 kit 和 WinPS 5.1 拒绝门通过。
VM 升级仍未执行；后续使用 v2，见 [变更载荷升级续测](WIN10-CHANGED-UPGRADE-EVIDENCE.md)。
