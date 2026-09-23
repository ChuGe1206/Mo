# Mo（墨）输入法

Mo 是一款开源、离线优先、安装即用的 Windows 中文输入法。目标体验接近搜狗、讯飞：用户只安装一个签名安装包，不需要编辑 YAML、替换词库或手工部署 Rime。

项目已经进入 Phase 0 风险验证，**目前还不是可日常使用的输入法**。

## 已落地的基线

- Rust 领域模型与单线程 Engine Actor，包含 generation/revision 防陈旧状态机制；Broker 的 session/key/close 已全部经过 Actor，不再维护旁路输入状态。
- 有大小上限、版本协商、严格 request id 和会话隔离的二进制 IPC 协议；Windows Named Pipe 已具备双向登录会话 SID 复核、TIP 侧 Broker PID/映像身份校验、拒绝远程客户端和首帧/半帧硬超时，且 Broker/Engine 可跨连续连接常驻，TCP 仅保留为诊断 spike。
- Broker 已接入 16 个独立受保护的连接槽，共享唯一 Engine Actor；每槽保留原始服务端 handle，断线复用不产生名称重建缺口，也不向客户端授予创建服务端实例的权限。x64/x86 fake 与真实词库探针已通过 16 路同时连接、满载限时返回、槽复用和跨连接候选隔离，见 ADR 0019。
- 服务端 connect/read/write 使用 overlapped I/O，首帧、整帧组装、完整回复各有独立时限；超时取消并等待内核完成后才释放存储，不重试歧义提交。flush 不等待客户端读空，空闲等待不再每毫秒轮询，见 ADR 0020。
- Broker 已补齐进程内协调停机、原生引擎请求/finalize watchdog 和整池停机总预算。x64/x86 fake 与真实词库受控探针实际结束/重启 Broker，核对仅清除未提交预编辑、保留已上屏文字、fail-open 和新会话不重放旧词；独立子进程验证八类卡死/panic/总停机故障，见 ADR 0021。安装态 TIP 现可在固定 Program Files 布局下无参数、无窗口拉起 Broker，首次 release 启动会在机器资产验证后创建精确的 LocalAppData 用户目录；完整安装/普通软件组合仍未验收，见 ADR 0029。
- 首键专项已定位 Emoji 延迟加载，并补齐无输入的共享资源保活、构建态 Emoji `.ocd2` 预编译/全部词条读回校验、显式开启的无内容分段诊断与高精度完成检查 deadline；不放宽 50 ms、不注入预热按键。首次转换尾延迟、高频候选显示与普通宿主验收仍待闭环，见 ADR 0022。
- 候选定位增加宿主重入后的 context/range/epoch 身份保护，确定性回归证明并修复“布局已失效却重显旧位置”；按键重连和发送共用一份绝对 50 ms 传输预算，不重复续期。它不替代普通宿主或首次转换尾延迟验收，见 ADR 0023。
- 新增锁定来源的 core + Lua 自构建开发运行时与无输入资源准备扩展，将实际 OpenCC owner 初始化移至 Broker ready 前。v2 固定读取 DLL 旁的 `opencc` 目录，不搜索 cwd/构建 prefix，也不接受用户/共享目录转换资源覆盖；安装模式缺导出/缺资源或只有 v1 均拒绝。签名、安装权限和发行 SBOM 仍待验收，见 ADR 0024/0025。
- 安装态 deployed schema 与 Lua 已收口到 Program Files 机器素材：staging 与 prebuilt 使用同一只读目录，Lua 只搜索机器 shared 路径、禁用 native module，并拒绝用户 `rime.lua`/`lua`。用户词典与学习状态仍写入 LocalAppData；这不是 Lua 沙箱，机器脚本仍须签名和审查，见 ADR 0039。
- 新增 `mo-settings` 强类型设置核心：普通设置写入独立的版本化、非执行配置，不接触 Rime YAML/Lua；16 KiB 上限、精确字段、未来版本拒绝、仅文件缺失使用默认值，并以同目录 flush + Windows 原子替换保存。图形设置 UI 与真实引擎选项仍未完成，见 ADR 0041。
- 设置运行时计划已通过可选、固定 18 字节的 IPC 快照进入 x64/Win32 TIP：Broker 保留 revision 化的最后有效设置，损坏刷新不降级覆盖；客户端只在连接或显式刷新时查询，不增加逐键 I/O。候选窗已应用 System/Light/Dark 主题，其余引擎偏好仍明确未激活，见 ADR 0042。
- 首版 Windows 图形设置中心已用 Rust + 原生 Win32 自主实现：普通用户可直接选择并原子保存候选窗主题，首次运行不落盘，损坏/未来版本设置必须明确点击恢复默认后才覆盖；尚未接入引擎的选项只读展示，不制造“已生效”假象。`mo-settings.exe` 已进入 132-file 离线 stage、开始菜单、合规归属与六个内层 PE 签名合同，见 ADR 0043。
- librime 1.17.0 最小 C ABI 声明、安全 RAII 封装与 `EngineBackend` 适配器；Broker 以受控绝对路径加载 DLL，不依赖 PATH 或当前目录，失败时不会回退伪引擎。C/Rust ABI probe 覆盖 51 项布局断言，原生输出在进入 Actor 前全部转为 owned 领域快照。Actor 的当前页候选选择和前后翻页已用真实 rime-ice 验证。
- 默认 release Broker 只接受无参数的固定安装布局；`--fake`/调用者指定运行时仅在启用 debug assertions 的开发构建可用。TIP 的 x64 路径来自 Known Folder，x86 使用只读 64 位机器注册表回退，不读取环境变量；搬迁素材只允许连接、不允许自动启动。Broker 在创建用户目录、绑定 Pipe 或加载 `rime.dll` 前重新审计 Program Files、完整安装树 owner/DACL、reparse 和文件硬链接数；签名资源、持续防替换、AppContainer 和真实首次启动组合仍未验收，见 ADR 0038。
- 锁定 rime-ice 2026.06.30，并已用真实 librime 验证 `nihao -> 你好`。
- Mo 自主实现的极薄 C++ TSF/COM 壳，可编译为 x64/x86，并通过加载、类工厂、接口与卸载 probe。
- TSF 壳已实现 `OnTestKey*`/`OnKey*` 单次决策缓存、同步读写 Edit Session、预编辑 Range/Composition 生命周期和严格 UTF-8 转换；x64/x86 受控 TSF 文本存储探针均已把 Broker 提交写入真实 Windows EDIT 控件，且 Broker 不可用时 fail-open。
- 已接入自主 Win32 纵向候选窗、鼠标选词/翻页、DPI 缩放和屏幕边缘避让。候选动作通过协商 feature 绑定当前会话的 revision，拒绝陈旧/越界点击；鼠标在可同步或异步的 TSF 编辑锁内执行，并在锁内再次复核身份。布局变化使用异步只读定位。x64/x86 受控真实词库探针已通过鼠标上屏、松键刷新、延迟动作取消与焦点恢复；普通软件尚未验收。
- x64/x86 原生链路均已通过受限 Named Pipe 与同一个 x64 Rust Broker 完成真实握手、会话、按键与候选动作往返，并穿过真实 librime/rime-ice 验证 `nihao + Space -> 你好`、PageUp/PageDown、数字选词、前后翻页和当前页第二候选提交；CI 同时核对 TIP Edit Session 写入 EDIT 和 TSF context 的最终文本，而非仅停留在 IPC 快照。
- WiX v4 已具备完整 payload、机器级 MSI 事务和非提升 current-user finalizer；用户状态使用持久 undo journal、精确 Burn 正反向命令及稳定升级引用计数。最新 82-source/139-file stage 可直接消费锁定源码归档离线重建，并已通过 88 项策略；仓库局部 WiX 4.0.6 已链接并反向核验包含设置中心及开始菜单入口的未签名 MSI/Bundle。安装包尚未执行，MSI ICE、混合 scope 升级/多用户语义、签名及隔离 VM 中真实注册、启用、修复、回滚、升级与卸载均未验收，因此仍不可日常使用。
- 一次性 VM 生命周期测试包已可把已核验 Bundle、registrar 和 132-file stage contract 绑定到同一哈希清单；最新 clean kit 与 `0.0.4.0 -> 0.0.5.0` rollback/upgrade matrix kit 已从当前源码离线重建，并通过实际 inventory 和 33 项策略检查。来宾脚本以虚拟硬件、机器哨兵、双显式开关和非提升令牌防止误在开发主机运行；每个已安装阶段审计 Program Files owner/DACL、重解析点和硬链接。当前主机没有 VM 入口，尚无真实 VM 运行结果，见 ADR 0032/0037/0038/0040。
- 安装器现分为显式 `DevelopmentTest` 与 `ProductionShape` 两条构建路径。故障命令在 registrar 编译期默认移除，MSI/Burn 故障节点也从生产形态链接图物理排除；两条路径的 `0.0.3.0 -> 0.0.4.0` linked 升级对均已反向验证，开发版 VM lifecycle/matrix kit 已按新 evidence 格式重新绑定。生产形态仍是未签名、不可部署的开发验证物，且尚未在真实 VM 执行，见 ADR 0033/0034。
- 生产形态 stage 已能生成哈希绑定、逐字节可重复的 SPDX 2.3 与第三方通知草案：132 个 payload 文件全部且仅归属一个组件。同时已组装并独立核验 9 份锁定源码归档与 15 份许可证/通知，rime-ice GPL 对应源被单独标记；签名合同固定六个内层 PE、MSI、Bundle 的六步内到外顺序，并拒绝开发 flavor 和哈希脱钩。技术材料已闭环，但 librime 组合许可证仍为 `NOASSERTION`，法律审查、实际签名和发行授权尚未完成，见 ADR 0035/0036。

