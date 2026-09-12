# ADR 0009：受控 librime 运行时启动

- 状态：Accepted
- 日期：2026-09-12

## Context

Broker 已能通过 `RimeBackend` 驱动 Engine Actor，但链接期 `rime.lib` 和 Windows 默认 DLL 搜索顺序不适合作为产品启动边界：安装路径可能变化，PATH/当前目录可被环境影响，加载失败后若静默回退到伪后端则会形成“看似可输入、实际不是产品引擎”的错误状态。rime-ice 又必须先部署出完整 build 目录，不能把缺失或未部署的数据目录交给 librime 后再碰运气。

## Decision

- Windows Broker 只接受两种显式模式：`--fake` 是测试专用诊断模式；`--rime <absolute-rime.dll> <shared> <user>` 是真实引擎模式。无参数、参数多余或真实引擎初始化失败均直接退出，禁止自动回退。
- `mo-rime::Engine::load` 只接受绝对路径且文件名必须为 `rime.dll`。路径先 canonicalize，再使用 `LoadLibraryExW` 的 `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32`；不搜索 PATH、当前目录、应用目录或用户 DLL 目录。
- 运行时解析且只解析 `rime_get_api`，随后复用既有 API table 长度与必需函数校验。DLL owner 保存在 Engine 中，析构顺序保证先 `cleanup_all_sessions/finalize`，再 `FreeLibrary`。
- shared/user 目录必须是绝对、存在且可解析的 Unicode 路径。启动前要求 `default.yaml`、`rime_ice.schema.yaml` 及对应的 `user/build` 部署产物存在。Windows canonicalize 产生的 verbatim 前缀在传给 librime 前恢复为普通 DOS/UNC 绝对路径，避免 librime-lua 和用户库路径拼接失效。
- Named Pipe adapter 接受泛型 `EngineBackend` 注入；默认入口仅保留给 `--fake` probe，真实入口构造唯一的 `Engine -> RimeBackend -> EngineActor -> BrokerConnection` 所有权链。
- 真实 smoke 不再只从 Rust 直接调用 Session，而是让 x64 与 Win32 C++ 客户端分别执行 `nihao + Space`，穿过受限 Named Pipe、Broker、Actor、RimeBackend 和运行时加载的 librime，核对候选及提交均含 `你好`。

## Consequences

- Broker 不需要链接或随构建定位 `rime.lib`，部署位置也不需要加入 PATH；启动配置与错误原因是可观察且确定的。
- DLL 同目录依赖仍被允许，因此正式安装目录必须由安装器写入并用 ACL 防止普通进程篡改；本决策不替代签名、hash、SBOM 或插件允许列表验证。
- 当前官方 librime 资产只用于开发/CI 实证。正式发行仍必须切换到 Mo 从锁定源码构建、符合许可证策略的 DLL 和预编译资源包。
- 当前 listener 一次服务一个连接并让该连接拥有 Engine；多客户端 listener pool 与进程级 actor channel 留到后续实现。
