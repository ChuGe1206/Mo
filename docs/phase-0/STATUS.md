# Phase 0 状态

- 快照日期：2026-10-04
- 结论：Phase 0 已启动，G1 本机证据闭环；G2/G4 部分通过；G3 未通过。受控 TSF Edit Session 已贯通；未签名开发包已有 Win10 VM 有限生命周期验证，项目仍不可发布或日常使用。
- 本机环境：Windows 10 22H2 build 19045（尽力兼容环境）、Rust 1.97.1 x86_64-pc-windows-msvc、Visual Studio 2022 17.14.37、MSVC 14.44、Windows SDK 10.0.26100.0。

## 2026-10-04 Win10 用户词典恢复边界

- [恢复证据](WIN10-DB-RECOVERY-EVIDENCE.md)：新增严格原生探针与 marker/SHA/全新目录守卫的 14 组矩阵；两套构建布局复验一致。四组同步写后强制终止、追加失败回退、同步错误传播及 32,768 条恢复 compaction 检查通过。
- 每套保留两组负向结果：默认 paranoid 关闭时，日志读取失败后 DB::Open 成功却丢失 128 条合成记录；reuse 开/关均出现。严格 LevelDB 对照拒绝后可恢复，但实际 librime 有自动 repair/recreate 路径尚未注入。Sync 失败返回的记录重开后可见，不能据错误推断未写。日志复用继续禁止进入产品，未声明非同步学习或断电耐久性。
- 两份 native `/W4 /WX /MT` 构建、五项 harness/五项 native/两项构建拒绝、fmt/Clippy 通过；workspace 首轮 worker-panic 三秒退出失败保留，单项与完整复跑通过，根因未定。新增 CI gate 尚无远端结果。归档绑定 386 文件/20 外部身份；生产 runtime/stage/VM/deadline 不变，G2/G3 不升级。

## 2026-10-04 Win10 Broker 启动分项

- [启动分项证据](WIN10-BROKER-STARTUP-EVIDENCE.md)：新增仅 debug latency-trace 启用、就绪后有界输出的八阶段计时；默认/release 为零大小无计时器。五组双架构完整 TIP 各首轮/第二轮通过，三个无输入 fake 对照保存。
- 已有目录 reuse 条件 backend prepare 141–147 ms、pipe 绑定约 1.4 ms，本轮 host ready 298/175 ms；fresh reuse 仍为 2,044 ms。fake 首次 host 1,705 ms、main 内 2.8 ms，说明外部差值需要继续测量，不能归因于 IPC/磁盘/扫描器。先前超预算样本保留，不能升级为稳定冷启动通过。
- fmt、四种 debug/release Clippy、两套 workspace tests 通过；默认 debug 与 feature release 实际 PE 无诊断标记，两种 release 各五项启动拒绝通过。归档绑定 55 文件/157 外部输入；生产 runtime、stage、VM 与原 deadline 不变，G2/G3 状态不升级。

## 2026-10-04 Win10 用户词典启动诊断

- [DB 打开与映射证据](WIN10-DB-OPEN-EVIDENCE.md)：八个 Actor 合成进程、默认双架构四组完整 TIP 首轮/第二轮及 native DB 转发/重开/锁拒绝检查通过。新增共享 prebuilt Actor fixture、原生 DB I/O scope 和 upstream experimental reuse_logs 对照；未接入产品。
- 既有 shared profile 的首个 DB::Open 对照为 reuse 关闭 106–221 ms、打开 2–3 ms，Sync 次数 2–3 → 0；fresh reuse 的 Actor prepare 仍为 403 ms。mapped touch 存在缓存/顺序混杂，完整 TIP 是 local user/build 且允许合成学习，不能当作安装态共享数据结论。
- reuse 条件完整 Broker ready 为 471/525 ms，400 ms activation 仍未满足；首键成功发生在预先 ready 后。生产 runtime/stage/VM 0.0.11.0 未变，50 ms deadline 未改。原始清单绑定 92 文件/205 外部输入，Rust fmt/Clippy/workspace tests 和三项 Actor 守卫通过；G2/G3 保持原状态。

## 2026-10-03 Win10 VM 续测

- 会话恢复记录见 [CONTEXT-RECOVERY-WIN10.md](CONTEXT-RECOVERY-WIN10.md)，优先 Win10 验证。
- 已修复 GUI Bundle 等待退出和两类 Win10 安装状态缓存误判，见 ADR 0052。机器 MSI 安装及普通用户六项 finalizer 事务已有真实 VM 证据。0.0.9.10 发现卸载空根目录残留并修复；0.0.9.11 的 Win10 clean install/repair/uninstall 完整通过，三阶段 exit 0、默认输入法未变、文件及 ACL 校验通过，卸载根目录/profile/COM 清除。证据位于 `build/win10-evidence-clean-v1/V0911`；0.0.9.11 → 0.0.10.0 六阶段回滚/升级矩阵也已完整通过，证据位于 `build/win10-evidence-clean-v1/Matrix0100`；真实输入、loaded-TIP 和登录/重启未完成，G3 仍未整体通过。

- 本轮继续 Win10 真实 x64 记事本测试，定位并修复 TSF test/key 回调 repeat count 差异导致的重复派发（ADR 0054）。双架构 fake 回归和旧/新 DLL 反向对照通过；VM 临时修复 DLL 的精确提交回读、繁体/深色设置功能通过，详见 [桌面有限实测](WIN10-DESKTOP-EVIDENCE.md)。两次 default/一次 trace 完整真实词库仍在 x64 TIP 首键响应超时失败；后续阶段未执行。前轮原 VM 安装树恢复为 0.0.10.0；后续已升级包含修复的 0.0.11.0，见下项；G2/G3 状态不升级。

- 0.0.11.0 新 stage/linked/MSI ICE/VM kit 通过，Win10 从 0.0.10.0 变更载荷升级 exit 0，旧 MSI 移除、新载荷/权限/COM/默认及设置保护通过；一次重启后状态复核，以及安装态 x64/x86 记事本与 x86 写字板两次提交精确回读通过。证据见 [0.0.11.0 续测](WIN10-PACKAGED-0110-EVIDENCE.md)。隔离 trace 保留失败，拿到 Actor 执行约 1.31/1.67 秒、后续键 252 ms；直接探针定位主要长耗时在 native process_key，具体原因未明。当前 VM 保留新包，测试窗口关闭；完整宿主/延迟/loaded-TIP 矩阵仍待验证。

## G0：工程基线——本机通过，远端 CI 待首次运行

- Cargo workspace 包含 `mo-domain`、`mo-engine`、`mo-ipc`、`mo-windows-pipe`、`mo-windows-platform`、`mo-broker`、`mo-rime-sys`、`mo-rime`。
- `cargo +1.97.1 fmt --all -- --check`：通过。
- `cargo +1.97.1 clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo +1.97.1 test --workspace`：通过；设置会话替换回归增加后端拒绝创建时旧预编辑继续、新会话从空预编辑起步的断言。显式 `mo-broker/latency-trace` 仍保留独立分支；构建高负载时曾有启动 watchdog fixture marker 未到达的历史负向结果，保留于 ADR 0024。
- 默认 release 的 workspace/all-targets Clippy 与 Broker 启动负向 smoke：通过；CI 已新增独立约束关闭 debug assertions 的安装模式分支。
- `cargo +stable doc --workspace --no-deps`：通过。
- 默认/诊断 feature 的 debug/release 四种 workspace/all-targets Clippy 均通过；默认及 release feature 的计时 envelope 以编译期断言保证零大小，默认不启动 logger。TIP 的 CMake 路径本机未运行，其证据来自 MSBuild；core + Lua runtime 已用锁定 CMake 3.31.10/VS2022 完成全新目录构建。
- Rust toolchain、librime、rime-ice 与官方验证资产均已锁定；GitHub Actions 已覆盖 Rust、TSF x64/x86、librime ABI 和真实 rime-ice smoke。
- 尚未在 GitHub runner 上产生首个 CI 结果；本地 `main` 保留正式版打板基线，日常开发和 Win10 验证切至 `develop`；`origin` 已配置为项目 GitHub 仓库。

