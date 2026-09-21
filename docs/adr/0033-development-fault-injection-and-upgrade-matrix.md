# ADR 0033：开发安装器故障注入与升级矩阵

## 状态

已接受（linked authoring 与一次性 VM 测试包已验证；尚未取得真实 VM 执行证据）

## 背景

clean install/repair/uninstall 只能证明成功路径，不能证明 MSI 已改变机器 TSF 状态后
确实执行逆向回滚，也不能证明 current-user finalizer 成功后 Burn 能同时撤销用户和
机器两层状态。marker 缺失的 repair、Major Upgrade 以及旧 ProductCode 清除同样不能
由反编译或单版本生命周期替代。

## 决策

1. registrar 增加唯一的 `development-test-fail-fixed` 命令。它不读写产品状态并固定
   返回 `E_FAIL`；双架构构建探针比较调用前后完整 status，证明可观察状态不变。
2. MSI 的安全公共属性 `MO_TEST_FAIL_AFTER_MACHINE_PROFILE` 默认不存在。只有 Burn
   隐藏数值变量被显式设为 1 时才传入；对应 deferred、非 impersonated、vital action
   位于 profile 变更之后、commit 之前，使 Windows Installer 必须运行既有 rollback。
3. Burn 的第二个隐藏变量控制链尾的 vital ExePackage。它只在显式设为 1 时运行，
   在 current-user finalizer 后固定失败，使 Burn 必须先逆转用户 finalizer，再撤销 MSI。
   两个变量均为 `Value=0`、`Hidden=yes`、`Persisted=no`，仅允许 WixStdBA 通过精确的
   `Variable=Value` 参数覆盖；普通安装、修复、卸载和 UI 不触发故障。
4. linked verifier 必须从反编译 MSI、Burn manifest 和 BootstrapperApplicationData
   重新证明动作顺序、条件属性、隐藏默认值、可覆盖白名单、三份 attached payload
   及其字节哈希。仅检查源 WXS 不构成通过。
5. `verify-linked-upgrade-pair.ps1` 接受两个独立 linked evidence。低版本必须小于高版本，
   stage hash、MSI UpgradeCode 和 Bundle UpgradeCode 必须相同；MSI ProductCode、Bundle
   registration id 及两个最终文件哈希必须不同。
6. VM matrix 驱动仍要求匹配实体的虚拟机哨兵、两个显式执行开关和非提升交互令牌。
   它依次验证机器故障回滚、用户故障回滚、marker 缺失 repair fail-closed、
   `0.0.1.0 -> 0.0.2.0` Major Upgrade 以及高版本卸载。每个边界核对 131 个安装文件、
   HKLM 双 COM view、TSF/profile/default 状态和两个 ProductCode 的 MSI 状态。
7. marker 缺失测试只在一次性 VM 删除精确 HKCU marker；预期 repair 失败后机器和文件
   必须保持已安装，marker 仍缺失。驱动随后只恢复固定 marker 字符串，以继续升级测试。
   任一其他失败保留现场，不猜测清理。

## 验证

- x64/Win32 registrar 均以 `/W4 /WX` 构建，固定失败返回 `0x80004005`，调用前后 status
  完全一致；静态 authoring 10 项、VM/payload/kit 策略 25 项通过。
- WiX 4.0.6 实际链接的两个 Bundle 分别为 `0.0.1.0` 与 `0.0.2.0`。反向提取确认两个
  隐藏变量默认关闭且可覆盖，MSI 条件属性存在，故障包位于 finalizer 之后，三份
  attached payload 与输入逐字节一致。
- upgrade-pair evidence 证明两版共享 MSI/Bundle UpgradeCode，ProductCode 与 Bundle id
  各自不同；测试包在 Windows PowerShell 5.1 完成完整 inventory/hash 校验。
- 本轮 `0.0.1.0` MSI/Bundle SHA-256 为
  `61543458FD91AD1E701AB0435E6B9401E8F2705D3037AB01CD761B53B2E01BC5` / 
  `48434A0F537DF4AC74CA88F85D1F1B363C68267EC626CE0F7E9D3A35A0DBF41C`；
  `0.0.2.0` 为
  `6152546AD7675AD6531E38687A23E866D89B19BE8C273210F4CFDBAC59893602` / 
  `11DC8033D4498AAB06B6CF3CA01C285745B265906CC4AB69590AB325001FF3BD`。
  stage manifest 与最终 matrix kit manifest SHA-256 分别为
  `77479B1D31310058CAD87718E0B5325398C6DBCB5C2B0BC0C1566E1D5FFC7C8F`、
  `6C6DE9439444AE0E0A16912D1AE2228C5B6A76A9B329EDCBF77B3AE73C2955F8`。
- Windows PowerShell 5.1 兼容性检查发现并修复了 `.NET Framework` 不提供
  `Path.IsPathFullyQualified` 的问题，统一使用既有 DOS 绝对路径守卫。实体开发主机在
  读取/执行 Bundle 前因缺少匹配 VM 哨兵被拒绝。

## 限制

当前没有可用 Windows Sandbox/Hyper-V 入口，因此本 ADR 不宣称任何真实安装、回滚
或升级已通过。故障开关只允许存在于 unsigned development Bundle，发行构建必须移除
或由独立 build flavor 排除。MSI ICE、重启恢复、右键提升、UAC 关闭、多用户、跨会话、
loaded TIP 与普通应用输入仍需 VM/实体测试；G3 保持未通过。
