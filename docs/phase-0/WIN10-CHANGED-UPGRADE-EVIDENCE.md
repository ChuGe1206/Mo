# Win10 变更载荷升级测试包与驱动

日期：2026-10-08。develop / Win10 优先。本轮补齐 [0.0.12.0 开发包](WIN10-PACKAGED-0120-EVIDENCE.md)
尚缺的独立 guest harness；真实 VM 升级仍未执行。

## 新增能力

- prepare-vm-changed-upgrade-kit.ps1 绑定不同的旧、新 stage、linked 回执和 Bundle。
  旧 stage 按历史完整 inventory 复核，新 stage 必须通过当前准备策略。
  旧 ABI v2 仅描述旧安装树，不能用于新构建。
- changed-upgrade-policy.ps1 检查 exact Changed pair、Mo 升级家族、版本/产品/Bundle
  身份，以及 flat kit 的全部名字、大小和哈希。设置 fingerprint 只比较存在性、
  长度和 SHA-256；路径/祖先 reparse、UNC 和 alternate stream 拒绝。
- run-vm-changed-payload-upgrade.ps1 要求相符 VM sentinel、两项显式开关、普通权限
  64-bit 用户及精确旧 MSI/profile/finalizer 状态。已有 Mo Broker/settings 或
  设置 writer marker 时拒绝，不结束这些进程或删除 marker。
- 安装前逐项检查旧树/ACL/机器 COM，安装后检查新树/ACL/COM/profile、新旧 MSI、
  设置字节与默认 override。只有 exit 0 可继续；失败、reboot request 和检查
  不一致保留 failure、调用尝试、exit、final state 与日志哈希，completed 不置真。

完成项依据实际通过的检查写入。此驱动只运行新 Bundle；没有 clean install、
自动恢复旧版本、注入回滚、reboot 或桌面输入流程，也未证明所有宿主没有加载 TIP。

## 验证

| 本机检查 | 结果 |
| --- | --- |
| 新 kit/双安装树/设置/执行前拒绝策略 | 34 项通过 |
| 原升级 pair 策略 | 14 项通过 |
| 原 VM 生命周期策略 | 39 项通过 |
| 实际 0.0.11.0 → 0.0.12.0 kit | 构建及完整哈希、linked/registrar/两种载荷合同校验通过 |
| Win10 Windows PowerShell 5.1.19041.6456 | 实际 kit/最终 helper 与 driver 源哈希/AST 通过；两项主机拒绝门通过 |
| 脚本/CI | 四脚本 AST、十非空 CI PowerShell block、唯一新测试入口通过 |
| 远端 CI | 未核验 |

合成树各 132 个文件，可通过自己 manifest、不能通过另一版本 manifest。
篡改/重新列出 registrar、重新绑定错误 linked 产品、不同家族、同 stage、同包、
hidden/extra 文件、目录、junction 与设置创建/改写/删除均被拒绝。
初次测试用 PowerShell 自动变量保存参数，导致拒绝断言未执行；修正后完整
34 项通过并复跑。原始失败日志保留。测试不采集真实用户输入。

实际 WinPS 5.1 调用了未提供两开关和缺少 sentinel 的 driver，均在执行前拒绝；
没有创建 EvidenceDirectory 或启动安装器。成功升级路径仍需要 VM 实测，
不能由本机策略测试推断通过。本轮未修改 Rust/TSF 产品源码，未重建产品包。

## 最终测试包及使用

最终 kit：build/mo-vm-changed-upgrade-kit-0110-0120-v2。
含 12 个 hash-bound 文件及一份 manifest；manifest SHA-256：
5C6FFD012A7A9B8F22AA728C3CEFCAEEC3BA4CF81545F350F139BEE53F4CE65E。
v1 是调用状态命名完善前的中间产物，后续使用 v2。

将完整 v2 kit 复制到已初始化的 disposable Win10 VM 本地目录，在普通权限
64-bit PowerShell 5.1 执行：

~~~powershell
./run-vm-changed-payload-upgrade.ps1 -DisposableVm -AllowInstallerExecution -EvidenceDirectory C:\MoChangedEvidence0120
~~~

VM 用户处理 UAC。EvidenceDirectory 必须不存在；原用户/sentinel/Installed
receipt 必须吻合。准备器参数和边界见 [安装工具说明](../../installer/windows/README.md)。
kit 的未授权标记及哈希是未签名一致性记录，不能替代人类测试授权或代码签名。

## VM 状态与后续

VirtualBox CLI 确认 win10 在线。Computer Use 首次报告窗口最小化；
请求恢复并刷新窗口后捕获报：
FrameArrived timed out: timed out waiting on channel。
没有可见桌面、新包安装或输入证据。本轮未重试无密码账户登录；2026-10-07
的 guest command 账户限制仍为历史失败证据。

最近验证的 VM 保持 0.0.11.0 / ABI v2；本轮未升级至 0.0.12.0。
继续恢复桌面/执行访问，使用 v2 harness 先补单次变更载荷升级及设置保全，
随后安装态多窗口设置冲突、重新读取与 x64/x86 宿主；loaded TIP、登录/重启、
回滚和严格时延各自仍开放。时延专项保持降低优先级，G2/G3 不升级为整体通过。

## 封存

[Git 摘要](evidence/WIN10-CHANGED-UPGRADE-20261008.json)：
build/win10-evidence-clean-v1/ChangedUpgrade0120-v1，37 封存文件、
279 外部输入；manifest SHA-256：
87A5F2CDA740AFA1AB6AD9A3F0E11C02CDC75E43EFF7B787F11AEF7BD15912A2。
含最终 kit、验证源码、WinPS harness、原始正负日志、VM 访问失败；
旧、新 stage 全部条目逐项比对，前轮 Package0120-v1 的 manifest 未改。