## G1：librime FFI——本机通过

- 上游：librime 1.17.0，commit `33e78140250125871856cdc5b42ddc6a5fcd3cd4`。
- 官方 x64 验证资产 SHA-256：`7478c7caa4ff6b37de86daba1f7ce4a994a4f5ba24872a820fb2b3a9b01fed15`。
- `tools/abi-probe/compare.ps1`：官方 C header 与 Rust 声明的 55 项 size/alignment/offset/data_size 断言一致，包含可选 schema/option、当前页选择/翻页 API 尾部；本机 FFI 布局验证目标为 x64 Broker。i686 Rust target 未安装，本次 x86 Rust ABI 对照未运行。
- 安全封装确保 Engine/Session 单线程、Session 借用 Engine、所有 commit/context/status native 输出严格配对 `free_*`，并将返回值复制为 owned Rust snapshot。
- `RimeBackend` 已实现 Engine Actor 后端契约：私有 native session id 不越过适配层，key/commit/clear、修饰位映射、composition UTF-8 byte offset 校验、候选注释/标签与状态投影均有确定性伪 API 测试。当前页零基索引选择与前后翻页已通过安全可选 API 接入 Actor；五种锁定 schema 与 `traditionalization` 已按会话配置，未知值、旧表/截断字段/空槽均明确失败并回收未完成会话，见 ADR 0045。
- `candidate_smoke` 已对真实 librime/rime-ice 验证 `ni` 前后翻页恢复原页、选择当前页第二候选及准确 commit；该直接 API 检查已纳入完整真实引擎 smoke，并只使用可回收的独立用户目录。
- Broker 后端新增一个私有、无输入的 native 资源保活会话，永不承载用户 key/commit/clear、没有 wire token，用户会话仍各自创建/销毁。两个伪 API 测试验证隔离/销毁顺序与创建失败回收；保活+预编译的直接 API 对照中，首次转换约 27 ms，后续新会话首键约 0.7–1.7 ms，不是压力或机器冷启动通过。
- `Engine::load` 已用绝对 canonical DLL 路径和受限 `LoadLibraryExW` 搜索目录实现运行时加载；不读取 PATH/当前目录，解析 `rime_get_api` 及同一 DLL 的可选版本化 Mo 准备导出，并保证 finalize 后才卸载 DLL。当前只解析 v2、不回退 v1；相对路径、错误文件名和缺失文件均 fail-closed，要求准备的模式不允许缺导出回退。
- 官方 librime + 锁定 rime-ice 完成真实部署和 `nihao -> 你好` 候选及提交验证。
- 锁定 runtime 的 `settings_smoke` 已用独立用户目录验证五种方案 × 两种简繁模式全部创建、首键和销毁；全拼 `zhongguo` 实际候选在简/繁分别有“中国”/“中國”。
- 正式发行不得复用该官方预构建 DLL；原因见 G4。

## G2：TSF 与 IPC——部分通过

已通过：

