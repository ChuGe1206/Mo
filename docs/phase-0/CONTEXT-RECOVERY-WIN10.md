# 会话恢复与 Win10 验证交接

更新：2026-10-27。用户优先验证 Win10；其当前主机也是 Win10。虚拟机已准备，
凭据通道已恢复；密码不写入源码、日志或交接文档。

## 2026-10-27 0190 续验（最新入口）

- [续验记录](WIN10-0190-CONTINUATION-20261027.md)：保留的两版 stage 276 文件、升级 kit 14 文件全哈希通过。
- 已启动 VM，VBox 截图确认锁屏；Guest 可用但 Explorer 会话 0、LogonUI 1；等待用户手动登录。
- computer-use 两次截图超时后停止捕获；未操作认证/安全界面、未发起安装或重复复制。
- 最后验证安装态 0180，本轮未复核实际版本；登录后先查实际基线和客机 kit，再使用全新升级证据目录。
- 没有启动构建或测试；0190 的真实物理按键及其他未完成验收仍待安装后验证。

## 2026-10-23 过期 build 产物退役（维护记录）

- [退役记录](BUILD-ARTIFACT-RETIREMENT-20261023.md)：本轮删除 803 项，回收 27.99 GiB 逻辑文件占用。
- 除编译输出外，删除已被替代的旧安装包、解包副本、stage 载荷和重复源码归档。
- 当前 0180/0190 的 stage/linked/kit/pair、当前运行库、工具链、日志/清单及共享证据保留。
- 165 个旧根目录有 RETIRED.json；下文旧包和旧 probe 路径仅表示历史身份，不能再直接运行。
- 所有删除路径复查不存在，保护哈希一致；没有启动构建或 VM，安装状态未变。

## 2026-10-23 build 缓存清理（首轮）

- [清理记录](BUILD-CLEANUP-20261023.md)：删除 175 个可再生成工作目录，回收 9.52 GiB 逻辑文件占用。
- 清理旧 Rust/C++ 中间缓存、被替代运行库 compile 和已归档词库的重复解压目录。
- stage、安装包、升级 kit、当前运行库、ABI 探针、证据及用户数据保留；关键哈希复核通过。
- README/AGENTS 补充打包后回收规则；默认 target 不存在，未启动构建或 VM。
- 实际安装基线仍 0180，0190 v2 待后续核验并升级；本轮未改变安装状态。

## 2026-10-14 全键审查0190（开发入口）

- [本轮记录](WIN10-KEY-AUDIT-0190-20261014.md)、ADR0065、源码34850e7；空闲标点插入失败已修复。
- 四Rust门禁202＋1、32768 VK情形、真实 Rime 普通/Shift/快捷键/斜杠、双架构 TSF/IPC/恢复通过。
- 唯一 stage keys-0190-v2、linked keys-0190-v2、kit0180-0190-v2；v1保留不安装。
- guest C:/MoDev/Packages/0.0.19.0-v2 14文件已核验。复制返回20后原回执完成，禁止重复复制。
- 目前实际0180；已自主重启卸载旧 DLL，截图确认锁屏/run level2，等待用户登录；安装器未调用。
  后续检查桌面/实际基线，首次 driver使用新 Upgrade0190-v1；安全/认证界面用户处理。
- 默认target清零，约0.98GiB逻辑空间释放；封存包/证据保持，不为预热重建。

## 2026-10-14 实际 0180（历史基线）

- [安装态记录](WIN10-INSTALLED-0180-20261014.md)：实际版本 0.0.18.0，17→18 v3 已完成，exit 0。
- 普通权限 driver、用户确认安全提示；47 进程实时审计和配套载荷/机器 COM/权限/设置保全通过。
- 已安装双架构 ABI、中英各 7 幅预览通过，14/14 与 0170 像素一致；现有真实 Broker 保留。
- 用户新记事本 nihao 点击展开后反馈“nihao 保持紧凑”；真实文档注释展开还未独立验收。
- 原材料 Package0180-v2/upgrade-evidence-v3；安装态 installed-validation-evidence-v1。
  旧 upgrade-evidence-v2 取消保留；禁止重跑 17→18。后续从实际 0180 基线继续。
- 默认 target 不存在，本轮未构建；下列旧版本/取消/待升级叙述仅表示其记录时状态。

## 2026-10-13 紧凑注释展开0180（历史）

- [本轮记录](WIN10-COMPACT-EXPANDED-0180-20261013.md)：无注释不展开，有注释完整换行，六词291×81。
- Figma六款Expanded组件/实例同步，评审1553:1894；最终枚举源码22幅Win10预览通过。
- 四Rust门禁199+1、固定1.97.1双架构原生/IPC/恢复通过。
- 唯一stage expanded-0180-v3、linked expanded-0180-v3、kit0170-0180-v2；v2stage/v1kit不使用。
- Guest C:/MoDev/Packages/0.0.18.0-v2已14文件核验，session1/Broker0/设置0；自主重启且用户处理桌面。
- Upgrade0180-v2审计UAC返回取消，installer_invocation_attempted=false，实际安装保持0170。
  回执已回传Package0180-v2/upgrade-evidence-v2；待用户选择是否重发，不复用原证据目录。
  下一次使用Upgrade0180-v3并检查实际基线；安全提示需用户处理。
- 20:14默认target清零，释放约0.94GiB逻辑空间；封存包和证据保留。

## 2026-10-13 输入修复与0170实际验收

- [修复记录](WIN10-PREEDIT-0170-20261013.md)与 ADR 0064：OEM/Shift/系统键隔离，协商位置 metadata，
  TSF caret、候选光标/分段、翻页 tooltip 已实现。四 Rust 门禁及双架构 fake TSF/IPC/fault 通过。
- Win10 renderer 20 BMP、exit 0、299×60。0170 v2 fresh stage/MSI/Burn/ICE、pair/kit 已通过；
  guest C:/MoDev/Packages/0.0.17.0-v2 已复制并全哈希核验。
- [实际安装态](WIN10-INSTALLED-0170-20261013.md)：19:14 普通权限 driver 完成 16→17，exit 0；
  载荷/机器 COM/安装权限/设置/默认输入法/finalizer 全部通过，审计 53 个进程。
  已安装双架构 ABI、中英各 7 幅预览通过，14/14 与安装态 0160 像素一致；现有真实 Broker 保留。
  用户反馈光标、键盘翻页、标点“均正常”；随后要求缩紧展开态，下一步改候选详情布局。
  用户授权以后自主 VM 启停/重启/测试；安全提示仍由用户处理。关机后临时共享映射须恢复。
  唯一升级回执 Upgrade0170-v2 已回传 Package0170-v2/upgrade-evidence-v2；不得重复运行16→17。
- 唯一 kit：build/mo-vm-changed-upgrade-kit-0160-0170-v2；共享 Package0170-v2。
  v1 保留不用于升级。用户有限文档输入验收通过，G2/G3、50/400 ms 不变。
- 默认 target 于 09:23 清为 0，释放约 1.45 GiB 逻辑空间，封存包/证据保持。

## 2026-10-13 续测

