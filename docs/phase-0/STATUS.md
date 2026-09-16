# Phase 0 状态

- 快照日期：2026-09-16
- 结论：Phase 0 已启动，G1 本机证据闭环；G2/G4 部分通过；G3 未通过。受控 TSF Edit Session 已贯通，项目仍不可安装或日常使用。
- 本机环境：Windows 10 22H2 build 19045（尽力兼容环境）、Rust 1.97.1 x86_64-pc-windows-msvc、Visual Studio 2022 17.14.37、MSVC 14.44、Windows SDK 10.0.26100.0。

## G0：工程基线——本机通过，远端 CI 待首次运行

- Cargo workspace 包含 `mo-domain`、`mo-engine`、`mo-ipc`、`mo-windows-pipe`、`mo-windows-platform`、`mo-broker`、`mo-rime-sys`、`mo-rime`。
- `cargo +stable fmt --all -- --check`：通过。
- `cargo +stable clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo +stable test --workspace`：通过，共 66 个运行时测试和 1 个 compile-fail 契约测试。
- 默认 release 的 workspace/all-targets Clippy 与 Broker 启动负向 smoke：通过；CI 已新增独立约束关闭 debug assertions 的安装模式分支。
- `cargo +stable doc --workspace --no-deps`：通过。
- Rust toolchain、librime、rime-ice 与官方验证资产均已锁定；GitHub Actions 已覆盖 Rust、TSF x64/x86、librime ABI 和真实 rime-ice smoke。
- 尚未在 GitHub runner 上产生首个 CI 结果；本地 `main` 已建立 Phase 0 基线提交 `25ef5d7`，未配置 remote。

## G1：librime FFI——本机通过

- 上游：librime 1.17.0，commit `33e78140250125871856cdc5b42ddc6a5fcd3cd4`。
- 官方 x64 验证资产 SHA-256：`7478c7caa4ff6b37de86daba1f7ce4a994a4f5ba24872a820fb2b3a9b01fed15`。
- `tools/abi-probe/compare.ps1`：官方 C header 与 Rust 声明的 51 项 size/alignment/offset/data_size 断言一致，包含可选当前页选择/翻页 API 尾部；本机 FFI 布局验证目标为 x64 Broker。
- 安全封装确保 Engine/Session 单线程、Session 借用 Engine、所有 commit/context/status native 输出严格配对 `free_*`，并将返回值复制为 owned Rust snapshot。
- `RimeBackend` 已实现 Engine Actor 后端契约：私有 native session id 不越过适配层，key/commit/clear、修饰位映射、composition UTF-8 byte offset 校验、候选注释/标签与状态投影均有确定性伪 API 测试。当前页零基索引选择与前后翻页已通过安全可选 API 接入 Actor，旧表/截断字段/空函数指针明确报告缺失，不破坏基本按键输入；schema/options 仍显式返回 unsupported。
- `candidate_smoke` 已对真实 librime/rime-ice 验证 `ni` 前后翻页恢复原页、选择当前页第二候选及准确 commit；该直接 API 检查已纳入完整真实引擎 smoke，并只使用可回收的独立用户目录。
- `Engine::load` 已用绝对 canonical DLL 路径和受限 `LoadLibraryExW` 搜索目录实现运行时加载；不读取 PATH/当前目录，仅解析 `rime_get_api`，并保证 finalize 后才卸载 DLL。相对路径、错误文件名和缺失文件均 fail-closed。
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
- Broker 已补齐 PageUp/PageDown/Home/End 的 Windows VK -> X11 keysym 映射。x64/Win32 真实 IPC 探针均验证 `ni -> PageDown -> PageUp -> 2`：新页与原页不同、前页恢复、翻页不提交，数字选词准确提交原页第二候选。候选点击命令仍未进入 IPC；可见候选窗及普通宿主候选体验未据此验收。
- Broker 启动模式已 fail-closed：启用 debug assertions 的开发构建才接受 `--fake` 或带显式 DLL/shared/user 路径及部署标记的 `--rime`。默认 release 只接受无参数启动，从 Known Folder API 构造固定 Program Files/LocalAppData 布局并核对当前映像位置；真实初始化失败不会静默降级。Windows verbatim canonical 路径在传给 librime 前正规化，已实证避免 librime-lua/用户库路径失效。
- 安装模式将 DLL/shared/prebuilt 固定在机器安装根，把 user/staging 固定在当前用户根；缺少目录或 default/schema 标记时在 Pipe 创建前退出，不自动部署或回退。完整布局 fixture 已验证配置字段及缺资源拒绝；release 子进程已证明诊断参数和仓库映像被拒绝。真实安装资产加载、签名/ACL、reparse 防护与用户配置覆盖策略仍未完成。
- Broker 已移除连接内的诊断 ASCII echo 状态：wire session token 映射到 Engine Actor 的 generation-safe token，创建、按键和销毁全部经过可替换后端的 Actor；跨 session snapshot 使用同一全局 revision 顺序，断开时回收仍存活的引擎会话。真实启动使用 `RimeBackend`，确定性的 `FakeBackend` 只保留为显式测试模式。
- Engine Actor 已移入进程级专用线程，thread-affine librime backend 在线程内创建和销毁。Broker 在连接断开后以原受保护 DACL 重建 first pipe instance，不再随首个客户端退出；两个连续真实 Named Pipe 连接已验证会话回收与跨连接全局 revision。并发多实例仍等待发布模式收口、签名安装路径和 listener pool 设计，当前不会仅凭映像身份校验就授予客户端 `FILE_CREATE_PIPE_INSTANCE`。
- IPC 有 64 KiB 硬上限、最小可接收响应协商、CRC32 破损检测、UTF-8 校验、版本协商、严格递增 request id、connection generation、会话隔离和会话数量上限。
- Windows Named Pipe 使用 `LOCAL` 命名、当前 logon SID 受保护 DACL、`PIPE_REJECT_REMOTE_CLIENTS` 和 identification-only SQOS；服务端读取首个有界帧后模拟客户端并复核 logon SID，失败路径不进入 Broker 状态机。
- Named Pipe 已从真实内核对象读回并核对 protected DACL/唯一 ACE/SID/权限掩码，同时通过远程拒绝标志、端点逃逸拒绝、静默客户端首帧超时和 `Hello -> HelloAck` Broker 往返测试。
- Named Pipe 负向测试已证明当前登录会话不能创建第二服务端实例；已认证连接从首个可用字节起采用 2 秒完整帧 assembly deadline，半帧超时会断开并重建监听，完全空闲连接不会被误杀。客户端在自己的总 deadline 内跨越安全重建产生的短暂 endpoint 缺口。
- TIP 在发送 `Hello` 前用 `GetNamedPipeServerProcessId` 锁定服务端 PID，复核服务端进程与宿主属于同一 logon SID，并比较进程映像与预期 `mo-broker.exe` 的卷序列号/文件索引。x64/Win32 负向探针均证明错误映像身份被拒绝，随后正确身份仍可完成 IPC 与两轮 Edit Session 上屏。生产路径只从 TIP 自身固定安装布局推导；不接受环境变量或当前目录覆盖。
- 诊断 TCP transport 已通过真实 loopback framed I/O 测试；它不构成生产传输安全结论。