- 自主 C++ COM 壳以 `/W4 /WX /sdl /GS /guard:cf` 构建 x64 与 x86 DLL，零警告。
- 两种架构均通过 DLL 加载、导出、未知 CLSID、Server Lock、禁止聚合、`ITfTextInputProcessorEx`/`ITfKeyEventSink` 创建、错误参数和卸载探针。
- `ActivateEx` 已实现前台 key sink 的 advise/unadvise 对称生命周期；受控 manager + 真实 TSF context 探针证明 client ID/foreground 参数正确，`OnTestKey*` 与 `OnKey*` 当前一致 fail-open、不吞键。
- key sink 已用 `(context, virtual key, LPARAM, key direction)` 缓存 Broker 决策；匹配的 test/key 回调只向引擎发送一次按键。handled snapshot 通过同步读写 Edit Session 应用，严格转换 UTF-8，并在预编辑 Range 上执行更新、提交或清除；正式 Composition 被宿主拒绝时保留受控 Range 降级，已发生文档修改的错误路径仍吞键以避免重复上屏。
- TIP 已实现 `ITfCompositionSink` 并把自身交给 `StartComposition`：宿主主动终止时只清理匹配的 owned composition；TIP 主动提交/清空则先稳定 COM 引用并清除成员，再调用 `EndComposition`，可安全承受同步回调。x64/Win32 受控探针连续完成两轮 `M + Space`，核对最终 `mm`，证明结束后能立即创建下一 composition。
- x64 与 Win32 的受控 `ITextStoreACP` 探针均把 fake Broker 的 `M + Space` 经 TIP 写入真实 Windows EDIT 控件为单个 `m`，并从 TSF Context 再次读回核对。TIP 激活、Broker 缺席、协议或 Edit Session 失败均保持 fail-open；断线后使用 250 ms 节流进行有界重连。
- x64 与 Win32 原生客户端已对同一个 x64 Rust Broker 完成真实 `Hello -> OpenSession -> KeyEvent(M) -> Snapshot("m") -> CloseSession` 往返；并分别通过 `KeyEvent(nihao + Space)` 穿过 Broker、Actor、RimeBackend、运行时加载的 librime 与锁定 rime-ice，核对 `你好` 候选和提交。客户端采用 overlapped I/O、端到端硬 deadline、严格帧/CRC/UTF-8/请求号校验，失败或结果不明确时断开并保持 TIP fail-open。
- Broker 已补齐 PageUp/PageDown/Home/End 的 Windows VK -> X11 keysym 映射。x64/Win32 真实 IPC 探针均验证 `ni -> PageDown -> PageUp -> 2`：新页与原页不同、前页恢复、翻页不提交，数字选词准确提交原页第二候选。
- IPC 1.0 已新增协商 feature 的 13 字节 CandidateAction（kind 12），保持原 Snapshot 布局不变。Broker 按会话核对当前成功编码的候选 revision/count，未协商、空页、跨会话、越界、陈旧/重放、松键后旧版本与后端失败撤销授权均有测试；不符合条件的动作不会进入引擎。x64/Win32 C++ 真实探针均通过显式候选翻页/第二候选提交及陈旧动作拒绝后客户端 reset。
- IPC 1.0 另以协商式 kind 15 DetailedSnapshot 投影 librime 当前页候选注释和选择标签，保留旧 Snapshot 字节布局和未协商回退；超长单项元数据只省略该项。TIP 显示引擎标签，缺失时回退本页序号，`show_comments` 控制注释显隐。Rust 协议/Broker 与 x64/Win32 原生探针及真实 rime-ice 冒烟通过；真实注册宿主视觉验收仍待 VM，见 ADR 0049。
- 首版自主 Win32/GDI 候选表现层已接入 TIP：纵向 ordinal 列表、鼠标翻页/选词、不激活窗口、按宿主 DPI 缩放、monitor work area 避让、长文本省略和本页滚轮。collapsed caret 的零宽度矩形可定位，全零/裁剪/无 layout 则隐藏。布局 sink 对称 advise/unadvise，以带身份的异步只读锁更新锚点。UI-element-only 宿主不显示自绘窗；尚未实现 TSF UIElement 协作。
- 鼠标动作在持有 owner/context 的 TSF 可同步或异步写锁内重新验证焦点/context/generation/session/revision，排队期间不预先生成 commit。x64/Win32 fake 与真实词库受控探针均核对显隐、不抢焦点、旧按下保护、鼠标翻页/上屏与文档布局跟随；强制延迟写锁后，新按键或焦点丢失使旧动作取消。焦点恢复后完成第三轮选词，最终从 EDIT 和 TSF context 核对 `mmm`/`你好你好你好`。候选窗仅原生表现层，业务与授权仍在 Rust，阶段性边界见 ADR 0018。
- TIP 在固定的 16 个管道槽中轮转扫描，有忙槽时在原端到端硬时限内短重试；每个新 handle 都重新复核服务端身份，身份异常直接拒绝而不跳过。首次全部端点缺席立即 fail-open；曾认证后允许在同一时限内等待 Broker 重启。前景焦点恢复重置退避，该时序已由上述焦点恢复探针覆盖。
- Broker 启动模式已 fail-closed：启用 debug assertions 的开发构建才接受 `--fake` 或带显式 DLL/shared/user 路径及部署标记的 `--rime`。默认 release 只接受无参数启动，从 Known Folder API 构造固定 Program Files/LocalAppData 布局并核对当前映像位置；随后在任何用户目录创建、Pipe 绑定或 `rime.dll` 加载前审计 Program Files 与完整安装树的 owner/DACL、reparse 和文件硬链接数。真实初始化失败不会静默降级。Windows verbatim canonical 路径在传给 librime 前正规化，已实证避免 librime-lua/用户库路径失效，见 ADR 0038。
- ADR 0029 补齐安装态 Broker bootstrap：只有精确 `Program Files\Mo\tip\<arch>\mo-tip.dll` 获得固定相邻 Broker 的进程创建权限，搬迁/仓库布局保持 connect-only。无 shell/参数/继承 handle/控制台启动，Broker image reparse 拒绝；x86 对不可用的 ProgramFilesX64 Known Folder 只读 HKLM 64 位视图回退。端点缺失才启动，2 秒进程节流与 16 槽 first-instance 绑定保证唯一存活 Broker；并发宿主仍可能短暂创建多个 contender。连接后的 SID/PID/文件身份校验不变。
- release Broker 在完整验证机器资产后只逐级创建精确的 `LocalAppData\Mo\Rime`，拒绝路径逃逸、文件占位、每一级 reparse point 以及已有用户 `rime.lua`/`lua`；不再创建用户 `build`。staging 与 prebuilt 都固定到 Program Files 机器目录，用户目录只承载词典和学习状态。debug/release 安装布局测试、双架构 launcher 探针、fake 3 轮/架构故障矩阵均通过，见 ADR 0039。
- 安装模式将 DLL/shared/prebuilt/staging/相邻 OpenCC 固定在机器安装根，只把 user 固定在当前用户根；缺少目录、default/schema 标记或六份必要转换文件时在 Pipe 创建前退出，不自动部署或回退。完整布局 fixture 已验证字段隔离及每份转换文件缺失拒绝；release 子进程已证明诊断参数和仓库映像被拒绝。运行时 ACL/reparse/硬链接门已实现并以真实 Program Files 只读基线、SDDL/硬链接负例验证；真实安装资产加载、签名、持续 handle 防替换仍未完成。
- `mo-settings` 已建立普通用户设置的强类型 v1 合同：固定 Known Folder 相对路径、16 KiB 上限、精确枚举/布尔/候选范围、确定性编码、未来版本与未知/缺失/重复字段 fail-closed；仅文件不存在时采用默认值。同目录临时文件在 flush 后用 Windows replace/write-through 原子替换，基础测试覆盖首次保存、覆盖、碰撞和损坏恢复边界。图形前端已在 ADR 0043 接入；方案和简繁的会话初始配置及空闲边界切换见 ADR 0045/0046，Emoji 见 ADR 0050，候选数量等其余引擎选项仍未接入。
- 设置运行时计划拆分 presentation 与 engine preferences；其中方案、简繁和 Emoji 现已在新建会话应用，已连接 TIP 在当前预编辑完成后可自动换到新会话，其余引擎偏好仍未接入。Broker 维护 revision 化最后有效快照，并以协商式 `GetSettings`/固定 18 字节 payload 提供给同用户 TIP；新增协商式 `OpenSessionAck` 快照，准确标识本次会话实际使用的配置，避免两次请求间保存设置造成误判。运行中损坏返回稳定错误且不覆盖旧计划，debug Broker 不读取真实用户设置。x64/Win32 客户端在连接及显式刷新时双重校验快照，查询不进入逐键热路径；候选窗实际应用 System/Light/Dark 主题。协议与会话映射见 ADR 0042/0045/0046/0050。
- 首版独立 Windows 图形设置中心已由 Rust + 原生 Win32 实现。现在开放候选窗主题、候选注释显隐、五种输入方案、简繁模式及 Emoji，五个选项一次原子保存；后三项在当前输入完成后的安全空闲边界自动切到新 Broker 会话，不迁移当前预编辑。候选数、学习与隐私仍未接入/只读；当前锁定 schema 的页大小在机器预编译配置中固定为 5，不能仅裁剪显示列表来冒充改变引擎分页。首次打开不创建文件，保存逐级验证/创建固定 LocalAppData 目录并原子替换；损坏或未来版本文件禁用普通保存，只有用户明确“恢复默认设置”才覆盖。GUI 未在开发主机启动，未创建真实用户设置，见 ADR 0043/0045/0046/0049/0050。
- 设置保存成功后会向当前桌面的已激活 TIP 发出无数据的注册消息，TIP 在非按键消息路径向已认证 Broker 查询最新设置。主题或注释开关 revision 变化时重绘当前候选窗；旧 Broker 未协商设置 feature 或运行时文档损坏时保留会话与最后有效设置，传输歧义才断开并清除未提交预编辑。x64/Win32 多接收器广播与生命周期探针通过；真实安装宿主的即时效果待 VM 验收，见 ADR 0044/0049。
- Broker 已移除连接内的诊断 ASCII echo 状态：wire session token 映射到 Engine Actor 的 generation-safe token，创建、按键和销毁全部经过可替换后端的 Actor；跨 session snapshot 使用同一全局 revision 顺序，断开时回收仍存活的引擎会话。真实启动使用 `RimeBackend`，确定性的 `FakeBackend` 只保留为显式测试模式。
- Engine Actor 位于进程级专用线程，thread-affine librime backend 在线程内创建和销毁。生产 Broker 改为 16 个独立命名、单实例的受保护管道槽，每槽一个有界工作线程，共享 Actor；全部槽绑定及全部工作线程创建成功后才开始处理。原始 server handle 保留至槽结束，accepted stream 使用同一内核对象的副本，断开客户端后可复用而不重建名称。仍不授予客户端 `FILE_CREATE_PIPE_INSTANCE`，旧串行接口仅用于兼容测试，见 ADR 0019。
- x64/Win32 fake 与真实 rime-ice 客户端均同时保持 16 路连接，第 17 路在设置的总时限内失败，释放中间槽后新会话成功接入；原连接分别以自己的候选页 revision 提交自己的词，不被其他连接推进全局 revision 干扰。Rust 测试另行验证静默首帧客户端不会阻塞另一槽、重复 live accept 被拒绝、断开期间名称/DACL 不变、次槽冲突导致整池绑定回滚，以及并发生成的 1024 个 generation 非零且无重复。就绪信号在引擎初始化及工作线程创建后发出；真实 smoke 的错误分支已改为终止 owned 子进程并有界读取日志。
- 服务端 connect/read/write 已改为 overlapped I/O，固定槽/权限/唯一 Actor 不变。首帧整个解码共用 2 秒；已认证空闲连接用 pending 单字节 read 等待活动，随后完整 header/payload/短读共用 2 秒 assembly 时限，移除 1 ms Peek 轮询。完整编码回复的 header/payload/短写共用 2 秒；flush 不调用会等待对端读空的 FlushFileBuffers。超时按操作 CancelIoEx 并等待完成，歧义结果退出连接，不重发字节或引擎命令，见 ADR 0020。
- 新增真实内核故障测试验证未读回复填满管道后写超时、pending read 取消排空后复用、零预算不提交字节、flush 在客户端尚未读取时返回、分片 payload 不续期及正常空闲不耗 assembly 预算。Broker/共享 Actor 测试对截断与 CRC 损坏的 Space 请求核对错误、每个旧 backend session 销毁、恢复后空预编辑起步与一次提交；两个坏 Space 均未进入引擎。x64/Win32 fake 与真实词库 probe 另行通过三轮整池满载/释放/重新连接，核对新会话与不重放旧词。
- Broker 故障与恢复子阶段已落地：未命名进程内停机事件唤醒 connect/partial/idle/backpressured I/O；连接先回收 session、全部槽 join 后 Actor Shutdown/finalize。某槽诊断失败先取消其余槽，worker panic 直接 fail-stop。初始化 30 秒、排队及每个 native 请求 5 秒、finalize 5 秒、整池停机总预算 15 秒；独立 watchdog 不空闲轮询，也不让逐 session 预算累乘。八类子进程卡死/panic/总停机注入核对到达故障 marker、3 秒内 fast-fail，而不是把任意非零退出当成功。现有闲置 EngineClient 不阻止 Shutdown，见 ADR 0021。
- TIP 实际 Broker 退出/重启探针发现并修复延迟鼠标动作失败时旧拼音偶发残留：持有原 RW cookie 清除自身未提交 range 后断开，不嵌套申请写锁，也不重放歧义动作。故障探针保持文本锁延迟直到 owned Broker 确实退出。两次退出分别覆盖待执行选词与已提交文本；缺席期间不吞键，恢复后分别准确提交一轮，最终 EDIT/context 核对五轮词。harness 不写输入注册状态，不杀外部进程；现有 CI smoke 默认包含此回归。
- 提交前 fake 双架构各 100 轮故障回归通过（每轮两次实际退出）；真实词库的基本双架构链路及 Win32 单独 20 轮通过，但完整高频命令也出现过下述首键超时，不能算压力验收通过。真实 smoke 全程异步排空日志，失败附带 owned Broker 日志；fake/真实清理均确认子进程真正退出后才进行下一 probe/删除临时数据，修复 Kill 后直接清理导致的偶发文件占用错误。
- 首键子阶段补齐 Actor 排队/执行与客户端 write/header/payload/cancel 分段诊断、候选显示阶段只读 metadata；只在开发态显式开启，不记录输入内容、不阻塞 Actor 写日志。客户端改为 QPC-backed 单一 deadline，50 ms 不变；完成和解码后再次拒绝到期结果，迟到 snapshot 不发布、不重发，零预算/精确到期/有限 MAXDWORD 算术及真实 IPC sentinel 回归通过，见 ADR 0022。
- ADR 0022 资源子阶段的最终真实预编译资源回归在默认关闭诊断和显式开启诊断两种构建下，均完成 x64/Win32 各 20 轮故障检查与完整 IPC/pool/UI/edit 检查；每轮两次实际退出。该有限回归不取消 100 轮命令的失败。只读状态复核仍是 COM 双视图缺失、profile 未注册/启用/激活，没有设置默认输入法。
- 候选重入子阶段已复现并修复定位期间 TF_LC_CHANGE 后重显旧位置：持有 context/range 引用、捕获饱和 epoch 与会话/page 身份，宿主查询完成和 popup Update 前后复核。A/B 明确得到 key/hook/通知正常但 stale_visible=1；保护开启的默认 fake 双架构各 100 轮通过。该确定性缺陷不是此前偶发消失的已确认全部根因，见 ADR 0023。
- TIP 的重连/Hello/OpenSession/按键现共用 DispatchKey 入口的一份绝对 50 ms 传输 deadline，context 切换和按键错误不额外等 CloseSession ack；IPC 校验过期绝对 deadline 不因新连接而续期、sentinel 不发布以及新会话从空预编辑起步。此预算不保证同步宿主 COM/文档/渲染或内核取消排空的实际返回上界。
- 候选保护/共用 deadline 落地后，Win32 独立真实命令 100/100 轮通过，含同一个 x64 Broker 的完整 IPC/pool/UI/重入/文档与实际故障恢复检查；x64 完整命令仍在第 82/100 轮因首键超时失败。Win32 通过不构成双架构整体压力或普通宿主通过。
- ADR 0023 收尾恢复默认关闭诊断构建，真实预编译词库 x64/Win32 各 20 轮及完整 IPC/pool/TIP 检查通过，重入 fixture 使用有效 context view。Rust 105 项运行时测试、1 项 compile-fail doc test、fmt、默认 all-targets Clippy 通过；16 项内存注册事务与 20 项 OpenCC 检查通过。只读状态仍为双视图 COM 缺失、profile 未注册/启用/激活，未改变 Windows 输入状态。
- IPC 有 64 KiB 硬上限、最小可接收响应协商、CRC32 破损检测、UTF-8 校验、版本协商、严格递增 request id、connection generation、会话隔离和会话数量上限。
- ADR 0024 无输入资源准备子阶段：实际两个 Simplifier owner 在 Broker ready 前初始化，私有保活会话无 key/commit/clear/wire token。缺导出/缺字典均准确拒绝且不宣布就绪，忙会话拒绝不清空，Emoji/Unicode 路径验证通过。同一最终 DLL 的直接首键 process_key 对照约 23.7 ms → 1.19 ms，约 94.8 ms 准备成本前移；不构成机器冷启动指标。最终自构建 prepared/trace 完整命令 x64/Win32 各 100/100 轮通过（共 400 次实际退出），800 个被记录首键传输计时为 2.017–5.221 ms。当前受控压力样本通过，不升级为真实桌面宿主或全面冷启动通过，也不抹去此前超时/候选消失的负向证据。
- ADR 0024 收尾默认关闭诊断的完整命令双架构各 20/20 轮及 IPC/pool/UI/edit 检查通过；Rust 默认 109/trace 111 项与 compile-fail、四种 Clippy、五项 release 启动负向 smoke 通过。只读状态仍是双视图 COM 缺失、profile 未注册/启用/激活。来源路径守卫的四项匹配/外部路径/缺项/重复项检查通过，最终实际 CMake cache 也已核对全部 pinned header/library 来源。
- ADR 0025 转换资源搬迁子阶段：v2 只读 DLL 相邻 `opencc`、恰好 Emoji/繁体两个不同 owner，旧 v1 不回退。最终干净产物的 28 项文件边界、27 项真实引擎搬迁/解析、4 项准备及 3 个 Broker ready 前拒绝通过；33 份资源/11 份构建快照和 51 项 ABI 通过。五项 builder/四项 cache/七项生成工程守卫通过；已更正 deferred source property 未实际作用于 wrapper 的 W4/WX 问题，旧 runtime 的该选项意图不作为实证，TIP 独立严格编译证据不受影响。最终 prepared/trace 完整命令 x64/Win32 各 100/100 轮（400 次实际退出）通过，不升级为真实桌面宿主或机器冷启动通过。
- ADR 0025 默认关闭诊断收尾的完整命令 x64/Win32 各 20/20 轮及全部 IPC/pool/UI/edit 检查通过（另 80 次实际退出）；default 日志已核对两套 20 个 trial 和完整成功 marker。Rust 默认 109/trace 111 项及 compile-fail、四种 Clippy、doc、五项 release 拒绝、16 项注册策略/20 项既有 OpenCC 检查及八份 AST 通过。当前 registrar 只读状态仍为双视图 COM 缺失、profile 全 false，未更改默认输入法；G2/G4 保持部分通过、G3 未通过。
- ADR 0026 素材真实回归首条命令 x64 10/10、Win32 6/10 成功，Win32 第 7 轮在 stop cycle 0 前出现 candidate visibility mismatch；default 未记录具体 reset/阶段，根因仍未定位。保留负向日志，不把并发或 DLL 搬迁当作未经证明的原因。独立完整复跑同产物双架构各 10/10 通过；增加构建路径安全门后的最终 `mo-windows-stage-safe` 同产物双架构各 10/10（40 次明确退出）及 7 组 prebuilt-only golden 通过，但不据此宣称偶发问题已修复、正常宿主或日常使用稳定。
- ADR 0027 先确定性复现并修复 owner termination 遗留未提交拼音；匹配 composition 使用宿主 write cookie 清理自己的 live range，不影响 committed prefix，不再等待 CloseSession 或提交/重放候选。进一步稳定复现旧版双架构在清理写回调错误接受重入输入；最终固定范围、清空缓存按键、终止期间 fail-open/拒绝鼠标动作，并重验范围归属。探针新增普通终止/待写锁动作取消/写回调真实重入与恢复矩阵；默认关闭诊断仍有生命周期和断言检查点，trace 新 IID 防止扩大 buffer 的旧调用者越界，旧探针实证拒绝。非激活测试隔离与安全入口已实现，18 项 preflight/环境恢复检查通过；不是系统注册或安装。
- ADR 0027 中间三组非激活真实 trace/实际素材 default/fake 双架构各 20/20 通过；该中间素材尚无最终写重入保护，保留作负向对照。加保护的 trace x64 20/20、Win32 9 轮后第 10 轮 stale-press 失败；最终统一诊断 trace x64 20/20、Win32 1 轮后第 2 轮再次在成功按键后收到匹配 composition termination（传输 3194µs/dispatch 3614µs，owner active/foreground/sent 均 0）。这些样本明确保留，不能说抢焦点足以解释全部问题，也不能通过忽略真正终止通知“修复”候选显示。最终 source-matched 137-file 开发素材已重建，阶段回归见 ADR 0027；G2 仍部分通过。
- ADR 0027 最终素材实际 default 与独立最终 source/fake 各双架构 20/20 完整通过，两组合计 160 项真实终止写回调重入 / 160 次明确 Broker 故障退出；7 组 prebuilt-only golden、preparation/Actor、79 项素材与 18 项入口检查、Rust default 109/trace 111+compile-fail、default Clippy/fmt 通过。最终只读 profile 全 false/COM missing，137-file 清单 SHA-256 `0CE4054EBFD6329CD12C4545C176ABE200DE3B80C37B7664C618DE7E4DE06109`，68-source 对应。与失败的 final trace 分组记账，不升级为所有压力模式、真实宿主或日常使用稳定。
- ADR 0028 将上述偶发终止定位为未注册探针的真实 Thread Manager 同时激活当前系统文本服务：失败时 document focus 未丢失，异步终止栈经 TextInputFramework/MSCTF 消息路径进入。受控模型改用 NOACTIVATETIP/NOACTIVATEKEYBOARDLAYOUT 并验证 active flags、非激活 owner 和每轮恰好两次显式终止。隔离 trace 双架构各 100/100，最终诊断 IID 源码各 20/20；合计 480 次写回重入 fail-open、240 个完整故障 rounds / 480 次明确 Broker 退出，无额外终止。旧 v3 调用者双架构均在读 buffer 前获 E_NOINTERFACE。当前 70-source/137-file 开发素材清单 SHA-256 `F57A5B2966AD2B652FDEFCA6C93BFC1FA9D3F54136FE05F3F52B0CAAA3FF7E14`，79 项素材策略、7 组 prebuilt-only golden 及素材双架构各 10 轮通过。它关闭测试模型污染，不替代真实注册应用验收。
- Windows Named Pipe 使用 `LOCAL` 命名、当前 logon SID 受保护 DACL、`PIPE_REJECT_REMOTE_CLIENTS` 和 identification-only SQOS；服务端读取首个有界帧后模拟客户端并复核 logon SID，失败路径不进入 Broker 状态机。
- Named Pipe 已从真实内核对象读回并核对 protected DACL/唯一 ACE/SID/权限掩码，同时通过远程拒绝标志、端点逃逸拒绝、静默客户端首帧超时和 `Hello -> HelloAck` Broker 往返测试。
- Named Pipe 负向测试已证明当前登录会话不能在任一槽创建第二服务端实例；已认证连接从首个可用字节起采用 2 秒完整帧 assembly deadline，半帧超时断开客户端后复用 retained listener，完全空闲连接不会被误杀。静默首帧连接采用 2 秒时限，但已认证空闲连接仍占用一个槽。
- TIP 在发送 `Hello` 前用 `GetNamedPipeServerProcessId` 锁定服务端 PID，复核服务端进程与宿主属于同一 logon SID，并比较进程映像与预期 `mo-broker.exe` 的卷序列号/文件索引。x64/Win32 负向探针均证明错误映像身份被拒绝，随后正确身份仍可完成 IPC 与两轮 Edit Session 上屏。生产路径只从 TIP 自身固定安装布局推导；不接受环境变量或当前目录覆盖。
- 诊断 TCP transport 已通过真实 loopback framed I/O 测试；它不构成生产传输安全结论。