- [本轮记录](WIN10-CONTINUATION-20261013.md)：实际安装 DLL 双架构基础 ABI 通过，保留真实 Broker。
- 用户确认完整候选、鼠标翻页、笑脸面板和 Esc；键盘编辑光标、翻页说明和标点残留待修复。
- 审计 worker 补强退出码、具名完成预算和句柄释放，双 shell 各 30 项通过；Rust 四门禁通过。
  新封存 kit v4 仅含脚本更新，没有重复安装；当前 VM 仍 0160，v3 升级证据保持。
- GUI capture 两次超时后停止，真实键盘/鼠标反馈由用户提供，不作为全量自动桌面验收。

## 2026-10-12 实际安装 0.0.16.0

- [安装态记录](WIN10-INSTALLED-0160-20261012.md)：20:40 普通权限 driver 完成 15→16，退出码 0；
  配套载荷、机器 COM、安装 ACL、设置及默认输入法保全、最终用户状态全部通过。
- 新只读管理员复核解决当前用户模块读取权限；仅对 OS 明确的 ANTIMALWARE_LIGHT
  且双架构 TIP NotSigned 分类，不能按名字跳过。ADR 0063；format 4 保存实时可信回执。
- 当前 kit 为 build/mo-vm-changed-upgrade-kit-0150-0160-v3；guest 同版本 -v3 目录；
  回执 Upgrade0160-v1、InstalledUI0160-v1 已回传 Package0160-v3。
- 实际安装 EXE 中英各 7 幅、哈希绑定封存 stage，14/14 像素一致；仅合成 profile。
  两 shell 各 14 扫描＋26 审计＋22 事务、PS7 kit 34 项通过，新审计测试已接 CI。
- 用户手动处理 UAC；先前拆行/取消失败证据保留，没有修改安全设置或重复调用安装器。
  实际文档输入、翻页/表情交互、加载宿主矩阵及升级后重启仍待验收；G2/G3 不变。
- Figma 299×60 同步完成；默认 target 空，本轮没有 Rust 重建。下列历史记录不是当前安装版本。

## 2026-10-12 用户确认布局与重启后检查

- 用户确认原生 nihao 截图，Figma 参考实例 207:1556 已同步为 299×60；
  六 Compact 变体、页号标签及说明更新。详见紧凑设计文档和同步 JSON。
- 用户已重启 VM；普通用户 session 1、非提权，当前扫描没有观察到已加载 Mo TIP。
  模块读取失败仍有 DefenderSessionHelper、ctfmon、ChsIME；普通权限只读保护查询
  显示 Defender 为 ANTIMALWARE_LIGHT，其余 NONE。不能按进程名直接跳过。
- 已准备并发起只读管理员模块审计，UAC 等待用户确认；不执行安装或修改安全设置。
  guestcontrol 返回 20 的启动结果保留；随后 consent 进程仍在，尚无 audit.json。
  安装仍为 0.0.15.0，0.0.16.0 配套 kit 已封存，旧扫描门禁保持。

## 2026-10-12 紧凑 UI 续测入口

- [本轮证据](WIN10-DENSE-UI-20261012.md)：默认候选按内容宽度，nihao 五词客机 299×60；
  窄屏自动换行，超高单词按像素滚动，设置 960×620 并保留中英文可读性。
- fmt、两套 strict Clippy、193＋1 测试、双架构原生/fake IPC、Win10 候选及中英文
  各 7 幅设置通过。Figma 可编辑 v3 候选/设置评审板已同步。
- 0.0.16.0 unsigned DevelopmentTest 配套包、升级对和带 preflight 的 kit 已封存共享；
  客机仍为 0.0.15.0。扫描五个已加载宿主及三个读取失败，拒绝安装；待用户保存后注销/重新登录。
  保护未保存文档；后续 UAC 用户确认，G2/G3 及首键负向证据保持。

## 2026-10-12 旧宿主 DLL 续测

- [排查记录](WIN10-TIP-HOST-0150-EVIDENCE.md)：旧记事本 PID 868 从 10 月 4 日运行，
  映射大小与 0.0.11.0 一致；新合成记事本 PID 8428 已加载 0.0.15.0 大小，
  磁盘载荷及 Bamboo/横排设置正确。explorer/conhost 也保留旧 DLL。
- 升级 preflight 新增有明确覆盖边界的只读宿主检查，已加载或已确认当前用户的模块
  读取失败时拒绝；format 3 保存扫描。原封存 kit/安装包不修改。
- PS7/WinPS5.1 各 10＋22 合成检查通过，新扫描在 Win10 实际拒绝已加载宿主，
  没有调用安装器或终止进程；远端 CI 待回执。原未保存文档保留。
- computer-use 截图超时/点击坐标不可用，只有 VBox 截图及可访问性读取可用；
  新窗口已加载新版不等于皮肤交互通过。下一步保存后重开旧文档，必要时注销以卸载 shell 旧 DLL，
  再验候选/翻页/Emoji；G2/G3 不变，未新增 Rust 构建缓存。

## 2026-10-12 设置布局及 0.0.15.0 续测

[本轮记录](WIN10-LOCALIZED-0150-EVIDENCE.md)：源码 cf13a1a；FontRole 与共享侧栏/底部记录。
192＋1 测试、fmt、两套 Clippy、实际 release 14 幅像素对照通过；首轮偏移失败保留。
Win10 合成语言/偏好/CLI/覆盖四组通过，原安装与真实设置不变。
0.0.15.0 v2 配套包、ICE、14→15 pair/kit 已核验，已复制 C:/MoDev/Packages/0.0.15.0。
用户确认关闭设置并切回原输入法后，普通权限 driver 已完成 0.0.14.0→0.0.15.0 升级，
exit 0，载荷/ACL/机器 COM/设置/默认输入法/最终状态和两份日志身份通过。
新证据 C:/MoDev/Evidence/Upgrade0150-v2 已回传；启动 guestcontrol 返回 20，
但唯一 driver 实际成功，未重启安装。当前 VM 0.0.15.0，未代操作提权提示。
从实际安装 EXE 的合成呈现/中英切换通过，7/7 与此前客机 staged 图像一致。
下一步真实宿主/候选/表情/即时重绘；设置与测试 Broker 已关闭。
首轮 v1 包不用于升级；完整宿主/首键/G2/G3 边界不变。

## 2026-10-12 质量与文案续测

README/AGENTS 已加入开发质量执行标准和审核清单；[审查记录](CODE-QUALITY-20261012.md)
及 ADR 0061 说明本轮常量/类型/lint/失败退出码与 97 键中英文资源。
局部文案覆盖无需改 Rust；当前窗口按钮即时切换，启动语言通过独立偏好/CLI。
190 运行时＋1 doc、fmt、默认/诊断 Clippy、71 staging、双架构 fake 回归及语言预览通过。
本轮源码未进 VM 0.0.14.0，不改 engine/IPC 字节、50/400 ms 或 G2/G3。
下一步配套新包与安装态 UI 验证，继续保留首键负向证据。

## 2026-10-12 缓存与 VM 目录续测

[目录/开发包记录](WIN10-WORKSPACE-20261012.md)：用户要求统一 Mo 测试目录，并定期 cargo clean。
已清理默认 target 20338 文件/3.1 GiB，维护约定进入 AGENTS，每日 03:00 检查自动化
mo-cargo 已建立；有活动构建/测试时跳过，保留封存材料。

