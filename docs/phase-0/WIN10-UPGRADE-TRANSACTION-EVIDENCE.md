# Win10 升级事务最终状态完成门

日期：2026-10-08。继续 develop / Win10；本轮修正独立变更载荷升级 driver 的
最终状态读取失败仍可能完成的缺陷，未执行真实 VM 升级。

## 修复

旧 driver 在全部主检查后先置 completed=true；finally 读取 registrar 失败时
只记录 error，不撤销完成。现在抽出内部事务流程，最后的 registrar 读取必须
成功且严格匹配 Installed，才标记完成。

回执改为 format 2，新增 final_state_verified 与 final_state_failure。
读到的异常状态保留；若先有安装或其他检查错误，再出现最终读取错误，
failure 保留首个错误，final_state_failure 单独记录最后的错误。
两个错误均不会使 completed 置真。序列会重置旧成功标记，拒绝不明确的
安装返回值及非 0 exit（含 3010）。

固定生产 operations 仍在 VM、用户权限与 kit 校验之后创建；命令行没有
替换 callbacks 的参数。旧/new MSI、manifest、ACL、COM、设置与默认保护保留。
没有改 Rust/TIP 产品代码、输入 deadline 或安装包二进制。

## 验证与边界

- PowerShell 7 与 Windows PowerShell 5.1 各 21 项事务测试通过。
  覆盖存在/缺失设置的成功、预检查/旧载荷/读取/安装调用失败、1603/3010、
  缺失或多个 exit、升级后检查失败、设置创建/漂移、默认改变、最终读失败、
  最终 profile.active=true 和主/最终双重错误。
- 两项测试从真实 driver AST 提取 operations 源代码，通过内存替身执行，
  检查新旧 MSI、不同 payload contract 和带空格路径的 Burn 参数、
  -Wait/-PassThru，以及最终读取失败。包含在各 21 项中；不是真实安装。
- 原 kit 策略 34 项通过。WinPS 5.1.19041.6456 校验实际 v3 kit 和四份最终
  helper/driver 源码哈希；两项 host 执行拒绝门通过，未创建证据目录。
- 三脚本 AST、十非空 CI PowerShell block 和两个事务 CI 入口检查通过。
  两种 shell 的测试都接入 CI；远端 CI 未核验。

所有模拟输入为合成状态，未读写真实设置、注册表或启动安装器。
这些检查只验证事务编排和错误传播，不能升级为 VM MSI/Burn 成功、
loaded TIP、登录/重启、真实桌面输入或 G2/G3 完成。

## 最终包与封存

本轮使用 build/mo-vm-changed-upgrade-kit-0110-0120-v3；v2 保留为前轮记录。
v3 manifest SHA-256：7439A954101DF9EA522AA464A91807E841585C24B779095EDA1CC901ECBB390B。
产品仍是已构建的 0.0.12.0 unsigned DevelopmentTest；本轮仅更新 kit 的脚本。

[Git 摘要](evidence/WIN10-UPGRADE-TRANSACTION-20261008.json)：
build/win10-evidence-clean-v1/ChangedUpgradeTxn0120-v1；35 封存文件、
280 外部输入。
manifest SHA-256：9C15C62ED8CF675135C66FC3C11C903E08D975BAC1A42082AF75A79A352848BB。
包含旧 driver 源码、新源码/CI、v3 kit、原始测试日志及 WinPS harness。
两份历史封存 manifest 和旧/new stage 全部条目再次比对，未改。

## VM 状态及手动执行入口

Computer Use 重新识别 win10 窗口后，截图仍报
FrameArrived timed out: timed out waiting on channel。
无密码 guest command 的 2026-10-07 拒绝保持为历史证据，本轮未重复登录。
最近已验证的 VM 仍是 0.0.11.0 / ABI v2，本轮没有安装/输入/重启。

自动通道不可用时，需要 VM 用户手动运行一次已经准备好的测试。
computer-use 技能禁止通过 Windows UI 自动输入终端命令；UAC 由 VM 用户处理。
在已初始化的 disposable VM，以原普通权限用户打开 64-bit Windows PowerShell：

~~~powershell
Copy-Item -LiteralPath '\\vboxsvr\MoEvidence\ChangedUpgradeTxn0120-v1\kit' -Destination 'C:\MoChanged0120v3' -Recurse
powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\MoChanged0120v3\run-vm-changed-payload-upgrade.ps1 -DisposableVm -AllowInstallerExecution -EvidenceDirectory C:\MoChangedEvidence0120v3
~~~

两个本地目录都必须尚未存在。先关闭测试中的 Mo Broker/settings 并完成设置
写入；保留默认输入法、原 sentinel 和旧安装。driver 自己会拒绝不符的状态。
如果检查失败，保留输出和 upgrade-state.json；不要将失败写成升级通过。

将本地证据文件复制回现有共享目录，便于主机核验：

~~~powershell
Copy-Item -Path 'C:\MoChangedEvidence0120v3\*' -Destination '\\vboxsvr\MoEvidence\ChangedUpgradeGuest0120-v1'
~~~

该 guest 输出目录与封存目录分开，不能覆盖封存文件。
接下来核验真实升级和设置保全，再补安装态多窗口设置及 x64/x86 宿主。
时延专项维持低优先级，严格时限和完整 G2/G3 仍开放。