未通过：

- 完整注册宿主矩阵仍未完成。2026-10-03 已有 Win10 x64 Notepad 路由、预编辑/候选及精确提交和设置的有限 VM 验证（临时修复 DLL）；正式 composition 契约、WinUI 等其余宿主与冷启动验收仍缺证据，不能升级为完整 G2 通过。混合 DPI/多屏人工矩阵、双拼真实宿主输入习惯，以及真实 schema 标签/注释的视觉核对和高亮/页边界表现仍待完成。
- 连接池真实多应用宿主/满载恢复矩阵、已认证连接空闲租约、严格输入延迟指标；当前 overlapped I/O 仍在固定连接线程内等待，不是 IOCP 全异步调度。watchdog 约束进程健康，不声称原生操作可被安全取消。协调停机的生产服务控制/托盘/更新接入和自动重启仍未实现；极端内核/驱动不完成取消尚未注入。发布签名、持续防替换、设置迁移、通知在真实桌面宿主的验收、引擎偏好的活动会话迁移及其余偏好、AppContainer/WinUI 仍未完成。
- 实际崩溃回归目前只覆盖上述受控文本存储。engine commit 后/TSF 写入前、部分文档写入后的歧义故障及普通宿主矩阵仍未注入；不声称跨崩溃 exactly-once 或未提交输入不丢失。
- 机器冷启动和更广宿主/候选生命周期矩阵仍未通过。历史高频词库的首个 N 超时主要位于引擎转换；Emoji 延迟加载/weak owner 重复卸载有源码与独立消融证据。仅保活+预编译时，完整 x64 命令第 82/100 轮出现 Actor 55,959 µs、排队 3,469 µs 超时，未进入该命令 Win32 压力段，更早失败也保留。此前 x64 第 24/100、3/100 轮候选未显示（按键未超时），状态变化全部来源尚未确认。ADR 0024 prepared 自构建产物的双架构各 100 轮样本现已通过，但不据此宣称旧候选消失问题全部根因已确定。未放宽 deadline、禁用 Emoji 或自动重发；历史负向证据见 ADR 0022/0023。
- Windows 11 x64 真实桌面宿主矩阵；本次仅在 Windows 10 22H2 验证编译和 COM 加载。

