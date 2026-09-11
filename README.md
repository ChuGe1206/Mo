# Mo（墨）输入法

Mo 是一款开源、离线优先、安装即用的 Windows 中文输入法。目标体验接近搜狗、讯飞：用户只安装一个签名安装包，不需要编辑 YAML、替换词库或手工部署 Rime。

项目已经进入 Phase 0 风险验证，**目前还不是可日常使用的输入法**。

## 已落地的基线

- Rust 领域模型与单线程 Engine Actor，包含 generation/revision 防陈旧状态机制。
- 有大小上限、版本协商、严格 request id 和会话隔离的二进制 IPC 协议；Windows Named Pipe 已具备登录会话 DACL、拒绝远程客户端、首帧硬超时和客户端 SID 复核，TCP 仅保留为诊断 spike。
- librime 1.17.0 最小 C ABI 声明与安全 RAII 封装；C/Rust ABI probe 覆盖 44 项布局断言。
- 锁定 rime-ice 2026.06.30，并已用真实 librime 验证 `nihao -> 你好`。
- Mo 自主实现的极薄 C++ TSF/COM 壳，可编译为 x64/x86，并通过加载、类工厂、接口与卸载 probe。
- WiX v4 安装器占位工程会主动拒绝生成“看似可发布”的安装包；真实注册、启用、修复与卸载尚未实现。

## 架构路线

- Rust-first 混合架构：Rust 承担 Core、Broker、候选窗、包管理和工具；librime 保持上游 C++；Mo 自主实现极薄 C++ TSF 壳。
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
```

真实 librime/rime-ice 冒烟需要显式提供已核验的上游目录：

```powershell
./tools/rime-smoke/run.ps1 `
  -LibrimeDistDir <official-librime-dist> `
  -SharedDataDir <pinned-rime-ice> `
  -UserDataDir <disposable-user-dir> `
  -Deploy
```

设计基线见 [产品与软件架构设计 v0.2](docs/MO-INPUT-METHOD-DESIGN-v0.2.md)，当前实证见 [Phase 0 状态](docs/phase-0/STATUS.md)，硬验收门见 [Phase 0 验收门](docs/phase-0/ACCEPTANCE.md)。