新目录约定 C:/MoDev/Packages、Evidence、Archive/Legacy、Manifests。
整理工具的 PS7/WinPS5.1 各十项合成检查通过，源快照资源策略 71 项通过。
VM 从 saved 恢复，原生捕获仍超时，VBox 自带截图可读；无密码 guestcontrol
受账户限制拒绝。用户于 2026-10-12 确认已整理；WorkspaceGuest-v1 回执核验通过，
17 个来源/归档路径符合严格规则且全部 moved，completed=true、failure=null。
回执字段副本见 evidence/WIN10-WORKSPACE-MIGRATION-20261012.json，原文件保留在共享目录。结论依据工具回执
与用户确认；凭据通道恢复后也直接核验 17 个旧来源不存在、归档目标存在。

0.0.14.0 fresh stage 已构建，MIT 声明嵌入 TIP；实际 MSI/Burn/ICE 通过。
真实 stage golden/准备/Actor 通过，x64 fault N 47 ms 未消费失败，
Win32 fault 未执行；不扩大复跑、不改变 50/400 ms 或 G2/G3。
共享 Package0140-v1/kit 再次完整核验通过。已请求用户在普通权限 64-bit PowerShell
运行 0.0.11.0 → 0.0.14.0 升级入口并回传 ChangedUpgradeGuest0140-v1；
现已收到并核验升级回执：用户确认 UAC 后 exit 0，载荷/ACL/机器 COM、设置/
默认输入法保全和最终 Installed 状态通过，两份安装日志哈希一致。当前 VM 已安装
0.0.14.0，见 WIN10-PACKAGED-0140-EVIDENCE.md；真实桌面/loaded-TIP/重启未执行。


## 最新交接：2026-10-12

UI 按用户认可的 Figma 设计实施，开发仍在 develop，远端推送已获授权。
见 [原生 UI 记录](../design/MO-NATIVE-UI-20261012.md) 和 ADR 0060。
当前源码有新的候选页元数据、28 项符号动作、外观设置/快捷保存协商；
默认旧设置编码保持，非默认外观文件拒绝被旧版读取，必须配套更新三个二进制。

Rust 全套、双架构严格编译和合成界面通过；fake 首轮 Win32 fault 首键失败后
单架构复跑通过。真实 Rime IPC/表情/pool 双架构通过；x64 基本 TSF 复验通过，
故障恢复首键 N 62 ms 未消费；Win32 基本 TSF N 16 ms 未消费。原始日志
build/ui-20261011/rime-{final,recheck,win32}.log 保留，后续不能宣称已修复。

以上为原始 UI 实施阶段记录；后续已完成 0.0.14.0 配套打包及 VM 升级（见上节）。
设置文案改造为更晚的源码变更，尚未重新打包。后续验证 Win10 VM/真实应用 UI；
不循环首键测试阻塞。
此前 G2/G3 和 50/400 ms 状态保持。


## 恢复基线

工作目录 `E:\ChuGe\CodeProject\101_ProjectCollection\Mo`。恢复时位于 main，HEAD `9cc5d3b`；此前未提交成果已整理为 `8f43f33` 并推送 origin/develop。当前在 develop 开发，main 保留为后期正式版打板基线。后续已有远端推送授权，不能重置用户成果。ADR 0046–0051 和代码共同说明已实现空闲会话设置替换、MSI x86 COM 路径、静态 MSVC runtime、候选注释/标签、Emoji 偏好、会话学习/隐私策略。已生成 132-file stage 与开发态故障注入安装包；生成产物位于忽略的 build 目录，不代表发行版本。

已有本机 Rust fmt/clippy/workspace 测试通过；G1 本机证据闭环，G2/G4 部分通过，G3 未通过。真实 librime process_key 偶发超过 50 ms 的历史负向证据仍有效；不能通过放宽 deadline 或有限成功样本取消该风险。真实桌面宿主、完整安装矩阵和发行签名仍需证据。

## VM 与操作记录

- VirtualBox VM `win10`，UUID `eefd1936-2309-40b0-b33e-70b6b8c0bff2`，Win10 22H2 x64 build 19045，4 GB / 2 CPU。
- 已创建安装前快照 `before-mo-win10-validation-2026-09-29`；随后安装 Guest Additions 7.2.20。后来存在用户创建的后续快照，继续工作必须先查当前状态，不盲目恢复。
- 当前账户显示 admin，原用户 profile 路径 `C:\Users\34189`。blank-password guestcontrol 登录受限，采用 VirtualBox 键盘控制和共享文件夹测试。
- CLI `C:\Program Files\Oracle\VirtualBox\VBoxManage.exe`。无图形宿主窗口启动可用 headless；当前共享 MoEvidence 可写，MoKit099/MoLinked099 只读。
- 共享 Z: 映射 MoEvidence，Y: 映射 MoLinked099。键盘命令及屏幕更新可能延迟，应确认提示符和窗口焦点后发送下一操作。

## 已定位并处理的阻塞

详见 ADR 0052：GUI Bundle 未等待真实进程退出；SYSTEM TSF category 缓存回读；普通用户 TSF enabled 缓存回读。保留旧包失败证据，并用新的 stage/linked 包分别记录后续结果。

0.0.9.9 失败发生在 Burn 用户 finalizer，MSI 安装本身已返回 0，随后 Burn 完成 MSI 回滚。诊断时曾直接安装同一 MSI，再运行修复后的 registrar 验证事务；必须卸载诊断 MSI后才运行 clean lifecycle。

当前用户六项事务测试 2026-10-03 全部 exit 0，证据 `build/win10-evidence-clean-v1/enum-finalizer-transaction.txt`。0.0.9.10 三阶段虽都返回 0，但空安装根目录残留使完整 harness 失败；进一步修复 commit 后空目录清理。0.0.9.11 完整 clean install/repair/uninstall 已通过，详见 ADR 0052 和 `build/win10-evidence-clean-v1/V0911/vm-lifecycle-evidence.json`。该轮完成时 VM 已卸载 Mo，仅保留预期 remove-v1-enabled 用户逆操作 receipt；下一轮 clean/matrix 测试前须明确处理该测试 receipt，不能直接复用已存在的 evidence 目录。

## 验收边界

开发态 unsigned 包只供隔离 VM。Win10 是当前优先验证目标，Win11 尚未验证。G3 完整验收未完成，当前项目仍不能作为日常输入法发布。

## 当前已通过与下一步

| 检查 | 结果 |
| --- | --- |
| x64/Win32 严格编译与 registrar/ABI probe | 通过 |
| VM harness policy | 39/39 通过，补齐 MSI 前三段升级版本守卫 |
| 用户 finalizer 六项事务及逆操作 | 通过 |
| 0.0.9.11 MSI ICE、Bundle/payload 哈希校验 | 通过 |
| Win10 clean install/repair/uninstall | 完整通过，三次 exit 0，未切默认输入法 |
| Win10 六阶段回滚/升级矩阵 | 0.0.9.11 → 0.0.10.0 完整通过 |
| Win10 x64 记事本与设置 | 修复已随 0.0.11.0 打包，x64/x86 记事本及 x86 写字板有限通过；完整真实词库回归仍失败 |
| Win10 其余桌面宿主、loaded-TIP/登录重启 | 待验证 |

