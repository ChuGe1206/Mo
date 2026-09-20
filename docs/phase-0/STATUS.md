# Phase 0 状态

- 快照日期：2026-09-20
- 结论：Phase 0 已启动，G1 本机证据闭环；G2/G4 部分通过；G3 未通过。受控 TSF Edit Session 已贯通，项目仍不可安装或日常使用。
- 本机环境：Windows 10 22H2 build 19045（尽力兼容环境）、Rust 1.97.1 x86_64-pc-windows-msvc、Visual Studio 2022 17.14.37、MSVC 14.44、Windows SDK 10.0.26100.0。

## G0：工程基线——本机通过，远端 CI 待首次运行

- Cargo workspace 包含 `mo-domain`、`mo-engine`、`mo-ipc`、`mo-windows-pipe`、`mo-windows-platform`、`mo-broker`、`mo-rime-sys`、`mo-rime`。
- `cargo +stable fmt --all -- --check`：通过。
- `cargo +stable clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo +stable test --workspace`：通过，共 110 个运行时测试和 1 个 compile-fail 契约测试；显式 `mo-broker/latency-trace` debug 分支 112 项及 1 项 compile-fail 通过。构建高负载时曾有启动 watchdog fixture marker 未到达的负向结果，保留于 ADR 0024，编译结束后的两套全 workspace 通过。
- 默认 release 的 workspace/all-targets Clippy 与 Broker 启动负向 smoke：通过；CI 已新增独立约束关闭 debug assertions 的安装模式分支。
- `cargo +stable doc --workspace --no-deps`：通过。
- 默认/诊断 feature 的 debug/release 四种 workspace/all-targets Clippy 均通过；默认及 release feature 的计时 envelope 以编译期断言保证零大小，默认不启动 logger。TIP 的 CMake 路径本机未运行，其证据来自 MSBuild；core + Lua runtime 已用锁定 CMake 3.31.10/VS2022 完成全新目录构建。
- Rust toolchain、librime、rime-ice 与官方验证资产均已锁定；GitHub Actions 已覆盖 Rust、TSF x64/x86、librime ABI 和真实 rime-ice smoke。
- 尚未在 GitHub runner 上产生首个 CI 结果；本地 `main` 已建立 Phase 0 基线提交 `25ef5d7`，未配置 remote。

## G1：librime FFI——本机通过

