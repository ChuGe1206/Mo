# Phase 0 状态

- 快照日期：2026-09-12
- 结论：Phase 0 已启动，G1 本机证据闭环；G2/G4 部分通过；G3 未通过。项目尚不可安装或日常使用。
- 本机环境：Windows 10 22H2 build 19045（尽力兼容环境）、Rust 1.97.1 x86_64-pc-windows-msvc、Visual Studio 2022 17.14.37、MSVC 14.44、Windows SDK 10.0.26100.0。

## G0：工程基线——本机通过，远端 CI 待首次运行

- Cargo workspace 包含 `mo-domain`、`mo-engine`、`mo-ipc`、`mo-windows-pipe`、`mo-broker`、`mo-rime-sys`、`mo-rime`。
- `cargo +stable fmt --all -- --check`：通过。
- `cargo +stable clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo +stable test --workspace`：通过，共 50 个运行时测试和 1 个 compile-fail 契约测试。
- `cargo +stable doc --workspace --no-deps`：通过。
- Rust toolchain、librime、rime-ice 与官方验证资产均已锁定；GitHub Actions 已覆盖 Rust、TSF x64/x86、librime ABI 和真实 rime-ice smoke。
- 尚未在 GitHub runner 上产生首个 CI 结果；本地 `main` 已建立 Phase 0 基线提交 `25ef5d7`，未配置 remote。

## G1：librime FFI——本机通过

- 上游：librime 1.17.0，commit `33e78140250125871856cdc5b42ddc6a5fcd3cd4`。
- 官方 x64 验证资产 SHA-256：`7478c7caa4ff6b37de86daba1f7ce4a994a4f5ba24872a820fb2b3a9b01fed15`。
- `tools/abi-probe/compare.ps1`：官方 C header 与 Rust 声明的 44 项 size/alignment/offset/data_size 断言一致。
- 安全封装确保 Engine/Session 单线程、Session 借用 Engine、所有 commit/context/status native 输出严格配对 `free_*`，并将返回值复制为 owned Rust snapshot。
- `RimeBackend` 已实现 Engine Actor 后端契约：私有 native session id 不越过适配层，key/commit/clear、修饰位映射、composition UTF-8 byte offset 校验、候选注释/标签与状态投影均有确定性伪 API 测试。schema/options/选词/翻页仍因最小 ABI 前缀未扩展而显式返回 unsupported，不会静默忽略。
- `Engine::load` 已用绝对 canonical DLL 路径和受限 `LoadLibraryExW` 搜索目录实现运行时加载；不读取 PATH/当前目录，仅解析 `rime_get_api`，并保证 finalize 后才卸载 DLL。相对路径、错误文件名和缺失文件均 fail-closed。
- 官方 librime + 锁定 rime-ice 完成真实部署和 `nihao -> 你好` 候选及提交验证。
- 正式发行不得复用该官方预构建 DLL；原因见 G4。

## G2：TSF 与 IPC——部分通过

已通过：

- 自主 C++ COM 壳以 `/W4 /WX /sdl /GS /guard:cf` 构建 x64 与 x86 DLL，零警告。
- 两种架构均通过 DLL 加载、导出、未知 CLSID、Server Lock、禁止聚合、`ITfTextInputProcessorEx`/`ITfKeyEventSink` 创建、错误参数和卸载探针。
- `ActivateEx` 已实现前台 key sink 的 advise/unadvise 对称生命周期；受控 manager + 真实 TSF context 探针证明 client ID/foreground 参数正确，`OnTestKey*` 与 `OnKey*` 当前一致 fail-open、不吞键。
- x64 与 Win32 原生客户端已对同一个 x64 Rust Broker 完成真实 `Hello -> OpenSession -> KeyEvent(M) -> Snapshot("m") -> CloseSession` 往返；并分别通过 `KeyEvent(nihao + Space)` 穿过 Broker、Actor、RimeBackend、运行时加载的 librime 与锁定 rime-ice，核对 `你好` 候选和提交。客户端采用 overlapped I/O、端到端硬 deadline、严格帧/CRC/UTF-8/请求号校验，失败或结果不明确时断开并保持 TIP fail-open。
- Broker 启动模式已 fail-closed：`--fake` 仅供诊断，`--rime` 必须显式提供 DLL/shared/user 绝对路径和已部署标记；真实初始化失败不会静默降级。Windows verbatim canonical 路径在传给 librime 前正规化，已实证避免 librime-lua/用户库路径失效。
- Broker 已移除连接内的诊断 ASCII echo 状态：wire session token 映射到 Engine Actor 的 generation-safe token，创建、按键和销毁全部经过可替换后端的 Actor；跨 session snapshot 使用同一全局 revision 顺序，断开时回收仍存活的引擎会话。真实启动使用 `RimeBackend`，确定性的 `FakeBackend` 只保留为显式测试模式。
- IPC 有 64 KiB 硬上限、最小可接收响应协商、CRC32 破损检测、UTF-8 校验、版本协商、严格递增 request id、connection generation、会话隔离和会话数量上限。
- Windows Named Pipe 使用 `LOCAL` 命名、当前 logon SID 受保护 DACL、`PIPE_REJECT_REMOTE_CLIENTS` 和 identification-only SQOS；服务端读取首个有界帧后模拟客户端并复核 logon SID，失败路径不进入 Broker 状态机。
- Named Pipe 已从真实内核对象读回并核对 protected DACL/唯一 ACE/SID/权限掩码，同时通过远程拒绝标志、端点逃逸拒绝、静默客户端首帧超时和 `Hello -> HelloAck` Broker 往返测试。
- 诊断 TCP transport 已通过真实 loopback framed I/O 测试；它不构成生产传输安全结论。

未通过：

- 注册后的真实 TSF 宿主 key sink 激活、key callback 到 Broker 的派发、edit session 和 composition/candidate UI；当前真实 Broker 往返由同源码的独立原生 probe 验证。
- Named Pipe 多实例/overlapped I/O、已认证连接逐请求 deadline、DACL 负向访问测试，以及 AppContainer/WinUI 连接测试。
- Broker 超时、崩溃恢复、幂等提交与“不重复上屏”故障注入。
- Windows 11 x64 真实桌面宿主矩阵；本次仅在 Windows 10 22H2 验证编译和 COM 加载。

## G3：安装——未通过

- 已有 WiX v4 MSI/Bundle 占位 authoring 和独立 registrar 源码。
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

Phase 0 的下一检查点继续聚焦同一垂直链路：把已经嵌入 TIP DLL 的 BrokerClient 接到 key sink 决策缓存与 TSF edit session，完成真实宿主内的 composition/候选展示和幂等上屏。该链路通过 Notepad、WinUI/AppContainer、Broker 故障注入后，再固化发布版自构建 librime/资源布局并实现可回滚安装事务。
