# Mo（墨）输入法

Mo 是一款开源、离线优先、安装即用的 Windows 中文输入法。目标体验接近搜狗、讯飞：用户只安装一个签名安装包，不需要编辑 YAML、替换词库或手工部署 Rime。

项目已经进入 Phase 0 风险验证，**目前还不是可日常使用的输入法**。

## 已落地的基线

- Rust 领域模型与单线程 Engine Actor，包含 generation/revision 防陈旧状态机制；Broker 的 session/key/close 已全部经过 Actor，不再维护旁路输入状态。
- 有大小上限、版本协商、严格 request id 和会话隔离的二进制 IPC 协议；Windows Named Pipe 已具备双向登录会话 SID 复核、TIP 侧 Broker PID/映像身份校验、拒绝远程客户端和首帧/半帧硬超时，且 Broker/Engine 可跨连续连接常驻，TCP 仅保留为诊断 spike。
- Broker 已接入 16 个独立受保护的连接槽，共享唯一 Engine Actor；每槽保留原始服务端 handle，断线复用不产生名称重建缺口，也不向客户端授予创建服务端实例的权限。x64/x86 fake 与真实词库探针已通过 16 路同时连接、满载限时返回、槽复用和跨连接候选隔离，见 ADR 0019。
- librime 1.17.0 最小 C ABI 声明、安全 RAII 封装与 `EngineBackend` 适配器；Broker 以受控绝对路径加载 DLL，不依赖 PATH 或当前目录，失败时不会回退伪引擎。C/Rust ABI probe 覆盖 51 项布局断言，原生输出在进入 Actor 前全部转为 owned 领域快照。Actor 的当前页候选选择和前后翻页已用真实 rime-ice 验证。
- 默认 release Broker 只接受无参数的固定安装布局，路径来自 Windows Known Folder API；`--fake`/调用者指定运行时仅在启用 debug assertions 的开发构建可用。发布版目录 ACL、签名资源和首次启动准备仍未验收。
- 锁定 rime-ice 2026.06.30，并已用真实 librime 验证 `nihao -> 你好`。
- Mo 自主实现的极薄 C++ TSF/COM 壳，可编译为 x64/x86，并通过加载、类工厂、接口与卸载 probe。
- TSF 壳已实现 `OnTestKey*`/`OnKey*` 单次决策缓存、同步读写 Edit Session、预编辑 Range/Composition 生命周期和严格 UTF-8 转换；x64/x86 受控 TSF 文本存储探针均已把 Broker 提交写入真实 Windows EDIT 控件，且 Broker 不可用时 fail-open。
- 已接入自主 Win32 纵向候选窗、鼠标选词/翻页、DPI 缩放和屏幕边缘避让。候选动作通过协商 feature 绑定当前会话的 revision，拒绝陈旧/越界点击；鼠标在可同步或异步的 TSF 编辑锁内执行，并在锁内再次复核身份。布局变化使用异步只读定位。x64/x86 受控真实词库探针已通过鼠标上屏、松键刷新、延迟动作取消与焦点恢复；普通软件尚未验收。
- x64/x86 原生链路均已通过受限 Named Pipe 与同一个 x64 Rust Broker 完成真实握手、会话、按键与候选动作往返，并穿过真实 librime/rime-ice 验证 `nihao + Space -> 你好`、PageUp/PageDown、数字选词、前后翻页和当前页第二候选提交；CI 同时核对 TIP Edit Session 写入 EDIT 和 TSF context 的最终文本，而非仅停留在 IPC 快照。
- WiX v4 安装器占位工程会主动拒绝生成“看似可发布”的安装包；真实注册、启用、修复与卸载尚未实现。

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

该脚本验证 C++ x64/x86 的 IPC 快照、16 路连接池与 TIP Edit Session 上屏真实引擎链路；较小的 Rust FFI 单层验证仍可用 `tools/rime-smoke/run.ps1`。

设计基线见 [产品与软件架构设计 v0.2](docs/MO-INPUT-METHOD-DESIGN-v0.2.md)，当前实证见 [Phase 0 状态](docs/phase-0/STATUS.md)，硬验收门见 [Phase 0 验收门](docs/phase-0/ACCEPTANCE.md)。