## 架构路线

- Rust-first 混合架构：Rust 承担 Core、Broker、候选命令/页版本授权、包管理和工具；librime 保持上游 C++；Mo 自主实现 C++ TSF 壳，首版候选表现层为独立的小型 Win32/GDI 模块，后续可替换为 Rust 跨平台界面，见 ADR 0018。
- 当前正式支持目标为 Windows 11 x64；Windows 10 22H2 仅尽力兼容。其他平台保留稳定领域模型、C ABI、数据格式和 golden tests 边界，待后续阶段确认。
- 输入热路径完全离线；设置、更新和未来同步与输入进程隔离。
- rime-ice 是锁定的构建输入，由 Mo 生成预编译资源包，最终用户不直接维护它。
- Mo 自有代码采用 Apache-2.0。GPL 资源保持独立边界、对应源和构建记录；正式发行不使用包含 GPL octagram 插件的官方预构建 `rime.dll`。

## 本地验证

```powershell
cargo +stable fmt --all -- --check
cargo +stable clippy --workspace --all-targets -- -D warnings
cargo +stable test --workspace
./native/windows-tip/build-probe.ps1 -Architecture All -Backend MSBuild
./tools/tip-broker-smoke.ps1 -Architecture All
./tools/broker-startup-smoke.ps1
./tools/test-registered-tip-state.ps1
./installer/windows/test-vm-lifecycle-policy.ps1
```