## G3：安装——未通过

- 2026-10-03 Win10 VM：0.0.9.11 基础生命周期和 0.0.9.11 → 0.0.10.0 六阶段回滚/升级矩阵完整通过；两处失败注入与缺失 marker 修复拒绝符合预期，新旧产品替换及最终卸载通过，默认输入法未变，文件/ACL 校验通过。见 [Win10 实测](WIN10-INSTALLER-EVIDENCE.md) 与 `build/win10-evidence-clean-v1/Matrix0100`。真实桌面输入、loaded-TIP 升级/重启及发行项仍待验收。
- 版本契约纠正（ADR 0053）：下面历史记录中仅第四段递增的 0.0.9.x 包对曾通过旧 host 哈希/身份检查，但 MSI 忽略第四段，不能作为有效 MSI major-upgrade 证据。现在 host verifier、kit 生成器与 guest policy 均拒绝这类版本对；39 项 policy 通过。

- 对 Emoji 版最终 ProductionShape stage 的非并行真实词库恢复压力复跑：x64/Win32 各 10/10 轮通过，每轮两个真实 Broker 退出，合计 40 次退出/恢复；七组机器数据 golden、准备边界、候选/文档保留和无 commit 重放检查仍通过。这只覆盖受控诊断 Broker 与隔离 TIP 宿主，未量化已安装首次切换 `<500 ms`，也不替代普通应用或 VM 安装矩阵。
- Emoji 开关源码已重建 ProductionShape 与 DevelopmentTest 两份 138-file development stage；当前 ProductionShape manifest SHA-256 为 `7B4AA5BBE0C4E4618C8B2E03EED957131655DD8D220415452DE7DD775ECE2D77`。89 项 staging、21 项作者层、七组机器词库 golden、真实 librime/TIP 双架构冒烟及两份 stage 各架构一轮双次 Broker 故障恢复通过。未签名 ProductionShape `0.0.9.5` MSI/Bundle SHA-256 分别为 `0CB9A5B8CB3E3F45E24C72D6420800C902CA65F9CFB86DA4993D7D073D3C0E6D`、`2C1E651EBF6A3D083F91B54E19DDB62A3D08777B379751CA5A4C369E15D33B5F`；linked 反向核验和 MSI ICE 通过。DevelopmentTest `0.0.9.5 → 0.0.9.6` 同载荷升级对、clean 与 matrix VM kit 已哈希绑定并通过 33 项 VM 策略检查。尚未执行安装、回滚或升级，也未签名。
- 新候选详情源码另行构建了带固定故障注入的 DevelopmentTest stage（manifest SHA-256 `4236F9591E4F053B088B33D9AA36D11879E5F34DDE68877BE3A2204282346E85`），与 ProductionShape 分离。未签名 `0.0.9.3 → 0.0.9.4` 升级对分别完成 linked 反向核验和无警告 MSI ICE；同载荷、不同 ProductCode/Bundle 身份及升级代码经 `verify-linked-upgrade-pair.ps1` 核对。clean lifecycle 与 rollback/upgrade VM kit 已绑定新 stage、Bundle 和 registrar 哈希，清单均通过 `Assert-MoVm*Kit`；33 项宿主 VM 策略测试、机器数据七组 golden、真实 librime 候选以及 x64/Win32 各一轮双次 Broker 故障恢复通过。本机未检测到可调用的 VM 管理程序，未执行安装、回滚或升级；两个 kit 均显式禁止直接授权执行。
- 候选详情与设置开关源码已重建成新 development stage：`mo-stage.json` SHA-256 `47F9A6F90DE1D82BFDA63BF7E985D144F87D6A415AFA584234A3C0972F91A4CF`，132 个 payload/6 个 evidence 文件。89 项 staging、21 项作者层、机器数据七组 golden、真实 librime 候选与 x64/Win32 各一轮双次 Broker 退出恢复通过。未签名 ProductionShape `0.0.9.3` MSI/Bundle SHA-256 分别为 `AC0B6674A47A5165D2CE4216CBCB181F7D5FA94CE51B39E0F66D68DECFA81162`、`0F3D33FD475EBDDDABBE296C4BAA7351ADC85CFFD7B302C56563828CD922BA2E`；linked 反向核验和无警告 MSI ICE 通过，未执行安装。
- 发现先前 `0.0.9.1` 的五个 Mo PE 依赖未链入 Bundle 的 Visual C++ 可再发行包。发布形态现对 Rust 显式开启 `crt-static`，对 MSBuild/CMake 原生目标使用 `/MT`；stage verifier 解析普通及 delay import 表并拒绝动态 VC 运行库依赖。新 132-file stage manifest SHA-256 为 `42ABB1B9CC788515E0541EC76544E8E4687EFE7B55F4CEAC8F156A5919292343`，六个 PE 的导入由 `dumpbin` 独立核对均无 VC 可再发行包 DLL。89 项 staging、21 项作者层、机器数据七组 golden、真实 librime 和双架构故障恢复通过；CMake x64/Win32 ABI 探针及八个 PE 导入检查也通过。未签名 ProductionShape `0.0.9.2` MSI/Bundle SHA-256 分别为 `D1DD8D1B2BCBEE74DE6E5E40CD8CBB3E246C39A7B90C132CF3F3B50AB1131F48`、`3DB2195050579B7F4C49AB6008A70DBE70890DE4C3A68BBB362C8E831C52069D`；linked 反向核验和无警告 MSI ICE 通过，`install_executed=false`，见 ADR 0048。
- `build.ps1 -ValidateMsi` 现将无 ICE 警告的静态校验纳入可选构建门；使用当前 132-file stage 的 ProductionShape `0.0.9.1` 重建及 linked 验证通过，证据明确记录 `msi_ice_validated=true`、`install_executed=false`。此次 MSI SHA-256 为 `7D73D0EA099140637299A851168387B1062834E322F05110C0044A1F2222F06C`，Bundle 为 `2AAF3631EC5831B8BD9E9181093A93C6BD838E82351C0B339577EB585B52DE5B`。同一 stage 的 SPDX/通知草案与绑定此次 MSI/Bundle 的签名顺序计划重新生成并各自验证通过，仍未签名或取得发行授权。
- 2026-09-28 MSI ICE 静态门首次在当前主机执行：原 `0.0.9.0` MSI 报 ICE69 x86 COM 跨组件文件引用警告；改为安装目录属性后，当前 stage 链接的未签名 ProductionShape `0.0.9.1` MSI/Bundle 通过反编译与嵌入字节核验，`wix msi validate` 返回 0 且无警告/错误。MSI SHA-256 为 `B32083D8A2CA7116C8458DB6F3F3ADD173D35920696E60973D82CF06402C6D20`，Bundle 为 `259DAD5D585C4F2F3F758B95D1E560EC9AA65635D462F103672AB27D1EAF3D50`。验证器现在把 ICE 警告视为失败；未运行任何安装、修复或升级，见 ADR 0047。
- 2026-09-28 从含会话设置应答的当前源码重新生成 development stage：`mo-stage.json` SHA-256 为 `DE30A6668FAEBB76F7001E0821C458B11C2A3DB4F668C2DFC7C330F196685C07`，132 个 payload 文件、6 个 evidence 文件。清单、staging 与作者层策略通过；stage 中的机器数据七组 golden、真实 librime 候选探针及 x64/Win32 各一轮 Broker 双次崩溃恢复通过。新的未签名 ProductionShape `0.0.9.0` MSI/Bundle 已链接并反向核验，SHA-256 分别为 `724D476143862F7710569C171CA1A333A884F426B6A6CF33ED892D477DF33854`、`8B6C19AFE36BF3215C2E61C3E48C51DBDE4184B7599D46AA4DD80DDA36F462BC`；未执行 MSI ICE、签名或安装，不能分发。
- WiX v4 作者层现覆盖完整 132-file payload：确定性生成器为设置中心、Broker/registrar、x64/x86 TIP、librime/OpenCC 与 rime-ice 源数据/预编译数据逐文件生成稳定 component/GUID，并从 XML 反向重建安装路径，锁定 Program Files 根、设置中心开始菜单快捷方式、bitness、key path、双 COM 视图且排除 evidence。最终离线重建的 85-source/139-file ProductionShape stage manifest SHA-256 为 `605017E0465896FDEFD78A8DD13036A4B50FAADB2609EA3B1569A14BA2751725`，88 项 staging、20 项完整作者层/篡改拒绝、7 组机器数据 golden；最终 stage 双架构各 10 轮故障恢复共 40 次明确 Broker 退出通过，同时保留此前 v1 并行运行 x64 第 9 轮首键 78 ms 超时的负向样本。仓库局部、精确哈希锁定的 WiX 4.0.6 已链接并反向核验未执行的 `0.0.8.0` ProductionShape：MSI SHA-256 `6B1FA032E2DD6DE094C0E4322E47BDE713D84B24B9DDDAE34FBC0975C00F88DD`，Bundle SHA-256 `93BB9186E06D05E14AF87E58B210009C33D4401CD2EAC747816091F1C60A1D29`；12 项合规、8 项来源材料、7 项签名顺序检查通过。MSI ICE 尚未运行。
- 机器级 TSF profile/category 已有 MSI 事务协议：install/repair/remove 延迟动作在变更前把 profile/category 两个 presence bit 写入 Program Files 固定标记，rollback 恢复原状态，commit 删除标记；全新安装遇到任一既有 Mo profile/category 会在变更前拒绝，repair/major upgrade 才可刷新，且旧包升级移除不先拆共享 profile。MSI 禁止关闭 rollback。动作使用内嵌 x64 registrar、无用户输入路径、以 non-impersonated 系统上下文执行。x64/x86 新鲜构建与 staged x64 registrar 的 8 组标记状态/重复创建/无残留自测通过，但尚未在 VM 注入真实 MSI 失败，也未以提升权限调用 TSF 变更 API。
- 开发态 registrar 已补齐当前用户 COM activation：以显式 WOW64 视图分别注册 x64/x86 `InprocServer32`，拒绝相对/缺失文件与冲突路径；`status` 可读回 COM/profile 启用状态。隔离测试 CLSID 已连续两次完成双视图写入、读回和无残留清理，未注册或启用 Mo profile。
- 已实现注册宿主 smoke 的权限拆分：提升脚本只管理机器级 TSF profile/category，普通权限脚本临时管理 HKCU 双视图 COM 与当前用户启用，并通过系统 `ITfKeystrokeMgr` 驱动 x64/Win32 探针后在 `finally` 中回滚用户状态。当前非提升开发会话只验证了双架构编译、权限门和干净状态，尚未执行需要人工提升准备的真实注册路由，因此 G2 不据此升级为通过。
- fake 与真实词库 smoke 均已支持 `-Registered`，共享严格用户态事务：提升的测试/Broker 被拒绝，准备不足在构建/部署前拒绝；部分 native 写入失败仍触发独立清理，清理错误或状态残留不再只告警后报成功，检测到外来 COM 路径则保留并要求人工审查。18 个内存策略场景通过，不修改 Windows 输入状态。当前管理员准备仍缺失（profile=false），两种 registered 入口已实证在此前置条件下拒绝且没有残留；真实注册路由仍未运行。具体步骤见 `REGISTERED-TEST.md`。
- 构建脚本默认拒绝生成不可部署安装包；工具链准备只接受官方 NuGet 大小/SHA-512 锁定的 WiX CLI 4.0.6 与 Bal/Util/Dependency 4.0.6，默认离线且不做全局安装。即使工具链存在，也必须显式传 `-AllowDevelopmentBuild`、使用新 build 子目录，并在成功返回前反向核验 linked 产物；产物名带 `development-unsigned`。
- linked 构建现分为 `DevelopmentTest` 与 `ProductionShape`。registrar 的固定失败命令由默认关闭的编译宏控制；生产形态的 MSI/Burn 也在预处理阶段移除安全属性、失败 action、隐藏变量、属性转发和链尾失败包。双架构默认/故障原生探针以及两种 flavor 的 `0.0.3.0 -> 0.0.4.0` 实际 linked 升级对均通过反编译、提取、字节和身份核验；开发 VM lifecycle/matrix kit 已按带 flavor 的 evidence 重新绑定。生产形态仍是 development stage、未签名且不可部署，不改变 G3 结论，见 ADR 0034。
- Bundle 已在机器 MSI 后串联 vital、`PerMachine=no` 的 current-user finalizer：在机器本地 HKCU 路径检测精确 v1 marker，以固定 `burn-user-finalizer` 前缀和全局 action 条件追加 install/remove/repair 或两个逆向 rollback，用稳定 dependency provider 参与升级引用计数。linked Burn manifest 已确认锁定版本的整体动作值为 Uninstall=4、Install=6、Repair=8。native 只接受五个精确子命令，拒绝提升、Session 0/AppContainer 与任一 HKCU COM shadow，启用前核对 Program Files 双架构文件、HKLM 双 COM 视图和 profile/category；named mutex 串行化同会话操作，输入 API 前持久化并读回含原 enabled bit 的 undo journal，跨 Burn 进程精确恢复。140 个状态组合、四类进程上下文、隔离 marker/六类 journal 真实 HKCU 清理测试及 direct/Burn-prefixed 五命令缺前置条件无副作用拒绝均通过；只使用 `InstallLayoutOrTip` 的 0/UNINSTALL flags，不抢默认输入法。尚未执行真实 TSF 用户变更或 linked Burn 回滚；混合 scope 的 WIX1140、MSI ICE、升级/多用户行为仍是 VM/发行门。
- 已新增一次性 VM 测试包与 Windows PowerShell 5.1 生命周期驱动：host 侧把 linked Bundle、staged registrar、stage manifest 和 inventory 绑定；guest 侧以虚拟硬件识别、MachineGuid/系统卷/计算机名/用户 SID 哨兵、双显式开关及非提升令牌防误运行。早期 `0.0.4.0 -> 0.0.5.0` rollback/upgrade matrix kit 曾从对应源码和锁定归档离线重建，实际 inventory 与 33 项策略/篡改测试通过；其 DevelopmentTest stage 双架构各 10 轮再获 40 次明确 Broker 退出。当前新基线为上文 `0.0.9.5 -> 0.0.9.6`，仍只证明 harness 和传输包 fail-closed，不声称安装生命周期通过。见 ADR 0032/0040/0050。
- 尚无真实 TSF API/MSI/Burn 失败注入、升级/修复/卸载 VM 矩阵、签名、首次启动性能或“不抢默认输入法”组合测试。
- 自动拉起与首次用户目录代码已完成，但尚未把 release payload 安装到 Program Files 做“注册 TIP -> 拉起 -> 首次目录 -> 普通应用输入”的组合验收；G3 因此仍未通过。
- 当前 development authoring 不得分发。