- 上游：librime 1.17.0，commit `33e78140250125871856cdc5b42ddc6a5fcd3cd4`。
- 官方 x64 验证资产 SHA-256：`7478c7caa4ff6b37de86daba1f7ce4a994a4f5ba24872a820fb2b3a9b01fed15`。
- `tools/abi-probe/compare.ps1`：官方 C header 与 Rust 声明的 51 项 size/alignment/offset/data_size 断言一致，包含可选当前页选择/翻页 API 尾部；本机 FFI 布局验证目标为 x64 Broker。
- 安全封装确保 Engine/Session 单线程、Session 借用 Engine、所有 commit/context/status native 输出严格配对 `free_*`，并将返回值复制为 owned Rust snapshot。
- `RimeBackend` 已实现 Engine Actor 后端契约：私有 native session id 不越过适配层，key/commit/clear、修饰位映射、composition UTF-8 byte offset 校验、候选注释/标签与状态投影均有确定性伪 API 测试。当前页零基索引选择与前后翻页已通过安全可选 API 接入 Actor，旧表/截断字段/空函数指针明确报告缺失，不破坏基本按键输入；schema/options 仍显式返回 unsupported。
- `candidate_smoke` 已对真实 librime/rime-ice 验证 `ni` 前后翻页恢复原页、选择当前页第二候选及准确 commit；该直接 API 检查已纳入完整真实引擎 smoke，并只使用可回收的独立用户目录。
- Broker 后端新增一个私有、无输入的 native 资源保活会话，永不承载用户 key/commit/clear、没有 wire token，用户会话仍各自创建/销毁。两个伪 API 测试验证隔离/销毁顺序与创建失败回收；保活+预编译的直接 API 对照中，首次转换约 27 ms，后续新会话首键约 0.7–1.7 ms，不是压力或机器冷启动通过。
- `Engine::load` 已用绝对 canonical DLL 路径和受限 `LoadLibraryExW` 搜索目录实现运行时加载；不读取 PATH/当前目录，解析 `rime_get_api` 及同一 DLL 的可选版本化 Mo 准备导出，并保证 finalize 后才卸载 DLL。当前只解析 v2、不回退 v1；相对路径、错误文件名和缺失文件均 fail-closed，要求准备的模式不允许缺导出回退。
- 官方 librime + 锁定 rime-ice 完成真实部署和 `nihao -> 你好` 候选及提交验证。
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
- 首版自主 Win32/GDI 候选表现层已接入 TIP：纵向 ordinal 列表、鼠标翻页/选词、不激活窗口、按宿主 DPI 缩放、monitor work area 避让、长文本省略和本页滚轮。collapsed caret 的零宽度矩形可定位，全零/裁剪/无 layout 则隐藏。布局 sink 对称 advise/unadvise，以带身份的异步只读锁更新锚点。UI-element-only 宿主不显示自绘窗；尚未实现 TSF UIElement 协作。
- 鼠标动作在持有 owner/context 的 TSF 可同步或异步写锁内重新验证焦点/context/generation/session/revision，排队期间不预先生成 commit。x64/Win32 fake 与真实词库受控探针均核对显隐、不抢焦点、旧按下保护、鼠标翻页/上屏与文档布局跟随；强制延迟写锁后，新按键或焦点丢失使旧动作取消。焦点恢复后完成第三轮选词，最终从 EDIT 和 TSF context 核对 `mmm`/`你好你好你好`。候选窗仅原生表现层，业务与授权仍在 Rust，阶段性边界见 ADR 0018。
- TIP 在固定的 16 个管道槽中轮转扫描，有忙槽时在原端到端硬时限内短重试；每个新 handle 都重新复核服务端身份，身份异常直接拒绝而不跳过。首次全部端点缺席立即 fail-open；曾认证后允许在同一时限内等待 Broker 重启。前景焦点恢复重置退避，该时序已由上述焦点恢复探针覆盖。
- Broker 启动模式已 fail-closed：启用 debug assertions 的开发构建才接受 `--fake` 或带显式 DLL/shared/user 路径及部署标记的 `--rime`。默认 release 只接受无参数启动，从 Known Folder API 构造固定 Program Files/LocalAppData 布局并核对当前映像位置；真实初始化失败不会静默降级。Windows verbatim canonical 路径在传给 librime 前正规化，已实证避免 librime-lua/用户库路径失效。
- ADR 0029 补齐安装态 Broker bootstrap：只有精确 `Program Files\Mo\tip\<arch>\mo-tip.dll` 获得固定相邻 Broker 的进程创建权限，搬迁/仓库布局保持 connect-only。无 shell/参数/继承 handle/控制台启动，Broker image reparse 拒绝；x86 对不可用的 ProgramFilesX64 Known Folder 只读 HKLM 64 位视图回退。端点缺失才启动，2 秒进程节流与 16 槽 first-instance 绑定保证唯一存活 Broker；并发宿主仍可能短暂创建多个 contender。连接后的 SID/PID/文件身份校验不变。
- release Broker 在完整验证机器资产后逐级创建精确的 `LocalAppData\Mo\Rime\build`，拒绝路径逃逸、文件占位和每一级 reparse point，空 user/staging 直接使用机器 prebuilt。debug/release 安装布局测试、双架构 launcher 探针、fake 3 轮/架构故障矩阵均通过。全新 72-source/137-file 开发素材清单 SHA-256 `C90F600DEE154E293884B3835525101DCFAB2CAB6617BB3580746BEC99681FB1`，79 项素材策略、7 组 golden 及真实素材双架构各 10 轮（40 次明确 Broker 退出）通过。完整 Program Files 安装组合、热路径进程创建延迟与 AppContainer 尚未验收。
- 安装模式将 DLL/shared/prebuilt/相邻 OpenCC 固定在机器安装根，把 user/staging 固定在当前用户根；缺少目录、default/schema 标记或六份必要转换文件时在 Pipe 创建前退出，不自动部署或回退。完整布局 fixture 已验证字段隔离及每份转换文件缺失拒绝；release 子进程已证明诊断参数和仓库映像被拒绝。真实安装资产加载、签名/ACL、全安装树 reparse 防护与用户配置覆盖策略仍未完成。
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

