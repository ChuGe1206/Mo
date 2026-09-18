# ADR 0016：安装模式 Broker 与开发诊断启动隔离

- 状态：接受
- 日期：2026-09-16
- 后续：ADR 0025 在固定机器布局加入 `Mo/runtime/librime/opencc` 与六份必要资源预检查；签名/ACL 与全部配置覆盖策略仍是独立门。

## 背景

ADR 0015 已让 TIP 核对 pipe 服务端 PID、logon SID 与预期映像身份，但若受信映像仍允许 `--fake` 或任意 `--rime` DLL/资源路径，同用户进程可以启动这个受信映像来绕过客户端的身份判断。这是发布版需要消除的可选运行时入口，而不是降低 Pipe 校验的问题。

产品运行目录还不能由 `ProgramFiles`、`LOCALAPPDATA` 环境变量、当前目录或传入参数决定，否则调用方仍能改变信任根。普通用户首次启动也不应该承担 Rime 源资源部署工作。

## 决策

- 以 `cfg(debug_assertions)` 隔离开发入口。启用 assertions 的开发构建保留显式 `--fake`/`--rime` 探针；关闭 assertions 的安装构建不包含 `StartupMode::Fake`，只接受无参数启动。默认 Cargo release 属于后者；发行构建必须禁止自定义 profile 重新开启 assertions。
- 新增 `mo-windows-platform`，集中封装 COM 初始化与 `SHGetKnownFolderPath` 的 owned 路径复制/配对内存释放。路径来自 `FOLDERID_ProgramFilesX64` 和当前进程用户的 `FOLDERID_LocalAppData`，不读取同名环境变量。
- 固定机器布局：`ProgramFilesX64/Mo/bin/mo-broker.exe`、`Mo/runtime/librime/rime.dll`、`Mo/data/rime-ice` 与其 `build` 子目录。固定用户布局：`LocalAppData/Mo/Rime` 与其 `build` staging 子目录。
- 先核对当前映像与预期安装映像的 canonical filesystem path，再检查 DLL、数据目录和 shared/prebuilt 的 default/schema 标记；失败发生在创建 Pipe 或加载 native runtime 之前。
- `EngineConfig` 显式区分 shared/prebuilt 和 user/staging。这里只消费已准备好的目录，不创建用户目录、不在线部署、不回退诊断后端。
- 配置 fixture 覆盖完整布局、缺少预编译 schema、外部映像与资源字段隔离；子进程 smoke 直接运行默认 release 可执行文件，验证 `--fake`、调用者运行时、未知参数与仓库位置均被拒绝。CI 同时 lint 开发与安装编译分支。

## 结果与剩余边界

默认 release 已没有调用者指定 native runtime 的命令入口，仓库或任意目录里的副本不能作为无参数安装进程启动。开发态真实 Rime/TSF 探针仍保留原工作流。

这不是完整发布认证：占位安装器尚未证明构建来源或拒绝诊断二进制，安装目录 ACL/签名/reparse 防护尚未实现，librime 的用户配置与 Lua/staging 覆盖策略也没有闭环。fixture 只生成配置，不加载真实安装资源；自构建允许列表 librime、资源包及用户目录准备仍是后续验收项。Pipe 的 protected DACL 保持不变，不能仅凭本 ADR 启用多实例并发。

Known Folder API 与内存责任依据 [Windows 官方文档](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shgetknownfolderpath)；prebuilt/staging traits 对应锁定版本的 [librime SetupDeployer](https://github.com/rime/librime/blob/33e78140250125871856cdc5b42ddc6a5fcd3cd4/src/rime/setup.cc)。