## G4：数据与许可证——部分通过

- 会话学习策略的锁定 core+Lua 运行时已重建；真实用户词典导出测试确认 `mo_disable_learning` 开→关→开时学习条目/词频为 0→1→1。设置中心新增“启用本地学习”和“隐私模式（暂停学习）”，Broker 按有效学习状态传会话选项，TIP 在空闲边界替换会话。旧运行时负控证明仅设置选项不足以阻止学习。最终源码的 ProductionShape/DevelopmentTest `learning-v2` stage 各有 132 个 payload 文件；`0.0.9.7` ProductionShape 和 `0.0.9.7→0.0.9.8` DevelopmentTest MSI/Bundle 已通过链接反向核验与 MSI ICE，clean/matrix VM kit 已绑定哈希。SPDX/通知草案、9 份来源归档、15 份许可材料及签名计划已核验；法律审查、签名和 VM 安装仍未完成。staged x64/Win32 后续各连续 10 轮故障恢复通过（共 40 次明确 Broker 退出），但 x64 曾出现 47 ms 首键未消费；2026-09-29 追踪又确认原生 `process_key` 有 95 ms 峰值，另一轮后续键的原生处理及候选读取合计约 86 ms。偶发超时仍未解决，见 ADR 0051。

- Emoji 版 `mo-windows-stage-emoji-v1` 的 132 个 payload 文件已重新生成并独立核验 SPDX 2.3/第三方通知草案，9 份精确源码归档和 15 份许可证/通知材料也已与同一 stage 哈希绑定并核验；12 项合规、8 项材料负例测试通过。`0.0.9.5` ProductionShape MSI/Bundle 的六个内层 PE → MSI → Bundle 签名顺序计划已绑定实际未签名哈希并通过 7 项检查。材料标记 `technical_materials_complete=true`，但 `legal_review_complete=false`、`release_authorized=false`；未发生签名或法律批准，见 ADR 0050。
- 针对候选详情的新 132-file stage，SPDX 2.3/通知草案、9 份精确源码归档和 15 份许可证/通知材料均重新生成并验证；签名顺序计划已绑定 `0.0.9.3` MSI/Bundle 并验证。仍未进行法律审查或实际签名，`legal_review_complete=false`、`release_authorized=false`。
- 静态 CRT 新 stage 的 132-file SPDX/通知草案、9 份精确源码归档和 15 份许可证/通知材料均重新生成并验证；签名计划已绑定 `0.0.9.2` MSI/Bundle 并验证。相关证据仍固定 `legal_review_complete=false`、`release_authorized=false`。
- 2026-09-28 已针对当前会话设置应答的 132-file stage 重新生成并验证 SPDX 2.3/第三方通知草案，以及 9 份精确源码归档和 15 份许可证/通知材料；签名顺序计划也已绑定新链接的、通过 MSI ICE 的 `0.0.9.1` MSI/Bundle 并通过复核。全部仍是 development-only：法律审查、签名与发行授权均未完成。
- rime-ice 2026.06.30 锁定到 `6810e8916d160498620a16fef2135956fecbd485`，source archive hash 已记录。
- 已从源部署完整 rime-ice 数据并运行真实 golden smoke。
- 新增固定安装布局的开发素材准备管线：核对 core+Lua/v2 来源与 33 份转换资源，从锁定 rime-ice archive 的 64 份输入全新编译 29 份 schema/词库，并从 84 份 Mo 源码快照全新构建 release Broker、Rust 设置中心和双架构 TIP。首份归档可由精确 Git commit 生成，后续也可直接消费同 SHA-256 归档离线重建，二者严格二选一。132 个 payload/6 个 evidence 文件有严格清单与依赖 receipt，原样保留上游归档/LICENSE/Credits；88 项构建清单/拒绝测试通过。机器 staging/prebuilt 路径及带恶意用户 Lua trap 的 fixture 完成 7 组 exactly-once golden，未在线生成词库或执行用户脚本。所有产物仍 development-only、不可安装/分发；不是签名、完整 SBOM 或正式许可结论。见 ADR 0026/0030/0039/0040/0043/0044。
- 已新增 hash 锁定 OpenCC 1.1.9 + bundled Marisa 的本地构建态编译工具，把锁定 Emoji/补充字典生成 `.ocd2`，读回核对全部 4857/1498 条 key 及有序 values。20 项完整性/负向检查通过；源文件与 manifest 保留，测试只复制到新 fixture。该 pack 未接入正式安装、签名更新或发行 SBOM，不把自声明哈希作为可信更新证明。
- 发现官方 librime Windows 资产静态包含 GPL-3.0-only `librime-octagram`。该资产现被明确限制为开发验证，不进入 Mo 发行物。
- 正式包必须从锁定 librime 源自行构建，插件采用允许列表；当前最小集合为 BSD-3-Clause core + rime-ice 必需的 BSD-3-Clause `librime-lua`。
- 新增 core + Lua 的允许列表开发运行时构建器，消费六份锁定 Git archive 与显式固定哈希工具包；拒绝旧输出、错误来源，禁用外部插件和 native 内容日志。OpenCC/core 共用 pinned Marisa 0.3.1，避免 bundled 0.2.6 覆盖库的头文件/ABI 混用。v2 只读取 DLL 相邻 `opencc` 资源，不使用 prefix/CWD/user/shared 搜索；Lua 只保留机器 shared 模块路径、清空 `package.cpath` 并只执行机器 `rime.lua`。构建前快照 12 份 Mo 输入及已验证 Emoji pack，format 2 provenance 记录策略/补丁哈希、33 份资源与 DLL 哈希。5 项 Lua 策略测试通过；旧运行时会触发的用户脚本哨兵在新运行时不执行。证据见 ADR 0024/0025/0039；这不是 Lua 沙箱、许可证批准或可发行包，签名/安装权限/逐文件 SBOM 仍未通过。
- rime-ice 资源仍按 GPL-3.0-only 独立边界处理；默认捆绑前仍需逐文件 SBOM、第三方通知、对应源/修改记录和正式许可证审查。
- 已新增与真实 stage 哈希绑定的 SPDX 2.3/通知草案流水线：132 个 payload 文件全部且仅匹配一个归属组件（Mo Broker 1、Mo 设置中心 1、Mo 原生 3、librime DLL 1、rime-ice 93、OpenCC 30、rime-ice OpenCC 衍生 3），并记录组件/依赖包及锁定来源 archive。进一步组装并核验 9 份精确源码归档和 15 份许可证/通知，显式标记 rime-ice GPL 对应源；12 项合规及 8 项材料真实/负向测试通过。librime 组合 DLL 仍保留 `NOASSERTION`，法律审查/发行授权固定为 false，G4 仍只部分通过，见 ADR 0035/0036/0043。
- 已生成哈希绑定的 Authenticode 顺序合同：设置中心加入后，六个内层 PE 必须先签名和时间戳，随后从新封口 stage 重建并签 MSI，再重建 Bundle 并最后签外壳；禁止修改已封口 stage、只签外壳或让 DevelopmentTest 进入流程。7 项真实/负向测试通过。当前没有访问证书或时间戳服务，所有产物仍未签名。
- 一次性 VM 策略已补齐安装树信任审计：显式栈枚举在进入目录前拒绝 reparse，所有文件用 Win32 handle 要求硬链接数为 1；Program Files、`Mo` 根及全部后代 owner 只接受 TrustedInstaller/SYSTEM/Administrators，任何非受信 SID 的 write-like allow ACE 或 null DACL 均拒绝。install、repair、故障保持态和 major upgrade 后都重跑审计；release Broker 现也在创建用户目录、Pipe 或加载 native code 前执行等价运行时门。33 项 VM 策略测试、Rust 正负例及当前主机 Program Files 只读基线通过，但尚无真实 VM 安装结果，见 ADR 0037/0038。

