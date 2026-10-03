# 会话恢复与 Win10 验证交接

更新：2026-10-03。用户优先验证 Win10；其当前主机也是 Win10。虚拟机已准备，测试账户无密码。

## 恢复基线

工作目录 `E:\ChuGe\CodeProject\101_ProjectCollection\Mo`，分支 main，恢复时 HEAD `9cc5d3b`。现有大量未提交修改是此前开发成果，不能重置。ADR 0046–0051 和代码共同说明已实现空闲会话设置替换、MSI x86 COM 路径、静态 MSVC runtime、候选注释/标签、Emoji 偏好、会话学习/隐私策略。已生成 132-file stage 与开发态故障注入安装包；生成产物位于忽略的 build 目录，不代表发行版本。

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