- 注册后的真实 TSF 宿主 key sink 激活，以及 Notepad/WinUI 中的正式 composition/candidate UI；当前候选窗与 Edit Session 证据来自不注册系统 TIP 的受控文本存储探针。混合 DPI/多屏人工矩阵、真实 schema 的选择标签/高亮/注释/页边界投影仍待完成。
- 连接池真实多应用宿主/满载恢复矩阵、已认证连接空闲租约、严格输入延迟指标；当前 overlapped I/O 仍在固定连接线程内等待，不是 IOCP 全异步调度。watchdog 约束进程健康，不声称原生操作可被安全取消。协调停机的生产服务控制/托盘/更新接入和自动重启仍未实现；极端内核/驱动不完成取消尚未注入。发布版来源/签名/安装 ACL/reparse、配置覆盖与 AppContainer/WinUI 仍未完成。
- 实际崩溃回归目前只覆盖上述受控文本存储。engine commit 后/TSF 写入前、部分文档写入后的歧义故障及普通宿主矩阵仍未注入；不声称跨崩溃 exactly-once 或未提交输入不丢失。
- 机器冷启动和更广宿主/候选生命周期矩阵仍未通过。历史高频词库的首个 N 超时主要位于引擎转换；Emoji 延迟加载/weak owner 重复卸载有源码与独立消融证据。仅保活+预编译时，完整 x64 命令第 82/100 轮出现 Actor 55,959 µs、排队 3,469 µs 超时，未进入该命令 Win32 压力段，更早失败也保留。此前 x64 第 24/100、3/100 轮候选未显示（按键未超时），状态变化全部来源尚未确认。ADR 0024 prepared 自构建产物的双架构各 100 轮样本现已通过，但不据此宣称旧候选消失问题全部根因已确定。未放宽 deadline、禁用 Emoji 或自动重发；历史负向证据见 ADR 0022/0023。
- Windows 11 x64 真实桌面宿主矩阵；本次仅在 Windows 10 22H2 验证编译和 COM 加载。

## G3：安装——未通过

- 已有 WiX v4 MSI/Bundle 占位 authoring 和独立 registrar 源码。
- 开发态 registrar 已补齐当前用户 COM activation：以显式 WOW64 视图分别注册 x64/x86 `InprocServer32`，拒绝相对/缺失文件与冲突路径；`status` 可读回 COM/profile 启用状态。隔离测试 CLSID 已连续两次完成双视图写入、读回和无残留清理，未注册或启用 Mo profile。
- 已实现注册宿主 smoke 的权限拆分：提升脚本只管理机器级 TSF profile/category，普通权限脚本临时管理 HKCU 双视图 COM 与当前用户启用，并通过系统 `ITfKeystrokeMgr` 驱动 x64/Win32 探针后在 `finally` 中回滚用户状态。当前非提升开发会话只验证了双架构编译、权限门和干净状态，尚未执行需要人工提升准备的真实注册路由，因此 G2 不据此升级为通过。
- fake 与真实词库 smoke 均已支持 `-Registered`，共享严格用户态事务：提升的测试/Broker 被拒绝，准备不足在构建/部署前拒绝；部分 native 写入失败仍触发独立清理，清理错误或状态残留不再只告警后报成功，检测到外来 COM 路径则保留并要求人工审查。16 个内存策略场景通过，不修改 Windows 输入状态。当前管理员准备仍缺失（profile=false），两种 registered 入口已实证在此前置条件下拒绝且没有残留；真实注册路由仍未运行。具体步骤见 `REGISTERED-TEST.md`。
- 构建脚本默认拒绝生成不可部署安装包；本机未安装 WiX，已验证其 fail-closed 行为。
- WiX 占位入口已改为只接受验证过的 `StageDirectory`，移除任意 Broker/TIP/registrar 参数；独立素材管线可准备 runtime/预编译数据/双架构前端，但当前 WXS 仍只收原来的四个 Mo 二进制，未加入完整 data/runtime 或事务，因此 G3 不升级。
- 尚无真实 TSF 注册/启用事务、升级/修复/卸载回滚、签名、首次启动性能或“不抢默认输入法”测试。
- 自动拉起与首次用户目录代码已完成，但尚未把 release payload 安装到 Program Files 做“注册 TIP -> 拉起 -> 首次目录 -> 普通应用输入”的组合验收；G3 因此仍未通过。
- 占位安装器不得分发。