未通过：

- 注册后的真实 TSF 宿主 key sink 激活，以及 Notepad/WinUI 中的正式 composition/candidate UI；当前 Edit Session 证据来自不注册系统 TIP 的受控文本存储探针。
- Named Pipe 安全多实例/服务端 overlapped I/O、发布版构建来源/签名/安装 ACL/reparse 防护及用户配置覆盖闭环、已认证连接空闲租约/完整逐请求 deadline，以及 AppContainer/WinUI 连接测试。
- Broker 超时、崩溃恢复、幂等提交与“不重复上屏”故障注入。
- Windows 11 x64 真实桌面宿主矩阵；本次仅在 Windows 10 22H2 验证编译和 COM 加载。

## G3：安装——未通过

- 已有 WiX v4 MSI/Bundle 占位 authoring 和独立 registrar 源码。
- 开发态 registrar 已补齐当前用户 COM activation：以显式 WOW64 视图分别注册 x64/x86 `InprocServer32`，拒绝相对/缺失文件与冲突路径；`status` 可读回 COM/profile 启用状态。隔离测试 CLSID 已连续两次完成双视图写入、读回和无残留清理，未注册或启用 Mo profile。
- 已实现注册宿主 smoke 的权限拆分：提升脚本只管理机器级 TSF profile/category，普通权限脚本临时管理 HKCU 双视图 COM 与当前用户启用，并通过系统 `ITfKeystrokeMgr` 驱动 x64/Win32 探针后在 `finally` 中回滚用户状态。当前非提升开发会话只验证了双架构编译、权限门和干净状态，尚未执行需要人工提升准备的真实注册路由，因此 G2 不据此升级为通过。
- 构建脚本默认拒绝生成不可部署安装包；本机未安装 WiX，已验证其 fail-closed 行为。
- 尚无真实 TSF 注册/启用事务、升级/修复/卸载回滚、签名、首次启动性能或“不抢默认输入法”测试。
- 占位安装器不得分发。

## G4：数据与许可证——部分通过

- rime-ice 2026.06.30 锁定到 `6810e8916d160498620a16fef2135956fecbd485`，source archive hash 已记录。
- 已从源部署完整 rime-ice 数据并运行真实 golden smoke。
- 发现官方 librime Windows 资产静态包含 GPL-3.0-only `librime-octagram`。该资产现被明确限制为开发验证，不进入 Mo 发行物。
- 正式包必须从锁定 librime 源自行构建，插件采用允许列表；当前最小集合为 BSD-3-Clause core + rime-ice 必需的 BSD-3-Clause `librime-lua`。
- rime-ice 资源仍按 GPL-3.0-only 独立边界处理；默认捆绑前仍需逐文件 SBOM、第三方通知、对应源/修改记录和正式许可证审查。

## 下一检查点

Phase 0 的下一检查点是将已通过受控探针的垂直链路注册到真实 Windows 宿主，完成 Notepad 中的正式 composition、候选窗和幂等上屏，再覆盖 WinUI/AppContainer 与 Broker 超时/崩溃故障注入。随后固化发布版自构建 librime/资源布局并实现可回滚安装事务。