Win10 六阶段矩阵已完成，原始证据 `build/win10-evidence-clean-v1/Matrix0100`，详见 [Win10 安装实测](WIN10-INSTALLER-EVIDENCE.md)。修复了 MSI 仅比较前三段而旧 verifier 按四段比较的缺口（ADR 0053）；以前仅第四段递增的包对不能作为 MSI major-upgrade 证据。下一步继续集中 Win10，验证普通桌面宿主输入、候选窗、设置切换、loaded-TIP 升级和登录/重启。若 VM 需提权，应在 Alt-Tab 确认选中盾牌 consent 窗口后释放 Alt，等实际 UAC 画面出现，再分次发送左箭头和 Enter；将窗口选择/键盘事件挤在同一串容易失焦。不要为此修改系统安全策略。

## 2026-10-03 桌面续测交接（0.0.10.0 历史轮次）

详见 [桌面有限实测](WIN10-DESKTOP-EVIDENCE.md) 与 ADR 0054。真实 Win10 Notepad 揭示 test/key 低字 repeat count 不同导致重复派发；缓存匹配已正规化，scan/flags/真实 auto-repeat 位仍保留。旧 DLL 被新回归拒绝，修复 DLL 通过；双架构 fake 全流程通过。VM 修复副本精确两次提交和繁体/深色 UI 设置通过。

完整真实词库两次 default/一次 trace 首键超时，未放宽 deadline；trace 在等待响应头约 50.2 ms 超时，没有失败 dispatch Actor 完成计时，不能断言根因。VM 快速切回后的首轮 ASCII fail-open 也保留。

**前轮 VM 已重新安装原 0.0.10.0；本轮后续又升级至 0.0.11.0。** 测试原 DLL已恢复，完整 payload/ACL/COM 核验通过，默认 override 未改；Microsoft 拼音为前景，测试窗口关闭。用户默认设置文件由测试创建并保留。该历史轮结束时修复尚未打包；其后已生成并安装包含 ADR 0054 的 0.0.11.0，见下节。原恢复/安装矩阵证据不覆盖新源码。

## 2026-10-03 0.0.11.0 后续交接

修复已进入新包，变更载荷升级、一次重启后安装状态及 x64/x86 记事本、x86 写字板精确回读有限通过。当前 VM 保留 0.0.11.0；新旧状态以 [0.0.11.0 续测证据](WIN10-PACKAGED-0110-EVIDENCE.md) 为准。主机失败 dispatch 已取得 Actor 完成计时（约 1.31/1.67 秒），直接探针缩到 native process_key；后续键也出现 252 ms。具体组件与 CPU/I/O 原因未定位，50 ms deadline 未放宽，G2/G3 不升级为整体通过。

## 2026-10-03 native 组件诊断后续交接

[组件延迟证据](WIN10-NATIVE-LATENCY-EVIDENCE.md) 保存完整复现路径。只在主机隔离 runtime/source、编译测试数据与 trace TIP/Broker 中工作，没有更改产品或操作 VM。长耗时已细分到 Prism、词表和用户词典查询；CPU/进程缺页计数支持继续检查等待与映射，但不能证明硬缺页/磁盘/Defender。配置消融有明显运行顺序/缓存波动；无输入文件预读耗时 5.8 秒而首键仍 65 ms，未作为修复。3 组首次真实 TIP 失败均保留，第二次完整 probe 在同一 Broker 中均通过；严格首键验收仍失败。当前 `build/runtime-component-trace-v1/dist` 是 v3 诊断 DLL，不可作为 stage 输入；生产 `mo-runtime-learning-v1/dist` 哈希未变。所有 owned 主机诊断 Broker 已结束。

## 2026-10-03 映射页/Actor 后续交接

继续集中 Win10，详见 [映射页及 Actor 证据](WIN10-MAPPED-PAGE-EVIDENCE.md)。当前隔离 DLL 在 `build/runtime-component-trace-v1/dist/lib/rime.dll`，只是诊断 v4；生产 `build/mo-runtime-learning-v1/dist/lib/rime.dll` 未被覆盖。直接 API 的映射准备改善在完整 TIP 路径不成立，启动到 Broker 就绪 7.1 秒，首次真实 probe 仍失败。新增 `crates/mo-rime/examples/actor_latency_probe.rs` 用于有 marker 的合成对照，严禁把这个诊断 DLL 打入包。VM 沿用上一轮 0.0.11.0，本轮未操作。后续优先定位候选懒加载、用户词典和代码页等待，再测试严格 50 ms；Win10 VM 其余桌面验收项仍待做。

## 2026-10-03 代码页/启动分项后续交接

详见[代码页与映射页组合证据](WIN10-IMAGE-PAGE-EVIDENCE.md)。生产 runtime 与安装载荷未变，50 ms key/400 ms activation 未改。Lua filter 消融的四次完整 TIP 首轮均失败；单独准备代码页也保留 135 ms 首键失败。`FD5229…` 隔离 DLL 的 mapped+image 条件四次首轮完整通过（trace x64 两次、默认 x64/Win32 各一次），但 Broker ready 为 1.866–2.431 秒，不能作为产品修复。

增加启动计时的 DLL 为 `build/win10-startup-components-v1/dist/lib/rime.dll`，SHA-256 `E6B85279359F11DD51F2D52ABF7121AB106C8972F16D31540DB854A4F484D08D`；先前 `build/win10-image-pages-v1/dist` 的 `FD5229…` 保持原样，旧 `runtime-component-trace-v1/dist` 仍是 v4。当前 `runtime-component-trace-v1/compile` 则生成了新启动诊断 DLL，继续时不可仅凭目录名判断版本，更不能用于 stage。

新目录 Actor prepare 1.53/5.42 秒、复用目录 334/352 ms，主要长区间细分到 Dictionary::Load 与 UserDictionary::Load；运行顺序/缓存仍是混杂因素。新 DLL 在复用合成目录的默认 x64/Win32 完整探针首轮和第二轮均通过，但 Broker ready 又出现 1,702/495 ms，冷启动/首次拉起仍未闭环。保存的 `native/librime/diagnostics` 补丁已在独立副本上应用并核对，构建成功。若 MSBuild 报 PATH/Path 重复键，使用本轮成功的无节点复用命令，不改系统 PATH。

原始日志/源码/两版 DLL/脚本在 `build/win10-evidence-clean-v1/ImagePages-v1`，由 manifest 锁定。本轮只读取过 VM 截图，未改变其安装或增加桌面输入验收；VM 仍为 0.0.11.0。owned 主机诊断进程均已退出。下一步集中 Win10，细分 Db::Open 和词库映射准备，测 fresh/existing profile 与首次拉起；随后补齐已注册宿主、loaded-TIP、登录矩阵。G2/G3 不升级为整体通过。

2026-10-04 收尾：Rust fmt/Clippy/workspace tests 通过。Cargo 重建了开发态 `target/debug/mo-broker.exe`（当前 SHA `277CEE…`），原探针用的是 `65B3BA…`，两者身份见 Git 结果汇总；不得把新二进制计入历史探针。本机原始归档文件校验通过。

## 2026-10-04 DB 打开与共享 prebuilt 后续交接

详见 [用户词典打开证据](WIN10-DB-OPEN-EVIDENCE.md)。新增诊断 Env/file 转发、DB::Open 与映射分项补丁，以及 marker 守卫的共享 machine prebuilt Actor 模型。八个 Actor 进程两键均通过；shared existing 的日志复用控制使首个 DB::Open 从 106–221 ms 降到 2–3 ms，该区间 Sync 从 2–3 次降为零。fresh reuse 仍有两次 Sync、Actor prepare 403 ms。字典 touch/cache 耗时仍波动，不能把总差值全部归因于 reuse。