## 下一检查点

 Mo 转换资源搬迁、开发素材/预编译 pack、宿主终止/探针隔离、固定安装态 Broker bootstrap、完整 payload/机器 profile 回滚，以及 current-user/Bundle 持久回滚作者层已分别闭环，见 ADR 0025–0031；受控 WiX 工具链、真实 linked 结构和 fail-closed VM lifecycle kit 也已闭环，见 ADR 0032。开发故障矩阵见 ADR 0033；production-shape 隔离见 ADR 0034；逐文件 SPDX/通知草案与来源锁定见 ADR 0035；来源材料和签名顺序见 ADR 0036；安装树安全审计与 Broker 运行时门见 ADR 0037/0038；机器-only Lua/prebuilt 边界见 ADR 0039；离线 stage 与最新 VM 基线见 ADR 0040；设置存储、运行时快照、原生 GUI/安装入口及实时提示见 ADR 0041–0044；协商式候选注释/标签见 ADR 0049，Emoji 开关见 ADR 0050。下一步优先在当前 Windows 10 x64 VM 的 0.0.11.0 基线上继续延迟及桌面宿主验收，补齐 loaded-TIP、登录/重启和其余真实引擎偏好；Windows 11 矩阵后续再做。系统路由验收仍需管理员明确准备，在 Notepad 验证 composition、候选窗、设置入口、保存后的即时重绘、自动拉起、首次目录和 Broker 故障恢复，再覆盖 WinUI/AppContainer/混合 DPI；步骤见 `REGISTERED-TEST.md`，不自动启动 UAC 或改默认输入法。librime 组合审查、实际签名、持续竞态防替换、多用户卸载策略、跨会话互斥与资源内容认证仍是发行门。

