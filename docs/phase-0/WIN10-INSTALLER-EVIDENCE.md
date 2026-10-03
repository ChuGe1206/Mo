# Win10 安装生命周期与回滚/升级实测

日期：2026-10-03。VirtualBox win10，Win10 22H2 x64 build 19045；普通用户启动 Burn，提权仅用于机器 MSI。全部使用隔离 VM 的 unsigned DevelopmentTest 包和同一哈希锁定 payload。

## 基础生命周期

0.0.9.11 的 Install / Repair / Uninstall 全部 exit 0，completed=true。默认输入法未变，安装/修复后文件 inventory、SHA-256、ACL/owner/hardlink/reparse、双架构 HKLM COM 与用户 marker/receipt 校验通过；卸载根目录、profile 与机器 COM 清除。

原始证据：`build/win10-evidence-clean-v1/V0911/vm-lifecycle-evidence.json`。此前 0.0.9.10 的空根目录失败证据继续保留；修复与边界见 ADR 0052。

## 六阶段矩阵

| 阶段 | 实际 exit | 验收结果 |
| --- | --- | --- |
| 机器 profile 成功写入后故意失败 | 1603 | MSI 逆操作后 profile/COM/文件与产品状态恢复干净 |
| 当前用户 finalizer 成功后故意失败 | 1 | 用户逆操作与 MSI 回滚均返回 0，恢复干净 |
| 安装基线 0.0.9.11 | 0 | 基线产品 installed、新产品 absent，全部文件/安全/用户状态通过 |
| 删除用户 marker 后修复 | 1 | 拒绝修复，原有文件/COM/profile/receipt 保持，随后测试脚本恢复 marker |
| major upgrade 到 0.0.10.0 | 0 | 新产品 installed，旧产品 absent，用户状态/default 与文件/安全检查通过 |
| 卸载升级产品 | 0 | 两产品均 absent，root/profile/COM 清除，保留预期 remove receipt |

`vm-rollback-upgrade-matrix-evidence.json` 中 completed、machine_failure_rolled_back、user_failure_rolled_back、missing_marker_repair_failed_closed、major_upgrade_completed、default_input_unchanged、install_tree_security_audited 均 true；failure=null。最终 base/upgrade MsiQueryProductState 均 -1。

进一步读取原始日志确认失败确实发生在目标注入点：MSI DevelopmentFailAfterMachineProfile 返回 actual error 1 并执行 RollbackInstallMachineProfile；Burn MoCurrentUserFinalizer 先返回 0，再由 MoDevelopmentFailureInjection 返回 0x80070001，随后用户/MSI rollback 均 0。

证据目录：`build/win10-evidence-clean-v1/Matrix0100`。十三份日志均按 guest JSON 内的 size/SHA-256 核对转移副本，通过。

## 产物绑定

- stage manifest SHA-256：`5B5AEDE16BC1DBE1A0495AE54A316AB3B466035EDB4F44CB8D3DEBF134DE1E56`。
- base 0.0.9.11 Bundle：`53CA5601F25A83190BB5BFCEA777CECA39C9ED77EC809AFFB1F36094C552AB2A`。
- upgrade 0.0.10.0 Bundle：`37A09099BEA90CE8161CFB5A68DFA17090B7737A78D1C8BC75BF0BC3E00E1754`。
- upgrade MSI：`4DAAD718A12A9C2405016A11007BB7CC21B65A354C99CBDD15C1A7809920BFE9`；ProductCode `{958FAED6-D2EC-4818-BAFC-E634A90CF5C6}`。
- 两包 MSI ICE、Bundle 反向解包/payload 校验通过。版本对 evidence 位于 `build/mo-win10-upgrade-pair-0911-0100-v1`；kit 位于 `build/mo-vm-matrix-kit-win10-0911-0100-v1`。
- 新版政策 39 项通过；host/guest 共享 MSI 前三段版本规则。仅第四段递增的实际 0.0.9.11 → 0.0.9.12 对被拒绝，详见 ADR 0053。

## 矩阵结束时 VM 状态与边界

矩阵结束时 VM 已卸载 Mo，Program Files\Mo 不存在，profile 未注册/启用/激活，机器 COM 缺失。用户 remove-v1-enabled receipt 是预期逆操作凭据；后续 clean 测试前要按精确已知状态处理它，不复用旧 evidence 目录。

这是一台 Win10 VM、同 payload 的有限矩阵。未覆盖 loaded-TIP 升级/重启、升级期间故障回滚、UAC 禁用/关闭、真实桌面宿主输入、候选窗/设置交互、首次切换 <500 ms、签名发行或 Win11。G3 尚未整体通过。下一步仍集中 Win10，验证真实应用输入和安装后登录/重启。

后续桌面续测重新安装了原 0.0.10.0；最新 VM 状态见 [桌面实测](WIN10-DESKTOP-EVIDENCE.md)，不要把本节矩阵结束状态当作当前状态。