默认 x64/Win32 完整 TIP 四组各首轮/第二轮通过，但使用 local user/build，且默认允许合成目录学习；不是共享 prebuilt 或安装态验收。reuse 条件 Broker ready 471/525 ms 仍超过 400 ms。native DB probe 的同步写/重开/锁拒绝/失败返回通过，不证明崩溃/断电或大用户词典恢复。Rust fmt/Clippy/workspace tests、Actor 三项守卫与 native 严格构建通过。

隔离 DLL `build/win10-db-open-v1/dist/lib/rime.dll` SHA `CC732CB66A745372D97E73AC4688E4D15240BFCB00385D37B17AEF281D37D65E`；当前 compile 也生成它，其他已归档 dist 版本仍保留。接受的生产 runtime `94D646…`、stage、VM 0.0.11.0 未变。reuse_logs 仅为 upstream experimental 诊断选项，未接入产品。

原始归档 `build/win10-evidence-clean-v1/DbOpen-v1` 的 92 个文件/205 个外部输入身份由清单 SHA `37BFAB3DE9C5355DFDB90BA91F305E230FCF1BF3CF34F1E634B00A0063E7DCA3` 锁定；实际 Broker `277CEE…` 和 Actor 已复制保留。后续优先测 Broker 参数/计划解析、pipe pool、Engine load、prepare/readiness，补代表性 profile 与错误/崩溃恢复；保持 Win10 优先及原 deadline，G2/G3 不升级。

## 2026-10-04 Broker 启动分项后续交接

详见 [启动分项证据](WIN10-BROKER-STARTUP-EVIDENCE.md)。`crates/mo-broker/src/startup_latency.rs` 固定八槽记录 Parse/PipeBind/Settings/EngineLoad/BackendPrepare/EngineStart/WorkersStart/MainToReady；仅 debug + latency-trace 启用，借既有有界 logger 在 listening 后输出，默认/release 为零大小且无诊断输出字符串。engine_start 包含 load/prepare；host-minus-main 含进程创建、运行时、调度与 stderr 接收，不能当作 PE/Defender/磁盘直接证据。

五组 local user/build 完整 TIP 首轮/第二轮（十次）全通过，默认允许合成学习。existing reuse 的 backend prepare 147/141 ms、host ready 298/175 ms；fresh reuse ready 2,044 ms，Session 1.54 秒。三个 fake 无输入启动 host 为 1,705/26/29 ms、main 内约 2.3–2.8 ms，首次副本路径与完整 TIP 路径不同。保留此前 471/525 ms 等失败预算样本，不宣称稳定启动或安装态通过。

实测 trace Broker SHA `DD3DB30E7D7287A186A4419075579395246BDA1C6B8A458EA04A089680A33817`，沿用隔离 runtime `CC732CB…`。后续 Cargo 已重建 root Broker；实际三种 Broker、双架构 TIP、DLL、源码/日志/harness/checks 封存于 `build/win10-evidence-clean-v1/BrokerStartup-v1`，55 文件/157 外部身份，manifest SHA `C1A50611CC5CC3F81EFE6726D228E23D1F7F076E822F5F89D40D65EC76CAE8F8`。fmt、四种 Clippy、两套 workspace tests、默认诊断关闭、两种 release 各五拒绝门通过。

VM 仍为 0.0.11.0；生产 runtime/stage 未变，owned Broker 均已退出。下一步验证 reuse_logs 合成崩溃/错误恢复与代表性词典，细分 fresh Session/文件打开/进程外启动，再考虑产品方案与安装态共享 prebuilt/注册宿主验收。G2/G3 与 50/400 ms 门槛不变。

## 2026-10-04 DB 崩溃/错误恢复后续交接

详见 [恢复边界证据](WIN10-DB-RECOVERY-EVIDENCE.md)。新增 DbRecoveryProbe 与两份 tools/test-db-recovery* 脚本，14 组直接 LevelDB API 矩阵在正常/复制诊断头 CI 布局分别运行；每套 12 正向、2 负向，合计 8 个完成同步写握手的 child 被父进程强制终止。同步更新/删除、append log/manifest 回退、Sync 错误传播和 32,768 条 recovery compaction 检查通过。

关键负向：实际默认 paranoid=false 的 read-open I/O 错误被忽略，DB::Open OK 却缺失全部 128 合成记录，reuse 0/1 都出现；strict=true 的直接 API 对照拒绝，移除注入后记录完整。Sync 错误后记录重开仍存在，失败结果有歧义。librime 普通更新是默认非同步写，上游还有自动 userdb recovery task/RepairDB/rename/remove/recreate 路径；本轮没有实际 Rime/Actor 错误注入，不能只设 paranoid 就宣称产品已修复。

接受 runtime `94D646…`、stage、VM 0.0.11.0 与预算未变。首/CI probe SHA `EF3D073A…` / `BF47CA22…`，源码/合成 DB/两套日志及首次 Rust 失败在 `build/win10-evidence-clean-v1/DbRecovery-v1`，386 文件/20 外部输入，manifest `F3F76F198F5BBEDCCC3DE1D3D76676C69BA5E98915D8E7CEE9C938380F9B61FF`。native strict builds、五 harness/五 native/两 build guards、fmt/Clippy 通过；workspace 首轮 worker-panic 子进程三秒未退出，单项 1.83 秒及完整复跑通过，未放宽断言、未定根因。CI 接线/AST 与本机布局验证通过，远端 Actions 未执行。

下一步优先定义并实测 Mo 用户词典的无破坏错误策略、实际 Actor/Broker 准备拒绝和恢复；日志复用暂不接入产品。随后继续非同步学习/崩溃边界、fresh 启动及 Win10 安装态共享数据/注册宿主验收。owned recovery child 均退出，G2/G3 状态不变。

## 2026-10-04 实际用户词典错误续查

[用户词典错误证据](WIN10-USERDB-ERRORS-EVIDENCE.md) 与 Proposed ADR 0055：独立 patch 开启严格 LevelDB 恢复检查、移除普通 Load 失败的自动 recovery task，并在 input-free preparation 核验必需主用户词典的 Load/loaded 状态。真实 WAL sharing error 和 checksum 损坏均由实际 Actor/Broker 在就绪前拒绝；四次失败后数据文件哈希/集合一致（排除 LOG/LOG.old/LOCK）。解除测试故障后恢复，两个 fixture 的 32 条同步合成记录全部读回；九项拒绝门/两个 AST、fmt、Clippy、mo-rime all-targets 通过。

完整 workspace 首轮四个三秒 watchdog 失败，默认复跑 panic 失败，串行复跑 startup 失败；单项 startup 随后 2.13 秒通过，未改时限/未定根因。正确映像位置的 TIP x64 首/第二轮及 Win32 首轮仍失败，Win32 第二轮通过，ready 2,951/1,757 ms；G2/G3、延迟预算不变。首版过度要求可缺省词典以及首轮 TIP Broker 路径配置错误的失败均归档。

