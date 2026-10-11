# Win10 测试目录与缓存维护 · 2026-10-12

## 统一布局

后续开发测试只使用一个可见根目录 C:/MoDev：

| 路径 | 用途 |
| --- | --- |
| C:/MoDev/Packages/版本 | 配套开发包和该版本的验证工具 |
| C:/MoDev/Evidence/版本-轮次 | 日志与验收回执，每轮新目录 |
| C:/MoDev/Archive/Legacy/原目录名 | 既有 MoKit/MoEvidence/MoMatrix/MoProbe 等历史资料 |
| C:/MoDev/Manifests | 原路径、目标路径、迁移状态与失败记录 |

程序按 Windows 安装约定位于 C:/Program Files/Mo，用户设置/词典位于
LocalAppData/Mo；不将用户数据移到开发测试目录。

[整理工具](../../installer/windows/organize-vm-workspace.ps1) 默认只预览。
执行要求既有 disposable VM sentinel 和显式 -DisposableVm -Apply。
只识别测试目录的严格名称列表；未知 Mo 名称、Windows、Program Files、用户目录不纳入。
同盘移动保留文件，拒绝重名覆盖、路径越界、reparse tree、变化的清单以及从旧目录运行的进程。
每次迁移保留回执；重复运行不会再次移动归档。

PS7 和 Win10 PowerShell 5.1 各十项合成文件测试通过；
新 CI 步骤尚无远端结果。源快照资源策略 71 项通过，包含十九份图标/许可输入。
完整 Figma SDS MIT 声明也嵌入两架构 TIP 的 RCDATA 306，原生 fixture 通过。

## 客机访问与执行边界

VM win10 已从 saved 恢复至 running。computer-use 原生截图先后出现
FrameArrived timed out 和 window capture timed out；VirtualBox 原生截图可读。
guestcontrol 无密码 admin 登录被 Windows account restriction 拒绝。
computer-use 技能禁止通过界面代敲终端命令，因此已请求用户在客机执行工具；
没有修改密码或登录策略，不能据这些错误推断 VM 性能。

工具已复制到共享 MoEvidence/WorkspaceTools-v1，带三文件 SHA256 manifest。
成功时迁移回执回传 MoEvidence/WorkspaceGuest-v1。
**未收到回执前，不声明客机目录已整理或已验证。**

## Cargo 缓存

2026-10-12 从仓库根目录执行 cargo clean，确认默认 target 位于仓库且没有相关运行进程。
完成输出：Removed 20338 files, 3.1GiB total。
保留 build 中的历史安装包、stage、运行库、用户数据与测试证据。
记录位于 build/mo-cache-maintenance.json；后续必要验证可能重新生成缓存。

用户要求已写入 AGENTS.md。每日 03:00 的线程维护自动化 mo-cargo 检查：
缓存超过 8 GiB、磁盘不足 20 GiB 或七天未清理时，空闲且 target 路径验证通过才清理；
有构建/测试/程序/打包任务则跳过。无变化保持安静，不为预热缓存重新构建。


## 配套 0.0.14.0 开发包

fresh release 源快照、x64/Win32 严格编译/ABI、132-file stage 通过；
设置 EXE 的六页合成预览已从实际 staged 二进制生成。
实际 MSI/Burn 反解绑定与 MSI ICE 通过，维持 unsigned DevelopmentTest，不是发行包。
stage manifest：5FB9D6E12A1D68FEC97AB78AF94BE0E34F39AC463658BBED1AECD4955B6FB546。

实际 stage 的七组预编译资源 golden、无输入准备和 Actor 分页通过；
x64 fault 首键 N 47 ms 未消费失败，没有执行到 Win32 fault，不重复此回归。
47 ms 不能据此当作超过 50 ms，根因未定。G2/G3 及历史负向结果保留。

本机没有执行安装器，客机安装、目录整理和真实宿主仍需要回执。
升级 kit 仅接受已验证的 0.0.11.0 基线；不同安装版本将被 driver 拒绝。

### 目录整理命令（虚拟机 PowerShell）

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "\\vboxsvr\MoEvidence\WorkspaceTools-v1\organize-vm-workspace.ps1" -DisposableVm -Apply
```


### 配套升级入口（虚拟机的普通权限 64-bit PowerShell）

关闭 Mo 设置和测试 Broker，保留原默认输入法及已启用但未活动的 Mo。
先确认上一步目录已整理；下列包和证据目录须尚未存在。
UAC 由用户处理，driver 会核对 VM sentinel、当前安装版本及升级后全部载荷/设置状态。

```powershell
Copy-Item -LiteralPath "\\vboxsvr\MoEvidence\Package0140-v1\kit" -Destination "C:\MoDev\Packages\0.0.14.0" -Recurse
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "C:\MoDev\Packages\0.0.14.0\run-vm-changed-payload-upgrade.ps1" -DisposableVm -AllowInstallerExecution -EvidenceDirectory "C:\MoDev\Evidence\Upgrade0140-v1"
Copy-Item -LiteralPath "C:\MoDev\Evidence\Upgrade0140-v1" -Destination "\\vboxsvr\MoEvidence\ChangedUpgradeGuest0140-v1" -Recurse
```

失败也保留并回传完整回执。若当前安装不是 0.0.11.0，停止并按实际版本准备升级 pair，
不能绕过 driver 或重复在已有证据目录运行。升级成功后再做 Notepad/x86 WordPad 输入、
皮肤即时更新、独立表情、Esc 返回和多窗口设置冲突验证；暂不宣称这些已通过。
