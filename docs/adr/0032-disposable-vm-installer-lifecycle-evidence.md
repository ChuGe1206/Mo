# ADR 0032：一次性 VM 安装生命周期证据边界

## 状态

已接受（测试包与 fail-closed 策略已验证；尚未取得真实 VM 执行证据）

## 背景

linked MSI/Bundle 的反编译结果只能证明静态结构，不能证明 Windows Installer、
Burn 权限拆分、TSF 注册 API 和当前用户 finalizer 在真实系统中的组合行为。直接在
开发主机尝试会污染输入法、COM、Program Files 与卸载注册，并且失败状态难以安全
恢复。普通“复制 EXE 到 VM 后双击”的手工流程也无法证明运行的正是已核验产物，
或逐字节检查完整安装树和卸载残留。

## 决策

1. `prepare-vm-test-kit.ps1` 只接受通过 stage 与 linked evidence 核验的 Bundle、
   registrar 和 stage manifest，复制到新的 `build/` 子目录，并为六个来宾文件生成
   精确 SHA-256/size inventory。测试包保持 development-only、不可分发，且
   `install_execution_authorized=false`；准备测试包不等于授权执行。
2. 来宾初始化必须同时给出两个显式开关，并且机器 BIOS manufacturer/model 必须
   呈现虚拟化特征。哨兵绑定当前 MachineGuid、系统卷序列号、计算机名、用户 SID 和 model；
   从别的 VM 复制哨兵无效。开发主机的实体型号已验证会 fail-closed，且不创建文件。
3. 生命周期驱动再次要求两个显式开关和匹配哨兵，并拒绝提升令牌。Bundle 必须由
   普通交互用户启动，让 Burn 只为 per-machine MSI 建立提升边界；直接右键管理员
   启动不能冒充成功路径。
4. clean install 前要求 `Program Files\Mo`、HKLM 双 COM view、TSF profile、用户
   marker/journal 全部不存在。install 后逐字节验证 131 个文件，核对双 COM 路径与
   `ThreadingModel=Apartment`、profile enabled、marker 和 install receipt，同时要求
   `profile.active=false`，把“不抢默认输入法”纳入机器断言。
5. repair 必须保留同一文件/机器注册并形成 repair receipt；uninstall 后安装根、
   HKLM COM、profile、enabled 与 marker 必须消失。每个 Burn 动作只接受退出码 0，
   3010 也停止流程，避免在未重启边界上继续给出伪成功。
6. 当前 ExePackage 没有 commit callback。成功 remove 必须暂留
   `mo-user-finalizer-remove-v1-enabled`，以便后续 MSI 卸载失败时逆向恢复原 enabled
   bit；下一次 clean install 会覆盖它。该可解释 receipt 不等于零残留，发行前需在
   自有 BA 成功回调清理与保留恢复能力之间作出明确设计。
7. 任一失败停止后续动作，但保留 VM 和已产生日志；`finally` 总是尝试写出阶段退出
   码、最终 registrar 状态、Bundle/log 哈希和失败信息。脚本不在失败后猜测清理，
   以免破坏故障现场。

## 验证

- 33 项内存/fixture 测试覆盖 clean/install/repair/uninstall 精确状态、默认输入法
  变化拒绝、status 畸形输入、payload 篡改/缺失/额外文件/路径大小写/路径逃逸，以及测试包
  篡改、额外文件和额外目录拒绝；新增 ACL owner/write-like ACE、reparse 与多硬链接拒绝，
  详见 ADR 0037；不执行安装器、不提权、不修改输入状态。
- 同一策略在 Windows PowerShell 5.1 中读取真实 131-file stage contract，并对实际
  生成的测试包完成 inventory/hash 核验。
- 实体开发主机以完整授权参数调用初始化器时，因 HP 实体型号被拒绝，且哨兵没有
  落盘；即使手工伪造一个与该主机 MachineGuid、卷序列号、用户 SID 等完全匹配的
  哨兵，生命周期侧的独立虚拟硬件检查仍拒绝。缺少任一授权开关时，初始化器和
  生命周期驱动都在读取/变更系统状态前拒绝。
- 已从 SHA-256 为
  `BA87B7A2AAC46F6F333E05F91D718DCCE6E4C9F1BD6571D1DA0250EF150E39DF`
  的 linked Bundle 生成 hash-locked 测试包，但没有在本机执行该 Bundle。

## 限制

当前主机没有可调用的 Windows Sandbox/Hyper-V 入口，Windows Installer 服务也不可
访问，因此本 ADR 不宣称 install/repair/uninstall 已通过。测试包 hash 是一致性证据，
不是代码签名或来源认证。clean lifecycle 不覆盖 major upgrade、强制 rollback、
重启恢复、UAC 关闭、右键提升、多用户/跨会话、loaded TIP、普通应用输入和 MSI
ICE；这些必须使用独立干净快照。G3 保持未通过。