新 DLL `E8D8C1FD…` 和 helper `74A82A4F…` 只在隔离 build 使用；接受 runtime `94D646…` 和 VM 0.0.11.0 未变。补丁尚未接入 runtime-build/CI/stage/安装包，v2 导出尚不能识别该新策略；完整 workspace 问题、新 ABI/provenance 构建门与 privacy/learning/schema/安装态复验是接入前置检查。源码、实际 PE、合成 DB 和负向日志封存在 `build/win10-evidence-clean-v1/UserDbErrors-v1`，身份见 Git 汇总。本轮全部 owned child 已结束，日常开发继续 develop。

## 2026-10-04 watchdog 退出路径交接

[退出证据](WIN10-WATCHDOG-EXIT-EVIDENCE.md) 与 ADR 0056：旧 test-only 标记确认八阶段均进入 fail_stop，全部观察到指向 owned child 的 WerFault，五项超过原三秒（exit 2,183–7,430 ms）。这些时戳含父进程/WMI观察成本，不精确分配 WER/CPU 开销；旧失败保留。

Windows Broker lifecycle 改为平台 safe primitive 的当前进程 TerminateProcess，固定应用退出码 E04D4F01，API 意外返回 fallback abort；不分配、不记录日志、不运行析构/正常 DLL detach。生产健康预算和 TIP deadline 未变，unit fixture 仍要求 intended marker+固定退出状态+三秒内结束。八阶段各三次共 24 child 实测 36–215 ms，全通过且无 parent kill。

默认/trace 完整 workspace、两套 Clippy/fmt、Release 优化 EngineService 12 项、双架构 native strict/ABI/nonmutating policy 与默认 fake TIP IPC/pool/UI/edit/一轮重连故障（共四次外部 Broker 退出）均通过。没有将外部 harness kill 混作内部 watchdog；Rust production panic=abort 和 pipe 极端错误 abort、永久 kernel I/O 卡住仍是边界。接受 runtime 94D646…、VM 0.0.11.0 与 G2/G3 未变，词库延迟仍失败。ADR 0055 继续 Proposed；完整 workspace 退出缺口已补，下一步新用户词典 ABI/provenance/CI 门及安装态 Win10 验收。源码/旧新实际 PE/日志归档 WatchdogExit-v1，身份见 Git 汇总；继续在 develop 开发。

## 2026-10-04 用户词典 ABI v3 工程化交接

[ABI v3 接入证据](WIN10-USERDB-ABI3-EVIDENCE.md) 与 ADR 0055：开发态契约已接受。
仅导出/解析 mo_rime_prepare_resources_v3，旧 v1/v2 在私有 anchor 前被拒绝；
builder 快照 userdb-preserve.patch，在 learning 后明确应用并检查实际源码。
provenance 的 ABI=3、strict-open-no-auto-recovery-v1、补丁/source inventory 绑定
同步进入 runtime/stage/部署工具和 CI。旧 runtime 在 staging 创建输出前被拒绝。

完整固定源码重建得到 BE5E3E37…，全新 release stage 在
build/mo-windows-stage-userdb-v3/stage；stage manifest 9650E87A…。
实际 Actor/Broker 矩阵 12 child 通过，两种 WAL 故障均就绪前拒绝，原数据文件
哈希/集合保全（排除 LOG/LOG.old/LOCK），解除故障后各读回 32 条同步合成记录。
preparation 3、relocation 27、隐私→正常→隐私学习 0/1/1、五方案/简繁体/Emoji，
默认/trace workspace、两套 Clippy/fmt、双架构 ABI/fake 全 IPC 及四次外部退出通过。
新 stage policy 97、authoring 21、lifecycle policy 18，七项真实 golden/准备/
Actor paging 通过。native 负向 policy 的 LASTEXITCODE 仅在全部断言后清除，
同进程 CI 状态 0；九 CI block AST 通过，PyYAML 缺失/远端 CI 未验证的边界保留。

实际 stage TIP x64 首 N 未消费（外部 63 ms，exit 1）；独立 Win32 同样失败
（外部 47 ms，exit 1），都未进入 stop cycle。47 ms 不直接代表 Actor 执行
时长或根因；50 ms 不变，G2/G3 仍开放。组合检查父进程中止、独立补跑及全部
失败日志都保留，未重试成通过。当前所有 owned Broker 已结束。

证据封存 UserDbAbi3-v1，610 文件/输入记录，manifest 28357664…；完整身份见
Git 汇总。VM 沿用 0.0.11.0/94D646…，本轮无安装或系统输入路由测试。下一步集中
Win10，定位真实 TIP 首键失败，再推进安装态新 ABI、普通宿主、loaded-TIP 升级
和登录/重启验收。继续在 develop 开发，main 留作正式版基线。

## 2026-10-04 ABI v3 首键与查询内部续查

详见 [ABI v3 首键证据](WIN10-ABI3-LATENCY-EVIDENCE.md)。新增可重复的双架构
合成测量脚本、八项拒绝门与两份 timing-only 补丁；补丁 replay 后仍通过严格
userdb startup 守卫，CI 已接线。24 次完整 probe 中 13 次失败、11 次通过。
基线 fresh 首次 Actor dispatch 99/155 ms、排队 17–33 µs；复用目录首 N
3–4 ms 但后续 H 仍失败。客户端约 50 ms 在响应头超时，deadline 未改。

诊断第二版首键 220/125 ms；x64 词表候选 Sort 约 100 ms、缺页增量 24，
Win32 用户词典/英文查询/Prism 也有长区间。未证明硬缺页、磁盘或调度原因；
旧 Evaluate 82 ms 的 self 盲区失败保留。仅 local user/build debug 计划，
没有安装态 shared-prebuilt 证据；376 ms 的单个 ready 成功不代表稳定激活。

诊断 DLL 6DE2485D…/C4BF7113… 分别保留 dist/dist-v2，当前 source/compile
是第二版；BE5E3E37… stage 与 VM 0.0.11.0 未改。fmt/Clippy/workspace、
双架构 trace ABI、拒绝门/replay 均通过；原始 PE/源码/profile/log 在
Abi3Latency-v1，由 Git JSON manifest 绑定。所有 owned 进程已退出。
继续在 develop；下一步追踪 Sort/Peek 映射访问等待，验证后续键与 fresh 启动，
随后安装态新 ABI、宿主及 loaded-TIP/登录矩阵。G2/G3 仍未完成。
## 2026-10-04 单字段页面读取与系统预取续查

[读取/预取证据](WIN10-READ-PAGES-EVIDENCE.md)：新增最多 512 条标量读取采样、
解码分项、严格合成 probe，以及可选 PrefetchVirtualMemory 实验。四个采样进程
首键各 419 次读取/24 次非驻留；具体 weight load 10.277 ms、string ID 3.016 ms
伴随工作集转换。另有 24 个非驻留只合计 52 µs 的样本；元数据查询本身增加
16–18 ms，未证明硬缺页/磁盘/调度责任，不能混同未采样性能。

同 DLL 关闭/开启对照共 32 次完整 TIP，9 失败/23 通过。预取 x64 fresh 首轮
通过但 ready 1020 ms；Win32 fresh/existing 首键仍 129/185 ms，全部 API 返回 1
也不能保证时限。一份约 60 MB mapping 单次预取 486 ms，因此不进入产品。
采样 DLL CF021ED0… 和预取 DLL F2A4C015… 保留 dist-v3/dist-v4，current source/
compile 是 v4；所有测试仅 local user/build debug 计划，前端 Win32 的引擎仍 x64。