## 2026-10-03 Win10 native 延迟组件续查

主机隔离诊断将首键主要长耗时细分到中文/英文 translator 的 Prism、词表和用户词典查询；线程 CPU 差值较低并伴随进程缺页增量，尚未证明具体 I/O/调度原因。17 组直接 API 共 680 个合成键断言通过；3 组完整真实 TIP 首次仍 exit 1，同一 Broker 第二次均 exit 0。整文件无输入预读花费约 5.8 秒，首键仍约 65 ms，没有加入产品。生产代码、包与 deadline 未改，本轮未操作 VM，G2/G3 不升级。详见 [组件诊断与边界](WIN10-NATIVE-LATENCY-EVIDENCE.md)。

## 2026-10-03 Win10 映射页/Actor 续查

[隔离映射页试验](WIN10-MAPPED-PAGE-EVIDENCE.md) 的直接 API 某样本首键降至 1.5 ms，但完整 TIP 首次仍超时；Broker 就绪 7.1 秒，Actor 仍有 741 ms 样本。未修改产品，50 ms/G2/G3 验收结论不变。新增只使用合成数据的 Actor 计时探针，build、rustfmt、Clippy 均通过。

## 2026-10-03 Win10 代码页及启动分项续查

[代码页/映射页组合试验](WIN10-IMAGE-PAGE-EVIDENCE.md) 已保存可复现的隔离诊断补丁。完整 TIP 的 Lua filters 消融四次首轮均失败；单独代码页准备仍有 135 ms 失败。组合准备在第一版 DLL 上四次首轮完整通过，含默认 x64/Win32，但 Broker ready 1.866–2.431 秒。补启动计时后的新 DLL 双架构完整首轮/第二轮通过，复用目录就绪仍为 495/1,702 ms；没有满足 400 ms activation 或首次切换目标。

新目录 Actor prepare 1.53–5.42 秒、复用目录 334–352 ms，长区间主要细分到词库加载及用户词典打开，尚未拆开缓存/目录状态和具体 I/O 原因。生产 DLL、0.0.11.0 载荷及 deadline 未改，VM 没有新增输入验收证据，G2/G3 状态不变。18 组完整 probe 与四组 Actor 日志由 `ImagePages-v1` 清单绑定；日常工作继续在 develop，main 保留正式版基线。

2026-10-04 收尾 workspace fmt、Clippy -D warnings、tests 通过；开发态 Broker 的测试重建与历史探针身份分开记录，详见上方证据。

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
