# ADR 0015：TIP 反向认证 Broker 服务端身份

- 状态：接受
- 日期：2026-09-15

## 背景

Broker 已通过 protected DACL、logon SID 模拟复核和首帧 deadline 认证客户端，但 TIP 过去只要能打开固定 pipe 名称就发送 `Hello`。若未来为并发实例向同一登录会话授予 `FILE_CREATE_PIPE_INSTANCE`，同用户进程可能抢先创建同名服务端并观察或伪造输入协议。客户端必须在协议交换前反向确认 pipe 的拥有者。

单纯比较进程路径字符串不足以处理大小写、硬链接或路径别名；只比较用户 SID 又不能排除同用户恶意进程。认证还不能依赖 PATH、当前目录或用户可控环境变量，否则安装布局本身不构成信任锚。

## 决策

- `BrokerClient` 必须持有绝对的预期 Broker 映像路径；路径为空或非 drive-absolute 时拒绝连接。
- TIP 从自身模块路径识别固定的生产布局 `<root>\tip\<arch>\mo-tip.dll -> <root>\bin\mo-broker.exe`。仓库探针只识别完整的 `native\windows-tip\out\msbuild\<arch>\Release` 结构；独立 IPC 探针显式接收绝对 Broker 路径。任何路径都不从环境变量或当前目录读取。
- 打开 pipe 后、发送 `Hello` 前调用 `GetNamedPipeServerProcessId`，以 `PROCESS_QUERY_LIMITED_INFORMATION` 打开服务端进程并读取其映像路径。
- 分别读取宿主与服务端 token 中带 `SE_GROUP_LOGON_ID` 的 SID，要求两者相等。随后打开预期和实际映像文件，以卷序列号及 64 位文件索引比较文件身份，不以路径字符串相等作为结论。
- 任一步失败立即关闭 pipe，TIP 保持既有 fail-open 键盘语义。x64/Win32 负向探针用真实 Broker pipe 配合错误预期映像，验证客户端在握手前拒绝；正向 IPC 和 Edit Session 探针继续验证合法 Broker。

## 结果

同用户任意可执行文件不能只靠抢占 pipe 名称冒充 Broker，且开发/生产路径来源被显式分离。这为后续多实例 listener 提供了必要的客户端认证前置条件。

该校验不是代码签名，也无法阻止攻击者用受信的开发版 `mo-broker.exe --fake` 作为 confused deputy。正式放宽 `FILE_CREATE_PIPE_INSTANCE` 前，仍须移除发行版诊断启动模式、证明安装目录 ACL 与签名策略，并完成多实例竞争/抢占测试；因此本 ADR 不授权立即更改 pipe DACL。