完整 workspace 发现 PID+时间戳测试目录碰撞；80adaaf 改原子序列及创建所有权，
固定同时间戳八线程回归通过。首轮失败与修复后通过均保留；fmt/Clippy/workspace、
strict native 四项、三/四补丁 replay、八拒绝门、十 CI block AST 通过。
原始产物/源码/profile/log 在 ReadPages-v1，由 Git JSON manifest 绑定。
接受 stage 与 VM 0.0.11.0 未改，50/400 ms 及 G2/G3 不变，owned 进程已退出。
继续 develop；下一步隔离 Win10 内核缺页/I/O/调度证据及 fresh 首次拉起验证，
随后新 ABI 安装态、宿主及 loaded-TIP/登录矩阵。

2026-10-05 收尾：按实验起始日保留证据文件名；567 个归档文件、245 个外部输入
校验通过，manifest 18BCD35D…，接受 stage DLL 哈希未变。32 次探针统计为
9 失败/23 通过，完整 workspace 在修复测试目录碰撞后通过。

## 2026-10-07 固定缓冲、线程调度与候选惰性生成续查

[缓冲与调度证据](WIN10-DEFERRED-EVIDENCE.md)：新增固定 2048 scope/每 capture、
512 scalar/每进程；ProcessKey 与 Menu::Prepare 分开缓冲并配对 current-thread
dispatch profiling。默认关闭，不用全系统 ETW 或硬件计数器，不进入产品包。
六项严格 probe（含已有采集所有权）、原四项页面/预取、十一拒绝门、五补丁
replay/实际源码、fmt/Clippy/workspace、十非空 CI block AST 通过；远端 CI 未验证。

五组共 40 次完整 TIP：21 失败/19 通过。未加内部诊断的 BE5E…基线 fresh 首
Actor x64 290 ms、Win32 88 ms，依旧失败。低扰动字段窗口最大 69.834 ms，
x64 fresh 321 ms native/29 次切换、位 0/31；调度位图是聚合信息，
未证明硬缺页、磁盘或具体等待原因。日志排出分项报告，但仍计入 50 ms 请求总耗时。

发现 ProcessKey 后还会惰性生成候选：v6 native 31.5 ms 后 Menu::Prepare
120.8 ms；v7 单独缓冲后仍有 94.3 ms menu（Sort 51.7 ms，2 次切换）。
快首 native 1.36 ms 的 x64 existing 后续 native 256 ms，完整首轮仍失败。
三个隔离 DLL 6830E63E…/D0053304…/4C3E5B73…，current source/compile 为 v7，
前版 source-v5/v6 与产物保留；仅 debug local user/build/x64 引擎，两种前端。

证据 Deferred-v1 按 SHA 去重 PE，原始日志/源码/合成 DB 与 external profile/build
字典哈希绑定。旧归档未改，stage BE5E…及 VM 0.0.11.0 未改，owned 进程均退出。
继续 develop；50/400 ms 与 G2/G3 仍开放，下一步关联缺页/I/O/逐项调度并
覆盖全请求/后续键，再推进安装态新 ABI、宿主与 loaded-TIP/登录矩阵。

## 2026-10-07 开发方向调整与设置冲突保护

用户要求多轮时延验证未能确定具体原因时继续其他方向。近期超时在 Win10
开发主机也复现，不是只在 VM；缓存、主机负载和调度仍未受控，不能确定 VM 性能
或其他单一原因。暂停扩展时延专项，保留 50/400 ms 与 G2/G3 未通过的状态；
后续优先设置可靠性、安装态新 ABI/设置功能及 loaded-TIP/登录重启验收。

[ADR 0057](../adr/0057-settings-checked-save.md) 修复旧设置窗口覆盖新选择/未来文档：
ordinary save 在自有跨进程保护内核对强类型值与 absent/stored 来源，冲突拒绝，
提示重新读取；显式恢复默认仍可替换，但也取得相同保护。Win10 关闭删除句柄
在强制结束自有 child 后无残留。八线程只一个成功，外来 marker 保全，
旧窗口不能关闭另一窗口的隐私偏好；设置模型 25/控制器 9 项通过，
ignored child 入口由父测试明确调用。fmt/Clippy/workspace 与 EXE 构建通过。
未操作真实用户配置、安装 GUI、stage 或 VM；v1 格式及输入时限不变。

## 2026-10-07 ABI v3 与设置修复整合包

[0.0.12.0 开发包证据](WIN10-PACKAGED-0120-EVIDENCE.md)：基于 6aebe7c 产品
源快照重新构建 release Broker/settings、双架构 TIP/registrar；使用已验收 BE5E…
ABI v3 运行时，诊断导出缺席。132-file 载荷、linked MSI/Burn、无警告 MSI ICE、
kit 哈希通过，unsigned DevelopmentTest 仍不可分发。

升级 pair 新增显式 Changed 模式，旧、新 stage 各自绑定；默认 Same 回执不变。
14 项策略、21 项作者、39 项 VM 策略通过；实际 0.0.11.0 → 0.0.12.0 pair
通过，默认模式和旧同载荷矩阵均拒绝它。脚本 AST、十个 CI block 通过，
远端 CI 未核验。Package0120-v1 封存 123 文件/139 外部输入，
manifest 62E9E136…；产品源 commit 与本轮验证工具改动分别记载。

VM 在线，但无密码 guest command 被账户限制拒绝，Computer Use 两次无法激活
窗口；本轮未执行安装/升级/GUI。最近验证的 VM 仍 0.0.11.0、旧 ABI v2。
等待恢复可操作桌面，继续独立变更载荷 guest 升级和多窗口设置/宿主验证；
现有 matrix 不能复用。时延专项保持暂停优先级，50/400 ms、G2/G3 未整体通过。

## 2026-10-08 独立变更载荷升级测试包

[双清单 kit 与 guest driver](WIN10-CHANGED-UPGRADE-EVIDENCE.md) 补齐
0.0.11.0 → 0.0.12.0 独立升级入口。旧树按历史 frozen inventory、新树按当前
stage 复核；12 文件 kit 分别绑定旧/新 Bundle、stage、linked 回执和 registrar。
普通用户、VM sentinel 与两执行开关控制 guest；安装前/后各自校验 MSI、载荷、
ACL/COM/profile/finalizer，设置存在性/字节和默认 override 不变，非 0 exit 拒绝。
只运行新 Bundle，未覆盖 clean install、注入 rollback、loaded TIP、reboot/输入。

新合成策略 34 项、原 pair 14 项、原 VM 策略 39 项通过，实际最终 v2 kit 哈希
校验通过；Win10 原生 PowerShell 5.1.19041.6456 实际 kit/helper/driver AST
及两项 host 拒绝门通过。四脚本和十 CI block AST 通过，测试接入唯一 CI step；
远端 CI 未核验。ChangedUpgrade0120-v1 封存 37 文件/279 外部输入，
manifest 87A5F2CD…，旧 Package0120-v1 未改。

VM 在线；窗口最小化后请求恢复，捕获 FrameArrived 超时，仍无可观察桌面。
本轮未执行升级/安装态设置/输入；最近 VM 验证版本仍 0.0.11.0/ABI v2。
后续使用 build/mo-vm-changed-upgrade-kit-0110-0120-v2；恢复访问后先单次升级
和设置保全，再多窗口设置与真实宿主。时延专项维持低优先级、G2/G3 保持开放。

