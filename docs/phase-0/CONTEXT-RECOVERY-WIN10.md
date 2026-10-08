# 会话恢复与 Win10 验证交接

更新：2026-10-04。用户优先验证 Win10；其当前主机也是 Win10。虚拟机已准备，测试账户无密码。

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