## G4：数据与许可证——部分通过

- rime-ice 2026.06.30 锁定到 `6810e8916d160498620a16fef2135956fecbd485`，source archive hash 已记录。
- 已从源部署完整 rime-ice 数据并运行真实 golden smoke。
- 新增固定安装布局的开发素材准备管线：核对 core+Lua/v2 来源与 33 份转换资源，从锁定 rime-ice archive 的 64 份输入全新编译 29 份 schema/词库，并从 68 份 Mo 源码快照全新构建 release Broker 和双架构前端。131 个 payload/6 个 evidence 文件有严格清单与依赖 receipt，原样保留上游归档/LICENSE/Credits；79 项构建清单/拒绝测试通过，包含 shell 路径展开拒绝。空 managed user/staging + explicit prebuilt 路径完成中文/Emoji/英文/日期/Unicode/数字/计算器 7 组 exactly-once golden，未在线生成词库。所有产物仍 development-only、不可安装/分发；不是签名、完整 SBOM 或正式许可结论。见 ADR 0026。
- 已新增 hash 锁定 OpenCC 1.1.9 + bundled Marisa 的本地构建态编译工具，把锁定 Emoji/补充字典生成 `.ocd2`，读回核对全部 4857/1498 条 key 及有序 values。20 项完整性/负向检查通过；源文件与 manifest 保留，测试只复制到新 fixture。该 pack 未接入正式安装、签名更新或发行 SBOM，不把自声明哈希作为可信更新证明。
- 发现官方 librime Windows 资产静态包含 GPL-3.0-only `librime-octagram`。该资产现被明确限制为开发验证，不进入 Mo 发行物。
- 正式包必须从锁定 librime 源自行构建，插件采用允许列表；当前最小集合为 BSD-3-Clause core + rime-ice 必需的 BSD-3-Clause `librime-lua`。
- 新增 core + Lua 的允许列表开发运行时构建器，消费六份锁定 Git archive 与显式固定哈希工具包；拒绝旧输出、错误来源，禁用外部插件和 native 内容日志。OpenCC/core 共用 pinned Marisa 0.3.1，避免 bundled 0.2.6 覆盖库的头文件/ABI 混用。v2 只读取 DLL 相邻 `opencc` 资源，不使用 prefix/CWD/user/shared 搜索；构建前快照 11 份 Mo 输入及已验证 Emoji pack，format 2 provenance 记录 33 份资源与 DLL 哈希。证据见 ADR 0024/0025；不是内容认证、许可证批准或可发行包，签名/安装权限/逐文件 SBOM 仍未通过。
- rime-ice 资源仍按 GPL-3.0-only 独立边界处理；默认捆绑前仍需逐文件 SBOM、第三方通知、对应源/修改记录和正式许可证审查。

## 下一检查点

Mo 转换资源搬迁、开发素材/预编译 pack、宿主终止/探针隔离，以及固定安装态 Broker 拉起和首次用户目录 bootstrap 已分别闭环，见 ADR 0025–0029。下一步是在管理员明确准备后完成双架构系统路由，在 Notepad 验收 composition、候选窗、自动拉起、首次目录和 Broker 故障恢复，再覆盖 WinUI/AppContainer/混合 DPI；步骤见 `REGISTERED-TEST.md`，不自动启动 UAC 或改默认输入法。同时补齐 dispatch 发送前长停顿、热路径进程创建监督器、全安装树权限/祖先目录、完整 Rime/Lua 覆盖策略和资源内容认证。随后完成签名、逐文件 SBOM、可回滚安装/升级/卸载与首次启动体验；素材管线不升级为普通宿主或日常使用通过。
