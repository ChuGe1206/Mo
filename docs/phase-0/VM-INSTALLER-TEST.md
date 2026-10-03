# 一次性 VM 安装生命周期验收

本流程只用于可回滚或可直接销毁的 Windows 11 x64 虚拟机。它会真实安装、修复、
启用并卸载 Mo，不能在开发主机、日常系统或含有既有 Mo 状态的机器上运行。

## 1. 在开发主机准备测试包

先按 `installer/windows/README.md` 生成并反向核验 linked development package，
然后创建新的测试包目录：

```powershell
./installer/windows/prepare-vm-test-kit.ps1 `
  -BundlePath "$PWD/build/mo-linked-installer-new/mo-setup-development-unsigned.exe" `
  -ProbeRegistrarPath "$PWD/build/mo-windows-stage-new/stage/payload/Mo/bin/mo-tip-registrar.exe" `
  -StageDirectory "$PWD/build/mo-windows-stage-new/stage" `
  -LinkedEvidencePath "$PWD/build/mo-linked-installer-new/verification/linked-installer-evidence.json" `
  -OutputDirectory "$PWD/build/mo-vm-test-kit-new"
```

准备脚本会核对 Bundle、stage manifest、probe registrar 与 linked evidence 的哈希，
并将六个必要文件复制到全新的 `build/` 子目录。测试包本身不授权执行安装器。

## 2. 准备一次性 Windows 11 x64 VM

创建干净快照，启用 UAC，并使用普通、非提升的交互式用户登录。把整个测试包复制
进 VM。先在 VM 内明确建立只绑定当前 MachineGuid、系统卷序列号与计算机名的哨兵：

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\initialize-disposable-vm.ps1 `
  -DisposableVm -AllowDestructiveInstallerTests
```

脚本会拒绝未识别为虚拟机的硬件。哨兵默认位于当前测试用户的
`%LOCALAPPDATA%\MoInstallerTest\disposable-vm.json`；复制别的机器或用户的哨兵无效。

## 3. 从非提升 PowerShell 运行生命周期

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\run-vm-installer-lifecycle.ps1 `
  -DisposableVm -AllowInstallerExecution `
  -EvidenceDirectory "$env:LOCALAPPDATA\MoInstallerTest\evidence-clean"
```

不要右键“以管理员身份运行”测试驱动。Burn 自行提升 per-machine MSI，同时
current-user finalizer 必须保留发起用户的非提升令牌。根据 VM 策略，安装阶段可能
出现一次 UAC 确认。

脚本依次验证：

- 初始状态无安装目录、机器 COM、TSF profile、用户 marker/journal；
- install 后 132 个文件与 stage 清单逐字节一致，HKLM x64/x86 COM 路径正确，
  profile 已启用但 `active=false`，因此没有抢默认输入法；
- install/repair 后重新审计 `Program Files\Mo`：Program Files、根和全部后代 owner/DACL
  不允许普通主体写入，树中没有 reparse point，每个文件只有一个硬链接；
- repair 后文件与机器注册仍精确，用户 journal 转为 repair receipt；
- uninstall 后安装目录、机器 COM、profile 与 marker 均消失；
- 每个 Burn 阶段退出码为 0，并保留 Bundle/package 日志及带哈希的 JSON 证据。

成功卸载后会保留 `mo-user-finalizer-remove-v1-enabled` receipt。这是当前
ExePackage 缺少 commit callback 的已知设计结果，用于 MSI 卸载失败时恢复用户原
enabled bit；下一次 clean install 会覆盖它。是否增加自有 BootstrapperApplication
在 Apply 成功后回收 receipt，属于进入发行前必须决定的产品策略。

任一断言失败时停止后续动作、记录当前状态并保留 VM 供分析。收集证据后回滚快照
或销毁 VM，不要把该 VM 当作日常环境。major upgrade、强制失败回滚、UAC 关闭、
右键提升启动和多用户矩阵需要独立快照，不能由本 clean lifecycle 结果替代。

## 候选窗与设置人工验收（另一份干净快照）