真实 librime/rime-ice 冒烟需要显式提供已核验的上游目录：

```powershell
./tools/tip-rime-smoke.ps1 `
  -LibrimeDistDir <official-librime-dist> `
  -SharedDataDir <pinned-rime-ice> `
  -UserDataDir <disposable-user-dir> `
  -Architecture All `
  -Deploy
```

该脚本验证 C++ x64/x86 的 IPC 快照、16 路连接池、TIP Edit Session 上屏和实际 Broker 退出/重启链路。fake/真实 smoke 都可加 `-FaultRepetitions 20` 重复故障检查，单架构上限 100 轮，失败不自动重试。较小的 Rust FFI 单层验证仍可用 `tools/rime-smoke/run.ps1`。

真实 smoke 可加 `-OpenccDataDir <已校验预编译包>` 和 `-LatencyTrace`。预编译工具及字典完整性检查步骤见 [OpenCC 构建工具](tools/opencc-build/README.md)；该开发包不是可发行安装资源或签名更新包。

允许列表开发运行时的显式来源准备、构建与边界检查见 [运行时构建工具](tools/runtime-build/README.md)。构建必须传入已校验的 `-OpenccDataDir`；搬迁时保持 DLL 与旁边的 `opencc` 目录一起移动。使用该运行时的 smoke 加 `-PreparedResources`；官方或旧 v1 DLL 不能用于这个模式。两者都不是可发行安装包。

注册系统路由测试需要分权限准备，见 [注册测试步骤](docs/phase-0/REGISTERED-TEST.md)。fake 与真实词库 smoke 均可加 `-Registered`，但只允许普通权限运行，并要求机器 profile 已在管理员 PowerShell 中准备好。用户态 COM/启用状态会回滚并严格核对；机器 profile 最后由管理员清理。此流程不设默认输入法，不是安装包或普通软件验收。

候选生命周期定位可使用当前源码和已验证开发素材包，产物必须写入新的绝对 `build` 子路径：

```powershell
./tools/candidate-lifecycle-probe.ps1 `
  -StageDirectory "$PWD/build/mo-windows-stage-candidate-final/stage" `
  -OutputDirectory "$PWD/build/mo-candidate-new" -Repetitions 20 -LatencyTrace
```

去掉 `-LatencyTrace` 验证默认关闭诊断，`-Fake` 验证无真实引擎的宿主状态机。入口不注册/安装、不替换素材包；默认测试宿主不获取前台或窗口队列激活，并禁止把用户当前系统文本服务激活进受控文档。宿主终止清理的确定性修复见 [ADR 0027](docs/adr/0027-host-termination-and-candidate-lifecycle-evidence.md)，旧探针污染的定位与隔离见 [ADR 0028](docs/adr/0028-isolated-tsf-probe-and-termination-origin.md)。不能将隔离压力通过当作普通宿主或日常使用验收。

设计基线见 [产品与软件架构设计 v0.2](docs/MO-INPUT-METHOD-DESIGN-v0.2.md)，当前实证见 [Phase 0 状态](docs/phase-0/STATUS.md)，硬验收门见 [Phase 0 验收门](docs/phase-0/ACCEPTANCE.md)。