## 2026-10-08 升级事务最终状态完成门

[事务修复与 v3 kit](WIN10-UPGRADE-TRANSACTION-EVIDENCE.md) 修正最后 registrar
读取失败仍可能 completed=true；format 2 回执要求最终 Installed 状态通过，
保存 final_state_verified/final_state_failure，主错误与最终错误各自保留，
异常可读状态不丢失。内部操作不作为 CLI 参数，VM/普通权限/kit 拒绝门保留。

PS7/WinPS5.1 各 21 事务测试（含两项原始 driver 操作块内存适配）通过，
原 kit 34 项、实际 v3 kit 哈希/最终源码和两项 host 拒绝门通过。
三脚本、十 CI block AST 和双 shell CI 入口通过；远端 CI 未核验。
ChangedUpgradeTxn0120-v1 封存 35 文件/280 外部输入，manifest 9C15C62E…。
产品二进制与历史归档未改；后续使用 build/mo-vm-changed-upgrade-kit-0110-0120-v3。

VM 窗口在，但捕获仍 FrameArrived 超时；本轮没有真实安装/输入/重启。
自动通道不可用时，VM 手动运行及共享目录证据回传步骤已写入文档。
最近 VM 验证仍 0.0.11.0/ABI v2，单次升级成功路径尚未取得真实证据。
继续优先升级/设置保全及普通宿主；时延专项低优先级，G2/G3 状态保持开放。

## 2026-10-08 候选窗口高对比度与外观重绘

[ADR 0058](../adr/0058-candidate-system-high-contrast.md) 实现高对比度优先于
保存的 Light/Dark/System，预编辑/页脚使用系统正文颜色、按压使用系统高亮；
退出后恢复保存主题，查询失败回退系统色。首次窗口与三个外观消息查询并重绘，
不变更候选 revision、焦点、文本、窗口锚点或会话，不改变 settings/IPC。

早期 Unicode scheme 指针 LocalFree 导致 x64 0xC0000374；改为只读 flags，
不释放系统方案指针后，双架构 /W4 /WX、配色策略、真实 EDIT/受控 TSF
外观重绘、fake IPC/连接池/候选与每架构两次 Broker 崩溃恢复全部通过。
Rust fmt/Clippy/workspace 通过；原失败日志保留，不声称保存了失败 PE。
[证据清单](evidence/WIN10-CANDIDATE-CONTRAST-20261008.json) 封存
CandidateContrast-v1，56 文件，manifest C47121D90D053447CC711D9C3324EC1451FE2C59BBB6FD4DEC239F5E998F5D25。

本轮未切换系统高对比度、运行真实 Rime 烟测、注册 TIP、操作真实设置或 VM；
未重建 stage/安装包，0.0.12.0 开发包不含本轮修复，最近验证的 VM 仍
0.0.11.0/ABI v2。真实视觉/混合 DPI/宿主验收保留；50/400 ms 和 G2/G3 未整体通过。
下一步可继续候选辅助功能和设置可靠性；guest 证据返回后核验新 ABI 升级。

## 2026-10-08 0.0.13.0 高对比度修复整合包

[Win10 开发包证据](WIN10-PACKAGED-0130-EVIDENCE.md)：产品源 0babd75，
fresh release / 双架构严格编译，89 源文件绑定、132 payload / 138 stage 项通过。
高对比度与安全 scheme 查询进入实际 TIP；ABI v3 BE5E…运行时不变。
实际 staged DLL + 隔离 fake Broker 的重绘/候选/提交及每架构两次退出恢复通过。
MSI/Burn、无警告 ICE、Changed 0.0.11.0 → 0.0.13.0 pair、kit 全哈希通过；
WinPS 5.1 核验最终脚本并从实际 kit driver 通过两项 host 拒绝。
Package0130-v1 封存 160 文件/140 外部输入，
manifest 1451CC7F0B477CF21355C3168182F42F0293C136024F4C919528B101370F0298。unsigned DevelopmentTest 不可分发。

最新 kit：build/mo-vm-changed-upgrade-kit-0110-0130-v1，使用最终 format 2 审计门；
共享封存 Package0130-v1/kit，guest 回传 ChangedUpgradeGuest0130-v1。
最近 VM 已验证基线仍 0.0.11.0/ABI v2，本轮没有新 VM 操作；若已执行旧 0.0.12.0
kit，先核验其回执再匹配升级基线。真实 Rime 输入/系统高对比度/安装态 GUI/
宿主/重启未验收，50/400 ms 与 G2/G3 仍开放，时延专项维持暂缓。

## 2026-10-11 Figma 输入体验设计提案

[设计交接](../design/MO-UI-DESIGN-20261011.md)：已创建 Figma 文件 pFUWDZYKrLw9dxdNQ4XanU，
四个现代中文设置页面及导航原型、纸白/夜墨/薄荷/暮霞/墨竹候选皮肤、
系统高对比度和竖排示例。28 主组件、62 语义/几何变量、共享字体/阴影样式；
文字、组件和矢量可编辑，两个展示画板 IMAGE fill 为 0。
保留显式保存、冲突/写忙提示、本地学习/隐私与不抢焦点合同。
新皮肤/横排/字号/自定义与设置界面独立主题均为设计，不是现有 0.0.13.0 功能。

初轮截图已检查；随后修正 auto-layout、状态/页码对齐、皮肤说明和颜色列。
get_screenshot 被 Starter 额度拒绝、备用浏览器初始化超时；
随后使用 Plugin API 原生截图完成两个修正后画板的末轮视觉复核，通过。
首轮 PNG 仅为历史参考，最终图片在本会话原生附件中；记录/ledger 位于 docs/design。
产品代码、stage/安装包/VM 未改；50/400 ms 与 G2/G3 仍开放。


## 2026-10-11 Figma 原生候选呈现

Figma 已确认 Professional / Full；从纸白/夜墨竖排高保真上下文接入
C++ CandidateWindow，见 [实施记录](../design/MO-NATIVE-CANDIDATE-20261011.md)
及 ADR 0059。圆角/细边框、分层字号、独立标签和注释已完成；
System/高对比度、焦点、revision 和空闲设置合同保留。
双架构 /W4 /WX、合成原生截图、200 次 GDI 刷新、
fake IPC/TSF/两次 Broker 崩溃恢复及 Rust fmt/Clippy/163 测试通过。
设置三页 UI/自定义皮肤尚待实施；没有更新 stage/MSI/VM，
最后已验证 VM 0.0.11.0、准备好的 0.0.13.0 kit 及 50/400 ms 边界不变，G2/G3 不升级。


## 2026-10-11 紧凑气泡、表情与图标设计

用户提供讯飞气泡参考，确认独立表情面板＋联想候选，并要求现代 Mo 图标。
[新版 Figma 记录](../design/MO-COMPACT-EMOJI-ICON-20261011.md) 已完成：
两行 560 × 86 气泡、右侧小翻页箭头、按需更多候选、Emoji/颜文字、
联想开关原型，以及原创 SVG/透明 PNG/七档 ICO。
最终候选与图标截图、可编辑性和图标尺寸/透明度检查通过。
这是设计与资产；横排/更多/独立表情和 PE 图标接入尚未实施，
原生源码维持 7ea9ec6 的竖排版本，stage/MSI/VM 和 G2/G3 不变。