生命周期驱动会卸载 Mo，因此请在另一份一次性 Windows 11 x64 快照中安装同一
DevelopmentTest Bundle，再以普通用户打开 Notepad 并手动选择 Mo；不要修改系统默认
输入法。输入 `nihao`，核对预编辑、候选窗位置、标签和至少一个实际有注释的候选。
从开始菜单打开设置中心，取消“显示候选注释”并保存：当前候选窗应随通知立即重绘，
文字和标签保持，注释消失；重新勾选后注释恢复。再用键盘和鼠标分别选词，核对上屏
仅一次。记录 Windows 版本、DPI、schema、设置文件值、候选窗截图与相关日志；在
WinUI/AppContainer 和混合 DPI 环境重复。此项仍是人工待验收，不能由 IPC/受控窗口
探针或 VM 安装驱动的成功结果替代。

另在全拼下将“启用 Emoji”取消并保存，结束当前预编辑后再次输入 `nihao`，确认
候选中没有 `👋`；重新启用并保存、结束当前预编辑后重复输入，确认 `👋` 恢复。
切换期间现有预编辑不得丢失或重复上屏，并记录设置文件的 `emoji` 值。

在独立的测试用户目录中，以同一固定短语检查学习策略：关闭“启用本地学习”后输入并上屏，
用户词典不得新增该短语；打开学习后再次上屏，应新增一次；再启用“隐私模式（暂停学习）”
并上屏，词频不得增加。每次更改后先完成当前预编辑，再开始下一次输入。使用与已安装
`rime.dll` 同源的 `rime_dict_manager.exe` 导出用户词典核对条目和频次，记录设置文件
两项布尔值及导出差异；勿记录实际私人输入内容。

## 4. 运行故障回滚与 Major Upgrade 矩阵

开发主机先分别用 `0.0.1.0`、`0.0.2.0` 链接两个全新输出。MSI 仅比较前三段，升级必须让前三段严格递增；仅第四段递增会被 host/guest 守卫拒绝（ADR 0053）。再核对升级对并生成
独立测试包：

```powershell
./installer/windows/verify-linked-upgrade-pair.ps1 `
  -BaseEvidencePath "$PWD/build/mo-linked-base/verification/linked-installer-evidence.json" `
  -UpgradeEvidencePath "$PWD/build/mo-linked-upgrade/verification/linked-installer-evidence.json" `
  -OutputDirectory "$PWD/build/mo-linked-upgrade-pair"
./installer/windows/prepare-vm-matrix-test-kit.ps1 `
  -BaseBundlePath "$PWD/build/mo-linked-base/mo-setup-development-unsigned.exe" `
  -UpgradeBundlePath "$PWD/build/mo-linked-upgrade/mo-setup-development-unsigned.exe" `
  -ProbeRegistrarPath "$PWD/build/mo-windows-stage-new/stage/payload/Mo/bin/mo-tip-registrar.exe" `
  -StageDirectory "$PWD/build/mo-windows-stage-new/stage" `
  -UpgradePairEvidencePath "$PWD/build/mo-linked-upgrade-pair/linked-upgrade-pair-evidence.json" `
  -OutputDirectory "$PWD/build/mo-vm-matrix-test-kit"
```

把 matrix kit 复制进另一份干净 Windows 11 x64 快照，按第 2 节初始化哨兵，然后从
非提升 Windows PowerShell 运行：

```powershell
powershell.exe -ExecutionPolicy Bypass -File .\run-vm-installer-matrix.ps1 `
  -DisposableVm -AllowInstallerExecution `
  -EvidenceDirectory "$env:LOCALAPPDATA\MoInstallerTest\evidence-matrix"
```

驱动依次要求：机器 profile 变更后的 MSI 故障回到全 clean、用户 finalizer 后的
Burn 故障回到全 clean、删除 marker 后 repair 必须失败且保持原安装、恢复精确 marker
后完成 Major Upgrade、旧 ProductCode 消失且新 ProductCode 存在，最后卸载回到允许的
remove receipt 状态。每个仍安装阶段都会重跑 ACL/reparse/hard-link 审计。故障变量默认均为 0，只有 matrix 驱动使用精确
`MoTestFailAfterMachineProfile=1` 或 `MoTestFailAfterUserFinalizer=1` 开启。
